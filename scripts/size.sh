#!/usr/bin/env bash
# The production lines a change adds or removes: what someone must read to run
# the app. Rust is `app/*/src`, each file up to its first `#[cfg(test)]`;
# TypeScript is `web/src` outside `generated/` and declaration files. Blank and
# comment lines are not counted. Informational: the merge gate prints it, so
# "this simplified the code" is a number in every report.
#
#   scripts/size.sh [<base>]   the working tree against <base>
#                              (default: its merge-base with origin/master)
set -euo pipefail

root=$(git rev-parse --show-toplevel)
base=${1:-$(git -C "$root" merge-base HEAD origin/master)}

rust() {
  local f t
  while IFS= read -r -d '' f; do
    t=$(grep -n '^#\[cfg(test)\]' "$f" | head -1 | cut -d: -f1 || true)
    if [ -n "$t" ]; then head -n $((t - 1)) "$f"; else cat "$f"; fi
  done < <(find "$1/app" -path '*/src/*' -name '*.rs' -print0) |
    { grep -cvE '^\s*$|^\s*//' || true; }
}

web() {
  find "$1/web/src" -name '*.ts' ! -name '*.d.ts' ! -path '*/generated/*' -print0 |
    xargs -0 cat | { grep -cvE '^\s*$|^\s*(//|\*|/\*)' || true; }
}

then=$(mktemp -d)
trap 'rm -rf "$then"' EXIT
git -C "$root" archive "$base" app web | tar -x -C "$then"

delta() { local d=$(($1 - $2)); [ "$d" -ge 0 ] && echo "+$d" || echo "$d"; }
r1=$(rust "$root") r0=$(rust "$then") w1=$(web "$root") w0=$(web "$then")
echo "size: rust $r1 ($(delta "$r1" "$r0")), web $w1 ($(delta "$w1" "$w0")) production lines against ${base:0:8}"
