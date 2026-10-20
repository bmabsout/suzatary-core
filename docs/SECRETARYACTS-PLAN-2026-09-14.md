# `secretaryacts` — implementation plan (2026-09-14)

Chain item: PROPOSED ACTS beside proposed emails. `propose_act` stores an
act with its exact arguments; the owner taps; the box performs it and writes
the result into the conversation. First act `auth <email>`: the device-code
URL and code arrive in chat, no terminal on the box. Opus plan against
`f7ed767`, accepted; **[v]** = read in the tree. As built: ARCHITECTURE
§10.3 "Acts" (which supersedes this plan where they differ).

## 0. What already exists

- **[v]** `Proposal` (`app/core/src/secretary/store.rs`), a closed sum; a
  tap carries an index (`Stored::tappable(index)`).
- **[v]** `Post` is NOT a capability (bundled, a send verb would compile);
  the binary supplies `Postman` (fixture `Recording`). `Tap::from_gate` is
  the only `Tap` constructor; loopback gets `actor_of`'s refusal.
- **[v]** `Consent(())` had one constructor, `given_at_a_terminal`.
- **[v]** `TokenStore::authorise` has the seam (`announce: &mut dyn
  FnMut(&DeviceCode)`, `wait`) and enforces `expires_in`; grants at
  `$XDG_STATE_HOME/suzatary/oauth/<slug>.json`, 0600, atomic rename;
  `DeviceCode.device_code` is a `Secret`.
- **[v]** `Turn::Act` starts no round; the worker's `Stream` can announce.

## 1. The types

- **`Act`** (`app/core/src/secretary/acts.rs`): closed sum, one arm
  `Act::Auth { account: EmailAddress }`. No `scope`: the SMTP grant is the
  SEND consent and must not ride in on a tap. `Act::sentence()` is the one
  imperative card and turn both print.
- **`EmailAddress`** newtype (`app/core/src/mail/address.rs`, serde
  `try_from`); `verbs::address` delegates.
- **`Proposal::Act(Act)`**, never a sibling store: `Standing`,
  `Entry::Proposed`, `tappable`, `dismissable`, `proposal:<conv>/<n>`, the
  Inbox count and `proposal.ts` all key off `Proposal` and its index.
- **`Outcome::Started { at, expires_at }`** at tap time (a second tap is
  refused); a SECOND `Entry::Performed` carries `Acted { what }` or
  `Failed { reason }` (fold is last-write-wins per index; document it). A
  `Started` past `expires_at` is refused with "expired; ask the secretary
  for a fresh act" (`unanswered` takes `now`).
- **`Consent::given_by_tap(&Tap) -> Consent`** (`app/core/src/mail/oauth.rs`):
  a witness, not a permission, stronger than `is_terminal`; `Consent` stays
  `Consent(())`; a test pins both constructors' call sites.

## 2. Where acts are performed

**Not in a bundle**: `trait Perform { fn begin(&self, act, consent: Consent,
say: Announce) }` beside `Post` in `secretary/routes.rs`. Deviates from the
item's "closed list on `ServerCapabilities`": a bundle member could one day
be widened into the secretary's bundle; `Act` and the surface snapshot carry
the closed list.

`routes::tap` gains `&dyn Perform` (`state.acting`); the arm
`Proposal::Act(act) => performer.begin(act, Consent::given_by_tap(tap),
announce)` returns `Started` (or `Failed` for an account not in the roster),
and `Performed` is appended AFTER `begin` returns.

**The flow** runs on a serve-owned task, NOT the conversation worker (its
stream would say Thinking), under `expires_in + 2×interval`, ceiling 20 min.
It appends a `Turn::Act` with `code.message` verbatim, account and expiry,
then the second `Performed` and a closing `Turn::Act` ("the grant is stored"
/ "the code expired before it was used" / the transport's words); each
append calls `Workers::announce(&id)`.

**Restart mid-flow.** `device_code` is never persisted. At serve startup
`secretary::acts::reconcile(caps, now)` closes each unowned `Started` with
`Failed { "the box restarted while this act was waiting; the device code is
gone and was never written down — ask the secretary for a fresh one" }` and a
matching `Turn::Act`.

## 3. The verb

`propose_act` on `secretary::VERBS` only. Schema: `act` (required; today
`"auth"`), `account` (required), `why`, `sources` (required, `Reasons::new`).
Refuses an address `Mailbox::accounts()` does not list, naming the list;
appends `Entry::Proposed { proposal: Proposal::Act(..), reasons }`; answers
`{ proposed: <index>, act, account, why, sources, next }`.

The sentence the secretary sees: "PROPOSE AN ACT this box will perform only
if the owner taps it. It is STORED with its exact arguments and shown to him
whole; you cannot run it and there is no verb that can. `act="auth"` with
`account=<address>` asks him to authorise that Microsoft account for mail
sync — the sign-in URL and code arrive in this conversation as turns, so he
never needs a terminal on the box. Use it when a sync is failing for want of
a grant and you can point at the evidence."

**Rendering.** `wire::ProposedAct::Auth { account }`; an `auth` card in
`web/src/proposal.ts` ("Proposed act", account, why/sources, "Run" with the
two-press confirm naming the account). `web/src/refs.ts` links `https://`
ONLY when `turn.who === "act"` (via `dom.ts`, scheme checked); the node test
includes the negative (a model turn's URL stays plain).

## 4. Containment

No act without a `Tap`: `begin` takes `Consent` by value, `given_by_tap`
takes `&Tap`. Audit: tap → `Entry::Performed.by`; act → `Entry::Proposed`,
immutable; result → second `Performed` + `Turn::Act`; grant → oauth state
file. Tests: `Perform` in no bundle; one file constructs `given_by_tap`, one
implements `Perform`; loopback mints no `Tap`; taps on `Started`/`Acted`
and expired `Started` refused; with a fake performer `Performed` follows
`begin`, and a refusing performer records `Failed`, not 500; the
secretary's surface snapshot grows one verb, the agent's is byte-identical.

## 5. Secrets

The refresh token goes through `TokenStore::write_grant`, never returned.
The conversation sees only `message`, `verification_uri`, `user_code`,
`expires_in`; a test asserts no `device_code`/`Secret` in the turn. The
module header records the 2026-09-11 decision: a live user code MAY go into
the git-ignored transcript, so `secretary/` holds credential-shaped values
any export or sync must answer for.

## 6. The series

| # | Commit | Who |
|---|---|---|
| 1 | `EmailAddress`; `verbs::address` delegates | Sonnet |
| 2 | `Act`, `acts.rs`, `Proposal::Act`, `Outcome::{Started, Acted}`, refusals/expiry, store tests | Sonnet |
| 3 | **`Consent::given_by_tap(&Tap)`; oauth.rs rule 3; tests** | **main session** |
| 4 | `propose_act` + surface entry + snapshot | Sonnet |
| 5 | `ProposedAct::Auth` + `routes::proposal` + generated TS | Sonnet |
| 6 | `proposal.ts` auth card + `refs.ts` act links + node tests | Sonnet |
| 7 | **`Perform` + `routes::tap` Act arm + fake-performer tests** | **main session** |
| 8 | **`Acting` (Real + fixture Recording), `state.acting`, task, timeout, announce** | **main session** |
| 9 | `reconcile` at startup | Sonnet |
| 10 | e2e: fixture act proposal, `Recording` performer, card, two turns | Sonnet |
| 11 | ARCHITECTURE §10.3, CLAUDE.md Irreversibles + Containment, SECRETARY.md "Proposing an act", chain | **main session** |

## 7. Risks, and what NOT to do

- No `Acts`/`Perform` on `SecretaryCapabilities`, ever; grant reads go
  through a read-only capability that mints nothing.
- The tap carries the index only. Never persist the `device_code` (a
  restart is reported, never recovered). Never run the flow on the worker.
- Never widen `Act` with a shell command, a `--scope smtp` arm or anything
  free-form: the closed sum makes "performs exactly what was shown"
  checkable. Act turns: `textContent` plus an explicit anchor, never
  `rich.ts`.
- Watch: the double-`Performed` fold; e2e reaching Microsoft (the fixture
  arm is a cargo feature like `Postman::Recording`); non-Microsoft accounts
  in `Mailbox::accounts()` (propose-time check a courtesy, tap-time refusal
  the authority).
