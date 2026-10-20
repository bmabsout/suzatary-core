#!/usr/bin/env bash
# THE APP'S BUILDS: everything a unit's start needs from an app tree, built
# and rooted under one prefix. The roots run (nix/services.nix) and
# scripts/deploy.sh both call it, so "what the roots run builds" is spelled
# once: deploy.sh builds under `deploy-` before it moves the lock, the run
# after the move builds the same derivations under `lock-` and finds them in
# the store.
#
#   scripts/app-builds.sh <app> <prefix>
#
# <app> is the tree's store path, <prefix> a path the link names extend:
# <prefix>src (the tree itself, rooted before anything is built from it),
# <prefix>bin, <prefix>dist, <prefix>plugins, and <prefix>shell, the
# operator's shell as a profile so its packages stay rooted, keeping its
# current generation only.
set -euo pipefail

if [[ "$#" -ne 2 ]]; then
  echo "usage: scripts/app-builds.sh <app> <prefix>" >&2
  exit 64
fi
app="$1"
at="$2"

mkdir -p "$(dirname "$at")"
nix-store --add-root "${at}src" --indirect -r "$app" >/dev/null
# A tree without the lock interface predates deploys by lock: its scripts
# write under their own read-only code root.
if [[ ! -e "$app/nix/app-input.nix" ]]; then
  echo "app-builds: $app predates deploys by lock: its scripts write under their own code root. Nothing built." >&2
  exit 1
fi
system="$(nix eval --impure --raw --expr builtins.currentSystem)"
nix build "$app#suzatary" -o "${at}bin" --print-out-paths
nix build "$app#suzatary-web" -o "${at}dist" --print-out-paths
nix build "$app#suzatary-plugins" -o "${at}plugins" --print-out-paths
nix develop "$app#devShells.$system.triage" --profile "${at}shell" --command true
# A profile grows a generation per build and roots every one; keep the current.
nix profile wipe-history --profile "${at}shell"
