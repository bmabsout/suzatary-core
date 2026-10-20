# A secret is asked for in the Inbox, never in a chat — 2026-09-21

Nothing pasted into a Claude session can be scrubbed from the server-side
record, so the owner (2026-09-21) asked for the agent to request secrets in the
Inbox, and for secretspec mappings to be settable from the app generally.
Chain: `secretask` (project chain `suzatary`).

## 1. What exists, and what is missing

Settings (`docs/SETTINGS-PLAN-2026-09-19.md`) has the only store and gate:
`settings::Writes` over `Secretspec::set`, `Settables` derived from the
instance file, a `Witness` from the sending gate, `POST
/api/settings/secret`; its §3.1: "the secretary puts things here; the page
invents nothing". Missing is the ask. A secret has the shape of the acts
the secretary already proposes (`Proposal::Act`, `Act::Auth { account }`):
"`PHALA_API_KEY` is declared and does not resolve; give it to me", a
write-only field, and the tap is the write. The Inbox is the general "the
box needs something from you" surface; settings lists what is set.

## 2. Rules, each a mechanism

1. **The name is a `Settable`.** `Act::Secret { name: Settable, why }`: the
   verb mints the name against `Settables` and refuses anything undeclared.
2. **The value travels once, in the tap, and is never stored.** The tap
   body's `value` goes to `settings::Writes` only — never the
   conversation, proposal, log or outcome; `Outcome::Acted { what }` names
   the secret. A secret tap without a value is refused naming the field; a
   value on any other act is refused.
3. **Same gate, same store.** The tap route requires a named peer; its
   `Witness` comes from that gate as `set_secret`'s does. One `Writes`
   serves both.
4. **The secretary is told what is missing.** `context` (or the doctor line)
   lists declared secrets that do not resolve; the secretary proposes the
   ask unprompted. A resolved secret produces no proposal.
5. **Settings stays**, its form one more caller of the same route.

## 3. The series

| # | Commit | Who |
|---|---|---|
| 1 | core: `Act::Secret`; `propose_act` takes `act="secret"`, `name`, `why` (rule 1); the tap route takes an optional value per rule 2; laws over fakes: the value never appears in `Stored`, a missing value is refused, a smuggled value is refused | Sonnet |
| 2 | cli + wire: the tap handler reads `value` and passes the Secretspec writer and `Settables` built for `set_secret`; the proposal row names its act kind so a card can draw the field; `context` reports declared-but-unresolved secrets | Sonnet |
| 3 | web: Inbox card for a secret act (name, why, password field, one button, "stored" on `Acted`); e2e over the isolated secretspec: proposal, tap stores, settings lists it as set | Sonnet |
| 4 | docs: CLAUDE.md Containment gains the rule; SECRETARY.md says when to propose; the chain | **main session** |

## 4. What not to do

- **Do not let the value near the transcript.** No verb returns it, no card
  echoes it, no log prints its length (`never expand a secret` holds for
  wire bodies too).
- **Do not widen `Settables`.** A new name is an instance-file edit, a
  changeset, a tap. The ask picks from the list.
- **Do not make the ask a todo.** A todo is a claim about the owner's life;
  the box needing input is a proposal.
