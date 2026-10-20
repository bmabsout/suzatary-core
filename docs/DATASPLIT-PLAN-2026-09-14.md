# `datasplit` — implementation plan (2026-09-14)

Chain item `datasplit` (the owner, 2026-09-13: separate the data from the repo
so part can be public, not yet; maybe before 2.0). Opus plan against
`66b9473`, accepted. `docs/HOSTING.md`'s open list names the inventory.

## Verified facts this plan rests on

- `Instance::under` resolves every path under one root; `Instance::locate`
  (flag → `SUZATARY_INSTANCE` → cwd) is already the pointer.
- `suzatary-io`/`suzatary-core` need no change: io takes paths; every
  `repo.join(` in app/io and app/cli names a CODE path.
- `appSrc` names `assetlinks.json` (code), `instance.toml`, `mbsyncrc`
  (data); `guards.rs` forbids the box's home in tracked `.claude/`,
  `scripts/`, `nix/`, so the data path arrives as environment.
- `daily/`, `build/`, `web/dist/`, `/secretary/` are gitignored; `notes/` is
  read by no code; worktrees duplicate chain stores.

## 1. Layout and the pointer

Data repo `<data>` (private): `instance.toml`, `events/`,
`events-triage/`, `events-suzatary/`, `events-triage-suzatary/`,
`state.json`, `briefings/`, `notes/`, `mbsyncrc`, and a
`.gitignore` of `/secretary/` (box-local conversations).

The pointer is one variable: `SUZATARY_INSTANCE=<data>/instance.toml`.

Two roots, two types, in `app/cli/src/instance.rs`: `Roots { code:
CodeRoot(PathBuf), data: DataRoot(PathBuf) }`; `Instance.repo` becomes
`Instance.roots` with `code()`/`data()`, `under` splits so a new path must
declare its side (ARCHITECTURE §10.7). `repo` stops defaulting to `"."`;
new `data` defaults to `"."`, so each step is a no-op until files move.

Which root each key takes: DataRoot — `chains[].events`/`.proposals`,
`mail.state`, `web.serving.briefings`, `secretary.conversations`,
`mail.mbsyncrc`. CodeRoot — `secretary.settings`, `web.serving.dist`.
Absolute paths (`tls_dir`, `mail.root`, `last_sync`) stay on DataRoot so a
second host's relative spelling follows its data.

## 2. Each writer after the split

As built: CLAUDE.md Operations, "The instance file has two roots".

- **`scripts/safe-push.sh`**: agent branch `git -C "$DATA"` (from
  `SUZATARY_INSTANCE`), `ALLOWED_PATTERN` widened, append-only check and
  `suzatary verify` kept; `--code` drops `notes/`, `instance.toml`,
  `mbsyncrc`, adds `instance.sample.toml`; one `flock`; asserts the instance
  exists before anything (no silent skip of live tests). Data repo's
  `.git/config`: credential helper by absolute path, no `core.hooksPath`.
- **`scripts/hourly-triage.sh`**: `cd "$REPO"` stays; exports
  `SUZATARY_INSTANCE`. `.claude-memory` stays code-side.
- **`nix/nixos-module.nix`**: `dataDir`;
  `environment.SUZATARY_INSTANCE = "${cfg.dataDir}/instance.toml"`.
- **`nix/services.nix`**: drop the poisoning detector's dead exclusions; a
  second detector over the data directory.
- **The vault** unchanged; the data remote is a backup (say so in HOSTING).
  `daily/`, `web/dist` stay code-side.

## 3. Tests

| test | after |
|---|---|
| `tests/live.rs`, `tests/perf_fold_chain.rs` | data root via `Instance::locate`; `eprintln!` skip when absent; `app/conformance/` stays a hard failure |
| `tests/accounts.rs`, `tests/decoration.rs` | `Instance::locate`; skip when absent |
| `tests/guards.rs` | plus two assertions over safe-push.sh's text |
| fixture chains | gain an absolute `data` |

CI: `live`/`perf` skip; commit `instance.sample.toml` (what `suzatary init`
emits, pinned by `tests/init.rs`); `appSrc` swaps `instance.toml`/`mbsyncrc`
for it. The mbsyncrc↔`inbox_folders` pin becomes a box test.

## 4. History

Fresh data repo, ONE commit: the chain's history is its content-addressed
objects, not git, and a filter split would rewrite every hash for nothing.
The code repo keeps its history. A public repo is a FOLLOW-UP: a fresh clone
of the post-split tree.

## 5. Migration on this box, each step reversible

Timers stopped (`suzatary-triage.timer`, `suzatary-mail-idle`, `suzatary-web`):

1. Data repo from `git archive HEAD -- <data paths>` + untracked live files
   (`events*/.lock`, `REJECTED`, `secretary/`), one commit, private remote
   (manual, personal account). *Rollback:* delete it.
2. Moved `instance.toml`: `repo = "<home>/Documents/Suzatary"`,
   `data = "."`. *Rollback:* two lines.
3. Both trees present: `todos open`, `verify`, `doctor`, chain tips
   byte-equal under old and new instance.
4. Units get `dataDir`; rebuild, restart, one green triage into the data
   repo. *Rollback:* revert the nixos commit.
5. ONE commit removing the data paths with the `.gitignore`, `flake.nix`,
   `safe-push.sh` edits. *Rollback:* `git revert` (stale evidence, not a
   second live chain).
6. `nix build .#suzatary`, `scripts/verify.sh`, the app, a brief page, one
   timer-driven triage.

## 6. The project chain

Goes with the data (the owner's default). The `.#triage` shell's `shellHook`
resolves `SUZATARY_INSTANCE` from the checkout like `nix/services.nix`
resolves `SUZATARY_REPO`, failing loudly: a duplicate chain is impossible.

## 7. The series

| # | commit | who |
|---|---|---|
| 1 | `Roots`/`CodeRoot`/`DataRoot`, `data` key, `under` split, `code()`/`data()` call sites; no path changes root | Sonnet |
| 2 | re-root data paths; test `repo=/a, data=/b` puts `events` under `/b`, `web/dist` under `/a` | Sonnet |
| 3 | `instance.sample.toml` + `appSrc` swap + `tests/init.rs` pin | Sonnet |
| 4 | `live.rs`, `perf_fold_chain.rs`, `accounts.rs`, `decoration.rs` instance-located, skip-when-absent | Sonnet |
| 5 | `safe-push.sh` + `hourly-triage.sh` + guards.rs assertions | main session (containment) |
| 6 | `nixos-module.nix` `dataDir`; `services.nix` detector | main session |
| 7 | HOSTING §5, ARCHITECTURE §10, CLAUDE.md Operations, CHANGELOG, README | Sonnet |
| 8 | migration commit (`git rm` data paths, `.gitignore`) after §5 steps 1–4 | main session |

## 8. Risks, each with its check

1. Code-repo path assumptions: a guard test that no tracked file under
   `app/cli/src`, `scripts/`, `nix/` has `"events` outside the parser.
2. `.claude-memory`: code side. 3. A missed `appSrc` swap fails `nix build`
   loudly. 4. Android/web: no filesystem path.
5. Silent live-test skip: safe-push asserts the instance. 6. Two repos, one
   flock. 7. Credential helper: absolute path in the data `.git/config`.

## 9. Phase 2: the code tree carries nothing personal (2026-09-26)

The owner, 2026-09-26: "make the steps needed so that we can separate out the
personal data". §5 moves the STORES; a public code tree also needs the
personal CONTENT out of the files that stay. Inventory at `db6f3e26`:

- ~~Moved by §5, missing from its list: the root `*.typ` files and
  `FETCHER`.~~ Wrong, checked 2026-09-26: the root `*.typ` files are
  TEMPLATES (no `#todo(` in any), read by `suzatary_io::build`, and
  `FETCHER` is read by `commands::up`; both are code and stay.
- **Personal content in code-side files** (96 tracked files match names,
  counsel, visa or family terms): fixtures and tests
  (`commands/fixture.rs`, `web/e2e/*`, `web/test/*`, `tests/common`,
  `core/tests/web_vectors.rs`), `app/conformance/live.json` (a live-chain
  snapshot), and the docs' verbatim quotes of the owner.
- **Box facts in code-side files:** CLAUDE.md's Operations section, the
  hostname in scripts, `secretspec.toml`'s names (names only, fine).

Status 2026-09-27 (the `scrub-2026-09-26` report, merged into
master with the recurring-price and livebranch series): 9.2 done
(`suzatary_core::cast`; master's newer tests draw on it too) and 9.3 done
for personal detail. The fold pins are RE-HOMED: `live.json` and
`view-live.json` are committed in the data repository at
`<data>/conformance/`, and the five tests that read them are back —
`app/cli/tests/live.rs`'s four read `instance.data()`'s `conformance/`,
`web/test/wasm.test.ts`'s one reads `$SUZATARY_LIVE_VIEW`, which
`scripts/web-test.sh` exports from the data tree; both skip with a line
where there is no instance. CLAUDE.md's box section has not moved to
`<data>/OPERATIONS.md`. 9.1 (the guard) is now built and green
(`app/cli/tests/guards.rs`'s `personal_terms_no_tracked_file_names_a_person`,
2026-09-27): it also caught personal content the earlier scrub had missed —
a test actor, a stand-in name reused across dismissal tests, and three
data-folder mentions in `.gitignore`, this plan and `safe-push.sh` — all
fixed in the same series.

The steps, each a commit:

| # | step | mechanism |
|---|---|---|
| 9.1 | A guard, not a sweep: `tests/guards.rs` reads `<data>/personal-terms.txt` (in the DATA repo, so the list itself is never public) and fails when any tracked code file contains a term. Skips with a line when the data root is absent (CI) — DONE 2026-09-27 | test |
| 9.2 | Fixtures become synthetic: one `fixture/people.rs`-style source of made-up names, todos and mail used by `fixture.rs`, e2e and unit tests; `live.json` conformance moves to the data repo (a box test, like `live.rs`) | agent |
| 9.3 | Docs: plans quote "the owner", not a name, and keep no immigration, family or counsel detail; CLAUDE.md's box section moves to `<data>/OPERATIONS.md`, and CLAUDE.md points at it | agent, reviewed |
| 9.5 | Public repo = a fresh clone of the post-split tree, ONE commit, only after 9.1 is green on the box and the owner says so | the owner |

## 10. Runbook for §5 on this box — DONE 2026-09-26

Run with the owner's yes. As run: the copy was of the LIVE working tree (uncommitted
objects included), not `git archive`; both roots absolute in the moved instance;
`instance.toml` in the checkout is an ignored link; the old copies are in
`~/.local/state/suzatary/datasplit-backup-2026-09-26` until a week of green
triage says they can go. Code `0988b85d`, data `b0ae0a2`, nixos config beside.

1. `systemctl --user stop suzatary-triage.timer suzatary-mail-idle.service suzatary-web.service`.
2. `mkdir <data> && git -C ~/Documents/Suzatary archive HEAD -- <§1 paths + §9 renders> | tar -x -C <data>`; copy untracked live files (`events*/.lock`, `REJECTED`, `secretary/`); `git init`, one commit.
3. A private remote for the data repository (personal account, private), push.
4. Edit the moved `instance.toml`: `repo = "<home>/Documents/Suzatary"`, `data = "."`.
5. Check both trees agree: `SUZATARY_INSTANCE=<data>/instance.toml suzatary todos open`, `verify`, `doctor`; chain tips byte-equal to the old tree's.
6. NixOS: `services.suzatary.dataDir = <data>`; rebuild; start the units; one green triage writing into the data repo.
7. Code repo: ONE commit `git rm -r --cached` of the data paths + `.gitignore`, `flake.nix` `appSrc`, `safe-push.sh`. Rollback: `git revert`.
8. `nix build .#suzatary`, the app, a briefing, one timer triage.
