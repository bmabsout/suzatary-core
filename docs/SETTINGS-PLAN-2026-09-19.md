# A settings screen, and the first secret the app may write — 2026-09-19

The owner, 2026-09-19: the API keys the app might need, and account config, in
a well-thought-out settings screen. Chain `settings` (project chain
`suzatary`). The first time the web app may write a SECRET, so the rules
come first. As built: CLAUDE.md Containment, "The app may set a secret, and
may never read one back".

## 1. Why this is not a small feature

`app/cli/tests/writes.rs` pins the app's POSTs (replica objects, an authored
event, a device token, the secretary's tap and dismiss). A secret is bigger:
a mail grant reads a mailbox, a provider key spends money. Done anyway
because a second host (the 2.0 goal) has a browser and a tailnet identity,
no shell: a system only its author can configure is not hostable.

## 2. The rules, before the screen

1. **Write-only, always.** No route, verb, page or log returns a value; the
   read surface is whether it RESOLVES (what `doctor` prints).
2. **A closed list of names**: only names the instance file DECLARES
   (providers' `key`, push channels, mail slots). Anything else is refused,
   so a compromised page cannot reach the GitHub token or the Android
   signing password.
3. **One store**: writes through `secretspec`, which the binary reads.
4. **The same gate as sending**: a named tailnet peer in `web.act_as`, the
   CSRF triple, nothing loopback can mint.
5. **No verb, ever**: the capability is in no bundle, pinned by a
   tree-walking test naming the one file allowed to mention it.
6. **It is an act, and acts are visible**: each set appends a `Turn::Act` to
   the default conversation ("the owner set the key for phala"), the NAME,
   never the value.
7. **Never in a transcript, log, error or refusal**: `Secret` carries this;
   the route must not echo its input.

## 3. What the screen is actually for

| what | lives now | shows | action |
|---|---|---|---|
| Mail accounts (7) | `instance.toml` + each strategy's credential store | synced when; the credential (Microsoft's two OAuth slots are two consents; lieer and the Bridge own theirs) | the OPEN DECISION pointing at it (§3.1); none of its own |
| Model providers | `[[providers]]` | name, base URL, attested, key resolves, which job uses it now | set or replace the key |
| Notifications | `[notify]` + channel secrets | channels configured, resolving, last push | set a channel key; send a test |
| This deployment | `instance.toml` | roster, who may read/write, chains, data root, version, build | nothing: it is a file |

## 3.1 The secretary puts things here; the page invents nothing

The owner, 2026-09-20: "it does belong in settings but the secretary can put
it there." Settings is the Inbox's fold of OPEN decisions (`INBOXPRODROME`)
filtered to those whose [`Reference`](../app/core/src/reference.rs) names an
account, drawn on that row; the Inbox still shows all (context is a filter,
never a second store, `CHATSHAPE` §9). No `Act` arm is special-cased; a new
act lands on the right row, or in the Inbox alone.

`Outcome::Started` does not settle its decision (`settled_note` answers
`None`), so a running flow stays open and in the Inbox. The card renders
what the flow waits for (verification URL, user code) on every frame that
draws it; the act turns stay the record.

The deployment row is read-only: the file (CLAUDE.md) has one writer.

## 4. What it is NOT

Not a general config editor, not a read-back, not a way to add a chain,
actor or viewer (the committed instance file's), not a dashboard (Usage
answers cost; this answers what is configured and missing).

## 5. The series

| # | Commit | Who |
|---|---|---|
| 1 | core: `Settable` (closed list from the instance file, smart constructor); `wire::Settings` (names, resolves, last set, no value); `routes::settings` | Sonnet |
| 2 | cli: `POST /api/settings/secret` behind gate and triple, through secretspec, undeclared name refused by name, act turn, value never logged; `writes.rs` pins the route and no verb | Sonnet |
| 3 | web: Settings page, §3's four sections, deployment read-only, account rows draw OPEN DECISIONS naming them (§3.1) via the Inbox's fold, a started flow shows what it waits for | Sonnet |
| 4 | e2e: a set key resolves without returning; undeclared name refused; read-only login sees no controls; act turn in the chat | Sonnet |
| 5 | CLAUDE.md Containment (the write, seven rules); HOSTING "configure it from the app"; chain | **main session** |

## 6. Risks, and what not to do

- **Do not add a `get`**: the day a secret is read back a screenshot leaks
  one; "does it resolve" is the useful half.
- **Do not let the name be free text**: the closed list keeps this from
  being "write any secret on the box".
- **Do not put the signing keystore or the GitHub token in the list**:
  release capabilities, a terminal's.
- **Do not make the deployment section editable**: a second writer is the
  dual-write failure already removed once.
- **Do not skip the act turn**: a capability change must leave a trace.
