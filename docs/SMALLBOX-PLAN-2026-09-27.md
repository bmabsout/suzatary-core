# A small box: mail, secrets and the app, nothing else — 2026-09-27

The owner, 2026-09-27, weighing a boxless design: "our search is actually
quite fast, and we could have a notmuch server in our app as well which would
allow us to locally store the emails in case offline, so I don't want to get
rid of it … Maybe it is optimal to retain a small box", and "we can always
update the code on a claude cloud instance as well".

## 1. What the box is for

| Keep on the box | Why it needs a box |
|---|---|
| Proton Bridge, IMAP sync and IDLE, the Gmail sync | long-running daemons holding logins; reacting within seconds |
| notmuch (55k messages, 9.1 GB plus a 1.3 GB index) | fast search, and the source of truth for mail state: read, unread and triage's tags |
| secretspec | mail and push credentials never leave the owner's machine |
| `suzatary serve` | the app, the login gate and push, on the tailnet |
| the triage timer | reads the local corpus; writes a batch |

| Take off the box | Where it goes |
|---|---|
| writing and changing code | cloud sessions and routines (`codingsession`, and the main session's cloud agents) |
| building (cargo, the verify target, Nix builds of the app and bundle) | CI or a builder, pushing to a Nix binary cache |
| the chain's only copy | git: the data repository, and GitHub as the hub once heads are derived (MULTIPRODROME-PLAN) |

Every disk crunch and outage in September came from the second table: the
shared Rust target (4 to 6 GB), the verify cache (3 to 5 GB), Nix builds on
each promote, and agent worktrees. The mail side grows slowly and has never
caused one.

## 2. The rules

1. **The box never compiles.** `promote.sh` moves `live` to a commit whose
   closure is already in the binary cache, and switches to it. A commit with
   no cached closure is refused by name, never built here.
   - The builder is aarch64: GitHub's arm64 runners, or a remote builder of
     that architecture. The existing x86 codespace builder does not fit.
   - The cache is Cachix, or a self-hosted Attic.
   - CI minutes return on 1 October.
2. **Code changes happen in the cloud.**
   - The main session briefs cloud sessions by default, not local worktree
     agents.
   - A routine fired from the app is the phone's path (`codingsession`).
   - The box's checkout is the live clone, and nothing else.
3. **Verification runs where the build does.**
   - `verify.sh` (the suite, live and guards) moves to CI.
   - The box keeps only the cheap live checks against its own data: the
     chain verifies, the instance parses, and the personal-terms guard. These
     read the data and need no compiler.
4. **Mail gains an offline half.**
   - The app caches recent and relevant threads on the phone, fetched
     through the box's search API.
   - Read and unread and tags sync back as notmuch tags, so notmuch stays the
     one truth.
   - Offline, the phone reads what it holds and queues tag changes, the same
     replica pattern as the chain.

## 3. The series

| # | What | Where |
|---|---|---|
| 1 | CI builds `.#suzatary`, `.#suzatary-web` and `.#suzatary-junk-seed` for aarch64 on each master push and pushes them to the cache; the box trusts the cache's key | CI, nixos |
| 2 | `promote.sh` refuses a rev with no cached closure, and substitutes instead of building; the roots unit only realises | scripts |
| 3 | `verify.sh` in CI; triage's push path keeps only the data checks | scripts, CI |
| 4 | remove the dev toolchain from the box's routine use: the shared target and verify cache are gone, and agents default to cloud | box, memory |
| 5 | the chain's hub on GitHub (MULTIPRODROME §7 rows 1 to 4) | Prodrome, Suzatary |
| 6 | offline mail: a thread cache on the phone and tag sync as a replica | core, web |

Row 1 needs CI minutes, which return on 1 October, and an arm64 runner or
builder. Rows 2 to 4 follow from it. Rows 5 and 6 are independent.
