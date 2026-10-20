#!/usr/bin/env bash
# The name of what the quick-check job reads, at a commit: the hash of the
# git tree entries (mode, blob, path) under every path but docs/, android/
# and web/ (bar the one web file the Rust tests read), so the name moves when
# one of those bytes does and for nothing else. CI only:
# .github/workflows/quick.yml keeps the job's pass under it.
#
# An empty list of entries is refused rather than named, since every empty
# list has the same name and would pass for any commit.
#
#   scripts/ci-names.sh [<commit>]    prints rust=…
set -euo pipefail
rev=${1:-HEAD}

entries=$(git ls-tree -r "$rev" | awk -F'\t' '$2 !~ /^(docs|android)\// &&
  ($2 !~ /^web\// || $2 ~ /^web\/static\/\.well-known\//)')
[ -n "$entries" ] || { echo "ci-names.sh: no tree entries" >&2; exit 1; }
printf 'rust=%s\n' "$(printf '%s\n' "$entries" | sha256sum | cut -c1-40)"
