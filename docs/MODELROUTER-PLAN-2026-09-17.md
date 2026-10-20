# Each job names its own model — 2026-09-17

The owner, 2026-09-17: use "some confidential computing api thing … instead of
anthropic's models", "also for the triage agents", via "a general harness …
that lets me choose one of n models but does it both cheaply and with good
perf". Then: "Okay go." Chain `modelrouter` (project chain `suzatary`).

## 1. What is true today

The box's measured spend: under $900 over the project at API list rates,
under $10 today. Overage is disabled, so cost arrives as rate-limit
refusals when a five-hour window fills, mostly from the automated side
(~600k tokens in one window, ~3M a day, forever).

Another vendor is cheap to adopt because **the model's protocol is one JSON
object, `{"say": …, "calls": [...]}`, and the verbs are values THIS side
dispatches** (`suzatary_core::secretary`, `agent::VERBS`): no vendor
tool-calling, any text-to-text endpoint drives it. The seams exist: `Model`
with `turn`, the shared prompt builder, `Reply::parse`, `choose_brain` (the
ONE place an interpreter is chosen), `Resolves` for secrets, injectable
transports (`usage::Http`, `oauth::Transport`).

## 2. The design

**One interpreter, not one per vendor.** `suzatary_io::openai` speaks
OpenAI-compatible chat completions over an injectable transport and
implements `Model`. A vendor is a BASE URL in the instance file, never a
Rust arm.

**A provider is declared, and so is its attestation:**

```toml
[[providers]]
name = "phala"
base_url = "https://inference.phala.com/v1"
key = "PHALA_API_KEY"      # the NAME of the secret, never the secret
attested = true            # a response with no receipt is a refusal
```

`attested = true` means the gateway returned a signed receipt, recorded
beside the conversation and logged. It does NOT mean this box verified the
quote against Intel's or NVIDIA's root of trust (a later commit). It makes
a silent fallback to unattested serving a refusal.

**Each job names its model.** `Job` is a closed sum (`Secretary`, `Review`,
`Triage`); `[models]` maps each to `provider/model` (e.g. `secretary =
"anthropic/claude-opus-5"`, `review = "phala/deepseek-v4-flash"`; an ORDER
since §2.2). `choose_brain(instance, job)` stays the one place. A job with no
entry behaves exactly as today, so the shipped file changes nothing until a
line is added.

> **Since 2026-09-26** (`docs/MODELCHOICE-PLAN-2026-09-26.md`): each
> backend is spelled once — `anthropic/` is the Messages API only,
> `claude-cli/` the subscription only — and a job with no usable entry
> answers with `claude-cli/<[secretary] model>`, no longer the "API if its
> key resolves, else the CLI" guess. The orders and the reserve are edited
> from `#/settings` and read by the server per turn, with no restart.

**Nothing switches on a guess.** `suzatary models differ --job review
--against <provider/model> --since <date>` replays recorded `Reviewed`
verdicts (batch, verdict, `why`) through the candidate and reports agreement
per decision, disagreements in full — the instrument the Rust port was
gated on. A model earns a job by agreeing, not by being cheap. (Superseded
by triagebench, §5 commit 3.)

**Why the review pass first**: high volume, already recorded with verdicts,
and a wrong answer stops at the Inbox. Triage is phase two: it runs as
Claude Code under the tool-guard hook, so moving it onto `Model` means the
verb loop directly over `AgentCapabilities` — BETTER containment (a bundle
with no shell instead of a hook denying tools), but its own plan.

## 2.1 A second provider, and what it is good for (2026-09-17)

Tailscale's model router (on Vercel's AI Gateway) is, like any gateway, a
BASE URL and a key name, declared beside the attested endpoint.

- **Breadth and honest billing**: hundreds of models at provider price, cost
  and usage ON EVERY REQUEST (a fact where the Usage page infers). Commit 3
  records it where a provider sends it.
- **Identity instead of keys**, by tailnet identity: nothing today (the
  agent has no shell to read a key), the answer for agents OFF the box.
- **No attestation**. So `Attestation` is a closed sum PER PROVIDER, not
  global: attested for mail-reading jobs, plain elsewhere, never confused.

**Almost every job here reads his life** (mail; batches about legal matters,
money, health): the gateway is for EVALUATION (fixture data, pricing), the attested
endpoint for real content. The §5 commit 3 differential reads real
decisions, so it runs on the provider a job would use, never a cheaper one.

## 2.2 Securely, and still using the subscription (2026-09-19)

The owner: "I do want you to figure out how to securely integrate it. I don't
want to be anthropic only … but want to take advantage of subscriptions."

**What already makes this safe, and it is not the model.** Four mechanisms
hold for any vendor: no bundle carries a trait that sends; the verbs are
values THIS side dispatches from a closed snapshotted list; anything
outward needs a `Tap`, minted only by the gate from a named tailnet peer;
prose is escaped where it enters (`escape_markup`) and a todo it writes is
provisional. A weaker model is easier to talk into nonsense; the blast
radius is unchanged. A configuration decision, not a rewrite of the fence.

**What genuinely changes: where the bytes go.** Three controls: (1) the key
is a NAME in the instance file, a value only in secretspec, set by the owner
at a terminal; (2) `attested` is declared per provider, enforced per
response; (3) a job's model is the owner's decision, and a change is recorded
on the project chain.

**Spill, do not switch.** A subscription is free at the margin until its
window fills; an API costs from the first token. So `[models]` names an
ORDER:

```toml
[models]
secretary = ["anthropic/claude-opus-5", "phala/deepseek-v4-flash"]
review    = ["anthropic/claude-sonnet-5", "phala/deepseek-v4-flash"]
triage    = ["phala/deepseek-v4-flash"]     # never worth a window
reserve   = 0.30                            # see below
```

`choose_brain` takes the first entry usable now: a subscription entry while
`usage::Allowance` is `Reported { utilization }` under `1 - reserve`;
skipped while `RateLimited` or above; `Unknown` counts as usable (refusing
work over an unreadable meter is worse than spending). **`reserve` is the
human's share**: without it the automated jobs eat the window before he
opens the app. A job that spills changes vendor, so the §5 commit 3
differential is not optional: a spill target that disagrees with recorded
verdicts does not belong in the list.

## 2.3 The subscription is not this project's (2026-09-19)

The owner: "I like to use my subscription for multiple projects not just this
one."

- **The meter is already account-wide** (`usage::Allowance` reads the
  account's windows), so other projects' use drives §2.2's spill here.
  Nothing to build.
- **`reserve` is the share of the ACCOUNT the automated jobs may never take;
  default 0.50** (0.30 was too small): a five-minute timer should never be
  why his own session is refused.
- **Automated jobs are not first in line**: `review =
  ["phala/deepseek-v4-flash", "anthropic/claude-sonnet-5"]`, `reserve =
  0.50`; secretary and triage as in §2.2. The secretary prefers the
  subscription (his conversation; quality is the product); review prefers
  the cheap provider, the subscription its fallback when no key resolves;
  triage never touches it. Only the order changes, the point of naming one.
- **Reporting gap** (`usagewholeaccount`): the Usage page counts this
  repo's two transcript directories beside the whole account's window, so
  other projects are invisible there and visible to the meter. It should
  say which it shows, or show both.

## 2.4 What this buys a second host (2026-09-19)

"It moves us closer to having it be hostable for other people." Which model
answers a job was the last hardcoded deployment fact (`choose_brain` looked
for one vendor's key, else that vendor's CLI); the owner's subscription and
setup-token do not transfer. A host now declares its own providers and
orders and differs by a file, not a patch: `docs/HOSTING.md` "Or somebody
else's model". Two obligations in this series:

- **A host with nothing configured is refused at STARTUP**, naming the job
  with no model, as a writer the gate would refuse already stops the server.
- **Attestation becomes a disclosure**: a host answers where another
  person's mail went, from the declaration and its refusal.

## 2.5 Spilling costs the secretary its session, and that is expensive (2026-09-19)

Found building §5 commit 2. A spilling brain cannot hold a `SessionModel` (a
session is a handle into ONE vendor's process), so `Sessions` answers `None`
and the secretary goes stateless: full context (the conversation is a file,
why `onechat` was safe), but no DELTA — the whole transcript every round
(153 turns yesterday), so more tokens, not fewer.

- **Now:** the secretary's order stays a single entry until `statelesswindow`
  lands, and `doctor` says when a job's order turns its session off.
- **Later:** the stateless prompt carries a WINDOW of the transcript, as the
  page does (`chatshape` §4.4). Filed as `statelesswindow`.

Triage and review hold no session and spill freely — the half about money.

## 3. What this is not

**Correction, same day** (the owner: "Don't these systems have good caching
methods?"): §2.5 counted tokens SENT; what is paid for is tokens MISSED.
Providers cache a prompt's prefix, and `prompt_of` is cache-shaped: manual,
verb list, fixed explainer, turns in order, trailer; nothing at the head
varies per round, so round N+1 shares round N's bytes up to its transcript's
end (95% of the current window's tokens are cache reads). The FILE makes a
model swappable; prefix caching makes replaying it cheap; the session was a
transport optimisation, not the memory. Still true, none a blocker:

- **A cold cache is a full-price read** (caches expire in minutes to an
  hour): `statelesswindow` bounds it — an optimisation, not a gate on
  spilling.
- **Spilling mid-conversation loses cache warmth** (each vendor caches its
  own prefix): a conversation FINISHES on the vendor it started and spills
  at a conversation boundary, not a turn — refining §2.2 for jobs holding a
  conversation.
- **Cache writes cost** (more than plain input), so the first round after a
  manual or verb-list edit pays: do not edit SECRETARY.md casually.

**Pin the property** (load-bearing, invisible: a "today is …" at the head
would silently destroy the cache). Law, in `app/io`'s tests beside the
prompt pins: two prompts built from the same conversation at different
instants are byte-identical up to where the shorter's transcript ends.

Not a swap of the main session's model (Claude's judgement is worth paying
for). Not a subagent change: Claude Code's Agent tool takes an Anthropic
model name; another vendor runs through the box's `Model` seam, not a
gateway pretending to be Anthropic.

## 4. The rule that has to be written down first

Mail bodies, chain content and the persona reach whatever model a job
names, so this rule came first; as adopted it is CLAUDE.md Containment, "A
job's model is a data decision, not a configuration detail" (the three
controls of §2.2, and an unmet `attested` claim refused, never silently
used).

## 5. The series

| # | Commit | Who |
|---|---|---|
| 1 | io: `openai.rs` — the OpenAI-compatible interpreter over an injectable transport, implementing `Model`; the attestation policy as a closed sum; scripted-transport tests including a missing receipt being refused | Sonnet |
| 2 | cli: `[[providers]]` and `[models]` in the instance file; `Job` as a closed sum; `choose_brain(instance, job)`; `doctor` prints what it read; a named provider whose secret does not resolve stops the server the way a dead writer already does | Sonnet |
| 3 | ~~cli: `suzatary models differ`~~ — SUBSUMED 2026-09-19 by `docs/TRIAGEBENCH-PLAN-2026-09-19.md` (its review-case half; a second implementation is refused). No job switches until the benchmark says it may | — |
| 4 | docs: the containment rule in CLAUDE.md, ARCHITECTURE, HOSTING (a second host needs to know how to name its own providers); the chain | **main session** |

1, 2, 3, 4 in order. Each builds and tests with no key and no network: the
transport is injected and scripted.

## 6. The one thing only the owner can do

Create the account, mint a key, and in a PLAIN TERMINAL run `secretspec set
PHALA_API_KEY`. Nothing here needs it to build or test; the differential
needs it to run.

## 7. Risks, and what not to do

- **Never switch a job on price**: the differential decides, however cheap.
- **Never let an unattested response through a provider that declared
  attestation** (§2): the declaration's whole value.
- **Never put a key in the instance file** (committed; it holds the NAME).
  `secretspec set` is the owner's, at a terminal, never an agent's.
- **Never claim the quote is verified** until a commit verifies it against a
  root of trust: recording a receipt and verifying a quote are different
  sentences. Since 2026-10-03 the TDX quote IS verified, to Intel's SGX root
  through current DCAP collateral, before a job is sent, and the report is
  fetched once per keyset (`suzatary_io::attestation::Identity`,
  `Identities`). What is still a stated claim is the source provenance: the
  `repo_commit` the owner's `expect_commit` is compared with sits beside the
  quote, and no measurement (RTMR3's event log, the compose hash) is compared
  with it yet.
- **Do not move triage in this plan**: its own plan, its own gate.
