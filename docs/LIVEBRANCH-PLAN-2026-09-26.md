# The box serves a live checkout — 2026-09-26

**Superseded 2026-10-04** by `docs/USERREPO-PLAN-2026-10-01.md` stage 2: the
units run the app the user repository's lock names, from the store, and a
deploy is `scripts/deploy.sh`'s commit of that lock. Rules 4 (the wait) and 5
(separate roots) carry over unchanged; the live clone and `promote.sh` are
deleted. Kept as the record of why the wait is shaped the way it is.

Todo `livebranch` (project chain `suzatary`): "The box should serve from its
own checkout at a live branch, fast-forwarded to a master commit that passed
the gates, rather than from the tree being worked in." Unblocked by
`datasplit` (2026-09-26).

## 1. What is wrong today

- Every unit runs `cfg.repo` = the working checkout: the triage wrapper
  execs its scripts, `serve-web.sh` and the roots unit `nix build` it, dirty
  tree included (the "Git tree is dirty" line on every run).
- `suzatary-roots` fires on the working checkout's `HEAD` and
  `refs/heads/master`: a local commit deploys, gated or not.
- `app-bin.sh` and `web-dist.sh` write ONE set of links
  (`~/.local/state/suzatary/roots/{app-bin,web-dist}`) whoever runs them, so
  a CLI call from a dev checkout repoints what the units run.
- A deploy is a hand-rolled wait in the main session. On 2026-09-26 it read
  a stale `Result=success`, restarted the web onto a failing build, and the
  app was down about 12 minutes.

## 2. Rules, each a mechanism

1. **The live checkout is its own clone**, `~/.local/share/suzatary/live`,
   branch `live`, origin GitHub. Not a worktree: agent worktree cleanup
   (`git worktree remove -f -f`, `prune`) must never be able to reach it.
   `services.suzatary.repo` points at it; nothing edits it by hand.
2. **Only a pushed master commit goes live.** `scripts/promote.sh [<rev>]`
   (default `origin/master`) fetches in the live clone, refuses a rev that
   is not an ancestor of `origin/master`, and fast-forwards `live` to it.
   `--rollback <rev>` resets `live` to an earlier rev instead, and is the
   only non-fast-forward move.
3. **The roots unit fires on `live`, not on master.** The path unit watches
   the live clone's `refs/heads/live`. Committing or pushing in the dev
   checkout deploys nothing.
4. **Promote owns the whole deploy.** After moving `live` it waits for a
   `suzatary-roots` run whose `ExecMainStartTimestamp` is at or after the
   move, and restarts `suzatary-web` only when that run succeeded. Then it
   waits for `/api/about` to answer with the promoted build. It exits
   non-zero on any failure and never restarts onto an unbuilt tree.
5. **Units and dev use separate roots.** `app-bin.sh` and `web-dist.sh`
   link into `$SUZATARY_ROOTS`, which the module sets for every unit
   (`~/.local/state/suzatary/roots`). Without it, as in a dev shell, they
   link into `…/roots-dev`, so a dev build cannot repoint the units. The
   rooted dev profile (`roots/triage-profile`) stays where it is.
6. **The data instance names the live clone** (`repo =` in
   the data repository's `instance.toml`): the templates, the secretary's settings
   and the served dist all come from what is live. The dev checkout's
   `instance.toml` link is unchanged, so a CLI call from dev reads the live
   deployment's data and code roots.
7. **A dirty live tree is an alarm again.** The trust gate's "modified
   tracked files" warning now fires only when the live tree changed without a
   promote, which should never happen.

## 3. The series

| # | Commit | Who |
|---|---|---|
| 1 | scripts: `app-bin.sh`/`web-dist.sh` link into `$SUZATARY_ROOTS`, default `roots-dev`; `root-serving.sh` likewise | agent |
| 2 | nix: the module sets `SUZATARY_ROOTS`; the path unit watches `${cfg.repo}/.git/refs/heads/live` and `HEAD`; `services.nix` wrappers unchanged | agent |
| 3 | scripts: `promote.sh` (rules 2 and 4) with `--rollback`; `guards.rs` pins that it gates on the run's start time and on `Result=success` | agent |
| 4 | docs: CLAUDE.md Operations ("deploy = promote.sh", the two roots dirs), HOSTING §8, this plan | agent |
| 5 | box: clone the live checkout, point the data instance and `services.suzatary.repo` and the briefing unit at it, rebuild, promote the current master | main session |

## 4. What not to do

- No worktree for live (rule 1).
- No deploy from a dev commit (rule 3).
- No restart without a fresh successful roots run (rule 4).
- No shared root links between dev and units (rule 5).
