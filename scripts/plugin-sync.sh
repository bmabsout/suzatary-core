#!/usr/bin/env bash
# Suzatary's plugin sync: every `[[plugins]]` source run into the cache tier,
# then every actor asked, what it proposes filed in the Inbox for a tap.
# Invoked by the systemd user unit suzatary-plugins.service every 15 minutes
# (nix/nixos-module.nix), and runnable by hand from any clone.
#
# WHAT RUNS IS THE RUST BINARY, `suzatary plugin sync`, out of the pure
# `.#suzatary` derivation (scripts/app-bin.sh), over the reference plugins
# `.#suzatary-plugins` builds, which a manifest's `component` names. Both are
# resolved first and ROOTED, as the binary always is: the daemon collects what
# nothing roots, and a sync must not lose its component between the build and
# the read.
#
# NO LOCK: a sync is monotone and idempotent (docs/PLUGINS-PLAN-2026-10-02.md
# §3). Two at once each learn only what the store does not already read, under
# each plugin's store's own `store.lock`; an actor's proposal is filed by its
# act, under the Inbox's own lock, so asking twice files it once. systemd
# never starts this oneshot twice.
set -euo pipefail

REPO="${SUZATARY_REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
cd "$REPO"

# The same roots directory scripts/app-bin.sh links into.
roots="${SUZATARY_ROOTS:-${XDG_STATE_HOME:-$HOME/.local/state}/suzatary/roots-dev}"
mkdir -p "$roots"

BIN="$(scripts/app-bin.sh)"
PLUGINS="$(nix build "$REPO#suzatary-plugins" -o "$roots/plugins" --print-out-paths)"

exec "$BIN/bin/suzatary" plugin sync --components "$PLUGINS" "$@"
