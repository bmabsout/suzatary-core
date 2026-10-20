# Build when the tree moves, not when the box is asked — 2026-09-21

Chain: `lazybuildstall` (project chain `suzatary`). Builds stalled exactly
when the box was used: a web restart was offline ~6 min building the bundle
before binding; a triage pass blocked 15 min in `app-bin.sh`'s `nix build`
after a commit — both on two cores against an agent's cargo build.

## 1. The guarantee is right; the moment is wrong

`scripts/app-bin.sh` and `scripts/web-dist.sh` resolve the pure derivations
and root them, cached by tree, so a stale binary cannot be served. That
stays. The fault: resolution happens at first use after a change, by
whichever caller comes first (`hourly-triage.sh`, four calls per pass;
`serve-web.sh`, both roots before binding; `web-e2e.sh`;
`.claude/mcp-suzatary.json`). Nothing builds ahead.

Not CI: the box is `aarch64-linux`, runners are x86_64 (the Codespace
builder takes the APK and checks, never the ARM binary), and the box's
only substituter is `cache.nixos.org`. CI-as-delivery needs an aarch64 builder
and a private signed cache — the 2.0 second host's problem.

## 2. The fix: a refresh that runs when HEAD moves

`suzatary-roots`: a user path unit on `.git/HEAD` and the ref it names,
whose service runs `app-bin.sh` and `web-dist.sh` and nothing else. After a
commit the next caller's resolution is a store-hash check. Safe because:

- **The callers keep resolving**; `app-bin.sh` at use time is the
  authority. Mid-refresh callers wait on the same build (the daemon
  serialises identical builds): worst case is today, never stale. `-o`
  roots move only on success.
- **No new definition of "current"**: the refresh runs the scripts that
  define it (never `nix build` itself); a second one would be dual-write.
- **Nothing serves before its build**: `serve-web.sh` resolves before
  binding.
- **Refs, not the working tree**: a dirty tree is not a version, so a
  half-written worktree never builds the deployed binary.

## 3. What it costs, stated

One background build per commit (~30 on a busy day), mostly no-ops for the
binary. `Nice=10` and `IOSchedulingClass=idle`, so a refresh never starves a
pass (that would move the stall, not remove it). No new disk.

## 4. The series

| # | Commit | Who |
|---|---|---|
| 1 | nix: `suzatary-roots.path` + `.service` in `nix/nixos-module.nix`, watching `HEAD` and `refs/heads/master`, running `app-bin.sh` then `web-dist.sh` under `Nice=10`; `Type=oneshot`, no self-overlap, logs both store paths (the journal line is the evidence) | Sonnet |
| 2 | scripts: `app-bin.sh`/`web-dist.sh` print one line, built or resolved ("resolved in 0.4 s"); `guards.rs` pins the unit invokes exactly those two scripts | Sonnet |
| 3 | docs: CLAUDE.md Operations gains "the roots refresh when HEAD moves; use-time resolution is a hash check"; the chain | **main session** |

## 5. Risks, and what not to do

Each rule is in §2–§3: the unit calls the two scripts, never `nix build`;
callers never trust a root without resolving; watch refs, not the tree;
keep the nice level.
