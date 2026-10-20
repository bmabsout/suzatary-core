#!/usr/bin/env bash
# Before anything lands in a PUBLIC repository (the Prodrome): the public tree's
# own guard (`public-tree.sh --check --lists`) over what every commit of the
# range adds, its messages, and the pull request's text, so both publication paths hold
# one invariant: every byte is plain text the secret scanner and the term
# matcher both read, or in a file a person admitted by its path and digest. The
# lists live only in the box-local tier, so this runs here and never in the
# public one. Prints where a hit is, never what it holds, and fails on any.
#
#   scripts/public-scan.sh [--admit <path>]... <repo> <range> [<pr>]
#   scripts/public-scan.sh ~/Documents/prodrome origin/main..land/p12 12
#
# A file the matcher cannot read as words (a binary, a run of base64 or hex, a
# quoted-printable body), or one holding a finding the tip's `.gitleaksignore`
# admits, is a hit until `--admit <path>`, given at a terminal and never from a
# script, says a person looked at every version of it the range holds; the
# admission is this scan's, by those bytes' digests.
set -Eeuo pipefail
# A scan that could not run is a 2, whatever stopped it, and only a hit is a 1:
# a caller that reads 1 as "a private term is in it" never reads a failure so.
trap 'exit 2' ERR

usage() {
  echo "usage: public-scan.sh [--admit <path>]... <repo> <range> [<pr>]" >&2
  exit 2
}
admits=()
while [[ "${1:-}" == --admit ]]; do
  (($# >= 2)) || usage
  [[ -t 0 ]] || {
    echo "public-scan: --admit $2 is a person's word that they looked at it, so it is taken at a terminal and never from a script" >&2
    exit 2
  }
  admits+=("$2")
  shift 2
done
(($# == 2 || $# == 3)) || usage
repo=$1
range=$2
pr=${3:-}

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# The lists are the point: with no instance to find them, refuse rather than
# pass a scan that read nothing.
[ -f "${SUZATARY_INSTANCE:-$root/instance.toml}" ] || {
  echo "public-scan: no instance file, so no lists to scan with" >&2
  exit 2
}
# The scanned tree is `$scan/tree`; the scan's own scratch sits beside it, so an
# early exit leaves nothing behind.
scan="$(mktemp -d)"
trap 'rm -rf "$scan"' EXIT
mkdir -p "$scan/tree"

# What the range would publish, laid out as one tree per commit (a push publishes every commit
# of it, so a term added in one commit and removed in the next is still published): each file
# the commit changes as that commit holds it, at its path in the repository, and that commit's
# `.gitleaksignore`. Its message, and the pull request's text, are trees of their own, so no
# file a commit adds stands where the scan writes its own words. The paths come NUL-separated and unquoted, deletions left out, so every path read is
# one the commit holds: a file that cannot be read is a scan that did not run (2), never a file
# skipped.
empty="$(git -C "$repo" hash-object -t tree /dev/null)"
git -C "$repo" rev-list --reverse "$range" >"$scan/commits"
n=0
while IFS= read -r commit; do
  n=$((n + 1))
  tree="$scan/files/$n-${commit:0:12}"
  mkdir -p "$tree" "$scan/text/$n-${commit:0:12}"
  parent="$(git -C "$repo" rev-parse --verify --quiet "$commit^")" || parent="$empty"
  git -C "$repo" -c core.quotePath=false diff -z --name-only --diff-filter=d "$parent" "$commit" >"$scan/paths" || {
    echo "public-scan: could not list what ${commit:0:12} changes" >&2
    exit 2
  }
  if git -C "$repo" cat-file -e "$commit:.gitleaksignore" 2>/dev/null; then
    printf '.gitleaksignore\0' >>"$scan/paths"
  fi
  while IFS= read -r -d '' path; do
    mkdir -p "$tree/$(dirname "$path")"
    git -C "$repo" show "$commit:$path" >"$tree/$path" || {
      echo "public-scan: could not read $(printf '%q' "$path") at ${commit:0:12}, so the scan did not run whole" >&2
      exit 2
    }
  done <"$scan/paths"
  git -C "$repo" log -1 --format=%B "$commit" >"$scan/text/$n-${commit:0:12}/message"
done <"$scan/commits"

if [ -n "$pr" ]; then
  slug="$(git -C "$repo" remote get-url origin | sed -E 's#.*github.com[:/]##; s#\.git$##')" || {
    echo "public-scan: could not read the repository's origin, so the pull request is unread" >&2
    exit 2
  }
  mkdir -p "$scan/text/pull-request"
  gh pr view "$pr" --repo "$slug" --json title,body --jq '.title + "\n" + .body' >"$scan/text/pull-request/pull-request" || {
    echo "public-scan: could not read pull request $pr, so the scan did not run whole" >&2
    exit 2
  }
fi

# The admissions are this scan's: each path given, by the digest of every version of it the
# range's commits hold, so a person's look covers each one a push would publish. A message or
# the pull request's text is words a person writes, never admitted: it is read as words alone.
for path in "${admits[@]}"; do
  touched=""
  for tree in "$scan"/files/*; do
    [ ! -f "$tree/$path" ] || touched=yes
  done
  [ -n "$touched" ] || {
    echo "public-scan: --admit $path names no file $range touches" >&2
    exit 2
  }
done

leaked=""
for tree in "$scan"/files/* "$scan"/text/*; do
  : >"$scan/admitted"
  for path in "${admits[@]}"; do
    [ ! -f "$tree/$path" ] ||
      printf '%s  %s\n' "$(sha256sum <"$tree/$path" | cut -d' ' -f1)" "$path" >>"$scan/admitted"
  done
  status=0
  case "$tree" in
    "$scan"/text/*) words=(--text) ;;
    *) words=(--admitted "$scan/admitted") ;;
  esac
  hits="$("$root/scripts/public-tree.sh" --check --lists "${words[@]}" "$tree")" || status=$?
  if [ "$status" -ne 0 ] && [ "$status" -ne 1 ]; then
    echo "public-scan: the leak guard could not run over $(basename "$tree") (it exited $status), so it is not clean" >&2
    exit 2
  fi
  if [ "$status" -eq 1 ]; then
    printf '%s\n' "$hits" | sed "s/^/public-scan: $(basename "$tree"): /" >&2
    leaked=yes
  fi
done
if [ -n "$leaked" ]; then
  echo "public-scan: a commit above would leak, or hold what a person must admit; nothing may land" >&2
  exit 1
fi
echo "public-scan: $(git -C "$repo" rev-list --count "$range") commit(s) clean" >&2
