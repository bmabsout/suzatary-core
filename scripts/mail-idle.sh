#!/usr/bin/env bash
# Suzatary's IMAP IDLE watcher. Invoked by the systemd user unit
# suzatary-mail-idle.service (nix/nixos-module.nix) and by `nix run
# .#mailIdle` (nix/services.nix, which supplies PATH, resolves
# $SUZATARY_REPO and gates on a reachable substituter).
#
# WHAT RUNS IS THE RUST BINARY: `suzatary mail idle`, out of the pure
# `.#suzatary` derivation (scripts/app-bin.sh resolves it, cached by tree —
# same rule scripts/serve-web.sh follows for `suzatary serve`).
#
# THIS VERB NEVER SYNCS. It opens one read-only IMAP connection per
# Microsoft account, IDLEs, and on a debounced wake starts the EXISTING
# suzatary-triage.service unit (`systemctl --user start`, which systemd
# itself refuses to start twice while one run is already active) — so there
# remains exactly ONE pipeline. Gmail (lieer) and Proton (the Bridge) are not
# watched here — the verb says so on start — and the 5-minute timer stays
# their only path, and everyone's fallback if this unit is ever down.
set -euo pipefail

# SUZATARY_REPO is set by the systemd unit (nix/nixos-module.nix, the ONE
# place this box says where it is); the fallback resolves this script's own
# checkout, so it is runnable by hand from any clone rather than assuming
# this one's (ROADMAP.md, "a second host").
REPO="${SUZATARY_REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
cd "$REPO"

# RESOLVED FIRST, EXEC'D SECOND — never `scripts/app-bin.sh mail idle …`,
# which would resolve and exec in one call with no path in between to root.
BIN="$(scripts/app-bin.sh)"

# WHAT THIS PROCESS RUNS STAYS ROOTED WHILE IT RUNS — the same guarantee
# `scripts/serve-web.sh` registers for the web app, through the same
# `scripts/root-serving.sh` helper. The `app-bin` link under the state
# directory moves whenever the roots refresh (`suzatary-roots.service`, on
# every commit), and the daemon collects whatever it no longer names once
# the store is tight; this long-running IDLE connection would keep running
# on the binary already in memory, but a debounced wake that shells back out
# through it, or a restart after a collection, would resolve and build again
# on the blocking path. So this
# script registers its OWN root for the exact binary it is about to exec,
# named for this unit and replaced only on its next start.
scripts/root-serving.sh app-bin-idle "$BIN"

exec nix develop "$REPO#triage" --command \
  "$BIN/bin/suzatary" mail idle --instance "$SUZATARY_INSTANCE"
