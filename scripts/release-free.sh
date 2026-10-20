#!/usr/bin/env bash
# THE BINARY IS NOT A FUNCTION OF ITS RELEASE. Every train ends with a release commit
# (app/release.toml and CHANGELOG.md), and a docs change moves docs/ or CLAUDE.md; if any of
# them reached `.#suzatary-bin`'s derivation, every train would compile the binary again, and
# every step keyed on it would run again. This copies HEAD's tree, evaluates the binary's
# derivation, changes those files in the copy (a new version, a changelog line, a CLAUDE.md
# line, a new page under docs/) and evaluates it again: the binary's derivation must be the
# same, and `.#suzatary`'s, the package that installs the release beside the binary, must
# move, or the release would no longer reach `--version` and `GET /api/about`.
#
#   scripts/release-free.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
git -C "$root" archive --format=tar HEAD | tar -x -C "$scratch"
system="$(nix eval --impure --raw --expr builtins.currentSystem)"
drv() { nix eval --raw "path:$scratch#packages.$system.$1.drvPath"; }

bin="$(drv suzatary-bin)"
app="$(drv suzatary)"
sed -i 's/^version = .*/version = "0.0.1"/' "$scratch/app/release.toml"
printf '\nA line no build reads.\n' | tee -a "$scratch/CHANGELOG.md" "$scratch/CLAUDE.md" >/dev/null
mkdir -p "$scratch/docs" && printf 'A page no build reads.\n' >"$scratch/docs/release-free.md"
bin_after="$(drv suzatary-bin)"
app_after="$(drv suzatary)"

if [[ "$bin" != "$bin_after" ]]; then
  echo "release-free: a release or docs change moved the binary's derivation ($bin, then $bin_after): every train would compile it again" >&2
  exit 1
fi
if [[ "$app" == "$app_after" ]]; then
  echo "release-free: a new release left .#suzatary as it was ($app): the release no longer reaches the package" >&2
  exit 1
fi
echo "release-free: the binary's derivation is $bin with either release; the package moved to $app_after"
