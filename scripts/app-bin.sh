#!/usr/bin/env bash
# THE DEPLOYED SUZATARY IS THE PURE `.#suzatary` DERIVATION, never
# scripts/app-build.sh's dev binary (docs/ARCHITECTURE.md §2, §8.2): that one
# skips `doCheck`, is never part of `nix flake check`, and lives in a mutable
# app/target/ this script must not depend on. This is the ONE place that
# rule lives — every script and `.claude/mcp-suzatary.json` that must run the
# real program point here rather than repeating the `nix build` invocation.
#
# `nix build --no-link` leaves nothing in the working tree, and costs nothing
# on a tree Nix has already built for this exact source (a store hash check,
# not a rebuild) — which is what makes "build on demand, cached by tree" true
# rather than aspirational.
#
#   scripts/app-bin.sh            print the store path (compose your own
#                                 "$(scripts/app-bin.sh)/bin/suzatary <verb>")
#   scripts/app-bin.sh <verb...>  resolve and exec suzatary with these args
#   SUZATARY_APP_REV=<sha> scripts/app-bin.sh ...   the same, built from that commit
set -euo pipefail

repo="${SUZATARY_REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
# ROOTED, NOT --no-link: the box's daemon collects on its own whenever the
# store drops under 3 GB free, and an output nothing roots can be deleted
# between this build and the exec below. The out-link under the box's state
# directory is a GC root; a rebuild simply moves it.
#
# WHOSE LINKS: the NixOS module
# sets SUZATARY_ROOTS for every unit, so the units' links are theirs alone.
# Unset, as in a dev shell, this links into `roots-dev`, so a CLI call from a
# dev checkout cannot repoint what the units run. scripts/web-dist.sh and
# scripts/root-serving.sh spell it the same way.
roots="${SUZATARY_ROOTS:-${XDG_STATE_HOME:-$HOME/.local/state}/suzatary/roots-dev}"
mkdir -p "$roots"

# ONE LINE TO STDERR SAYING WHAT HAPPENED: a pass's log otherwise says
# nothing about whether this call cost a
# hash check or a rebuild. Asking nix first (`nix eval .outPath`) would be a
# second evaluation, measured at 3.6 s on the box — four times a pass, to
# print a word — so the answer is read off the ROOT instead: if the link
# names the same store path after the build as before, the store already
# held it. Stdout is untouched: callers still parse exactly the store path.
start="$(date +%s.%N)"
# A NAMED COMMIT, NOT THE CHECKOUT: with SUZATARY_APP_REV set, the build is of that commit's
# PUBLIC tree (scripts/public-tree.sh --store), linked apart from the checkout's, so what a
# caller runs is a function of a commit, never of a working tree. The public tree is the store
# path a deploy of that commit fetches and the merge gate's deploy-build builds and roots, so
# `train.sh land` resolves the binary the gate already built for the train's tree.
flake="$repo" link="$roots/app-bin"
if [[ -n "${SUZATARY_APP_REV:-}" ]]; then
  flake="$("$repo/scripts/public-tree.sh" --store "$SUZATARY_APP_REV")"
  link="$roots/app-bin-rev"
fi
before="$(readlink -f "$link" 2>/dev/null || true)"
out="$(nix build "$flake#suzatary" -o "$link" --print-out-paths)"
elapsed="$(awk -v s="$start" -v e="$(date +%s.%N)" 'BEGIN { printf "%.1f", e - s }')"
if [[ "$before" == "$out" ]]; then status="resolved"; else status="built"; fi
printf 'app-bin: %s in %ss\n' "$status" "$elapsed" >&2

if [[ "$#" -eq 0 ]]; then
  printf '%s\n' "$out"
else
  exec "$out/bin/suzatary" "$@"
fi
