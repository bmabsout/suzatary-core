#!/usr/bin/env bash
# PreToolUse hook for the unattended triage session: no Edit or Write reaches
# a file.
#
# Why this exists: Edit(<glob>) argument-scoping is not enforced by the
# permission system (measured). The agent writes only through its verbs, onto
# its replica, so its writable set of files is empty and every call is denied
# BEFORE the tool runs.
#
# This is a BACKSTOP, not the fence. The agent holds
# no Edit at all, and scripts/tool-guard.sh denies by default every tool
# outside `mcp__suzatary__*`. Kept anyway, because a backstop costs nothing and
# the failure it covers — a settings file quietly reverting — is real.
#
# Registered in .claude/triage-settings.json, NOT in a project settings.json:
# aimed at the repo it would hit every human too, and a plain session in this
# directory could not write a file.
#
# Exit codes per the hooks contract: 0 = allow, 2 = block (stderr shown to the
# agent). It never answers 0.
set -uo pipefail

cat >/dev/null
echo "edit-guard: the triage agent writes no file; todos go through todo_add/todo_update, state.json through mark/noise_add." >&2
exit 2
