#!/usr/bin/env bash
# THE MERGE GATE'S CODESPACE, MADE: the machine `scripts/train.sh gate --remote <name>` runs the
# tree half in (.devcontainer/devcontainer.json says what it holds). What a Codespace is made
# with stays with it, so it is spelled here once:
#   machine       4 cores, 16 GB, 32 GB of disk (standardLinux32gb), the size the gate's
#                 budget names (the gate-warm-fast report)
#   idle timeout  10 minutes: it stops itself that long after its last session, the backstop
#                 for a train that could not stop it (the account's default was 30, then 240)
#   retention     30 days stopped before it is deleted, so its warm target and store outlive
#                 a quiet fortnight
# It prints the new Codespace's name, for `train.sh gate --remote`. Run it at a terminal, as
# the operator: it makes a machine that is billed while it runs.
#
# It makes one only in a repository the logged-in account owns (an organisation's machines and
# billing are never this script's), and only where no gate Codespace is: one machine holds the
# warm target, and a second would be billed beside it.
#
#   scripts/gate-codespace.sh [<repo>]   (default: this checkout's origin)
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
name=suzatary-gate
repo="${1:-$(gh repo view --json nameWithOwner -q .nameWithOwner)}"
login="$(gh api user -q .login)"
owner="$(gh repo view "$repo" --json owner -q .owner.login)"
if [[ "$owner" != "$login" ]]; then
  echo "gate-codespace.sh: $repo belongs to $owner, not to $login, the account logged in: a gate's Codespace is made only in the account's own repository" >&2
  exit 1
fi
existing="$(gh codespace list -R "$repo" --json displayName,name -q ".[] | select(.displayName == \"$name\") | .name")"
if [[ -n "$existing" ]]; then
  echo "gate-codespace.sh: $repo has a gate Codespace already: $existing" >&2
  echo "  use it (scripts/train.sh gate --remote $existing), or delete it first (gh codespace delete -c $existing)" >&2
  exit 1
fi
gh codespace create --repo "$repo" --branch master \
  --devcontainer-path .devcontainer/devcontainer.json \
  --machine standardLinux32gb --idle-timeout 10m --retention-period 720h \
  --display-name "$name"
