#!/usr/bin/env bash
# THE DEPLOYED FRONTEND IS THE PURE `.#suzatary-web` DERIVATION, never
# `web/dist` in the dev checkout: that directory is gitignored and any dev
# command overwrites it in place, so the box would serve a new bundle against
# an older server. This
# is the ONE place that rule lives for the frontend, exactly as
# scripts/app-bin.sh is for the binary — every script that must serve the
# real bundle points here rather than repeating the `nix build` invocation.
#
# ROOTED, NOT --no-link, for the same reason app-bin.sh is:
# the box's daemon collects on its own whenever the store drops under 3 GB
# free, and an output nothing roots can be deleted between this build and
# whatever reads the printed path next. The out-link under the box's state
# directory is a GC root; a rebuild simply moves it.
#
#   scripts/web-dist.sh   print the store path of the built frontend
set -euo pipefail

repo="${SUZATARY_REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
# The units' links under SUZATARY_ROOTS, `roots-dev` in a dev shell: see
# scripts/app-bin.sh.
roots="${SUZATARY_ROOTS:-${XDG_STATE_HOME:-$HOME/.local/state}/suzatary/roots-dev}"
mkdir -p "$roots"

# ONE LINE TO STDERR SAYING WHAT HAPPENED — see scripts/app-bin.sh's comment,
# the same mechanism for the frontend's derivation: read off the root, never
# a second evaluation. Stdout is untouched.
before="$(readlink -f "$roots/web-dist" 2>/dev/null || true)"
start="$(date +%s.%N)"
out="$(nix build "$repo#suzatary-web" -o "$roots/web-dist" --print-out-paths)"
elapsed="$(awk -v s="$start" -v e="$(date +%s.%N)" 'BEGIN { printf "%.1f", e - s }')"
if [[ "$before" == "$out" ]]; then status="resolved"; else status="built"; fi
printf 'web-dist: %s in %ss\n' "$status" "$elapsed" >&2

printf '%s\n' "$out"
