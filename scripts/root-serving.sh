#!/usr/bin/env bash
# ONE SPELLING OF "ROOT WHAT I AM ABOUT TO RUN". scripts/serve-web.sh and
# scripts/mail-idle.sh each resolve a
# store path (scripts/app-bin.sh / scripts/web-dist.sh) and then run it for a
# long time; the `app-bin`/`web-dist` links those scripts resolve THROUGH move
# whenever the roots refresh (`suzatary-roots.service`, on every commit), and
# the box's daemon collects whatever a link no longer names once the store is
# tight. A running process survives that collection — its own binary stays
# mapped — but anything it opens LATER from that path does not: a subprocess,
# a static file, a resource: `serve-web` would answer `/api/about` from the
# binary in memory while every static file 404s from a collected `web-dist`.
#
# So a long-running caller registers its OWN root for the exact path it is
# about to serve, named for its own unit and replaced only on its next
# start — a refresh behind it then moves the SHARED link and collects
# nothing the caller is reading.
#
#   scripts/root-serving.sh <root-name> <store-path>
set -euo pipefail

if [[ "$#" -ne 2 ]]; then
  echo "usage: scripts/root-serving.sh <root-name> <store-path>" >&2
  exit 64
fi

name="$1"
path="$2"

# Beside the links it protects: SUZATARY_ROOTS, `roots-dev` in a dev shell
# (scripts/app-bin.sh).
roots="${SUZATARY_ROOTS:-${XDG_STATE_HOME:-$HOME/.local/state}/suzatary/roots-dev}"
mkdir -p "$roots"
nix-store --add-root "$roots/$name" --indirect -r "$path" >/dev/null
