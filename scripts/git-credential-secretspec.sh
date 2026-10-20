#!/usr/bin/env bash
# Git credential helper backed by secretspec.
#
# The data repository's push token (SUZATARY_DATA_PUSH_TOKEN, scoped to that
# one repository: the secretary's repositories have a token of their own) lives
# in `pass` (GPG-encrypted at rest) and is resolved on demand; nothing on disk
# holds it in the clear.
#
# Deliberately NOT in a remote URL. A token in .git/config sits inside the
# unattended triage agent's Read(<repo>/**) scope. This script is inside that
# scope and that is fine — it contains no secret, only the instruction to go
# ask for one, and the agent has no Bash to run it.
#
# Wire it up IN THE DATA REPOSITORY ALONE, since its token pushes nothing else,
# through the units' link to the DEPLOYED app, which a deploy moves and a
# collection leaves standing: never a dev checkout (it follows whatever branch
# is checked out, so unreviewed code would sit in the push path) and never a
# store path (the next deploy's collection deletes it). The empty entry clears
# the helpers a global config lists, gh's among them, so this one is the only
# one asked:
#   git -C <data repository> config credential.https://github.com.helper ''
#   git -C <data repository> config --add credential.https://github.com.helper \
#     "$SUZATARY_ROOTS/app/scripts/git-credential-secretspec.sh"
set -euo pipefail

# Git calls helpers with get|store|erase. Only `get` is meaningful: the value is
# owned by pass, so writing back or erasing from git would be a lie about who
# holds it. Exit 0 silently on the others — a nonzero exit makes git treat the
# whole helper as broken.
[ "${1:-}" = "get" ] || exit 0

# SUZATARY_REPO is set by the systemd unit and by an interactive box's shell
# profile; the fallback resolves THIS SCRIPT'S OWN checkout (`$0` is the
# absolute path git invoked, since the helper is wired by absolute path above),
# so a second host's setup line needs no env var at all — ROADMAP.md, "a second
# host".
REPO="${SUZATARY_REPO:-$(cd "$(dirname "$0")/.." && pwd)}"

# --reason satisfies the require_reason="agents" policy in secretspec.toml and
# is what shows up in the audit log; without it this fails whenever git is
# invoked from an agent session.
PAT=$(secretspec get SUZATARY_DATA_PUSH_TOKEN \
  --file "$REPO/secretspec.toml" \
  --reason "git push/fetch of the data repository against github.com")

# GitHub ignores the username of token auth, but git needs one: the remote
# URL's own (`https://<user>@github.com/...`, which git hands us on stdin) when
# it names one, else GitHub's generic token user, so no deployment's name is
# written here.
user="x-access-token"
while IFS='=' read -r key value; do
  [ "$key" = "username" ] && [ -n "$value" ] && user="$value"
done
echo "username=$user"
echo "password=$PAT"
