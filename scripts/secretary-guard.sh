#!/usr/bin/env bash
# PreToolUse hook for the SECRETARY's model subprocess: DENY EVERYTHING.
#
# This is not the triage fence with a shorter allowlist — it is the same fence
# with an EMPTY one, and that difference is the whole design of
# docs/ARCHITECTURE.md §10.3. The triage agent calls suzatary verbs through MCP,
# so its guard has to let `mcp__suzatary__*` through. The secretary does not:
# its verbs are dispatched by `suzatary_core::secretary::converse` on THIS side
# of the `Model` trait, and the model's only channel is the JSON envelope it
# prints. So it needs no tools, is given none, and this refuses any it somehow
# acquires — including tools that did not exist when this was written, which is
# the property `scripts/tool-guard.sh`'s header explains at length and this one
# inherits by being stricter.
#
# ⚠️ ToolSearch IS NOT IN THE DENY LIST BESIDE THIS. The repository's rule is
# BLANKET — no settings file under `.claude/`
# may name it, and `app/cli/tests/guards.rs` checks every one of them — because
# over on the triage side it is the channel the verbs actually arrive through,
# and denying it makes the agent blind rather than safe. Here there is no MCP
# server at all, so denying it would blind
# nothing; naming it would still weaken a rule that is stronger for having no
# exceptions. THIS HOOK REFUSES IT ANYWAY, along with everything else, which is
# the whole point of a fence that denies by default.
#
# Exit codes per the hooks contract: 0 = allow, 2 = block (stderr goes back to
# the model). This never exits 0.
set -uo pipefail

# The input is read and discarded. There is no name that would change the
# answer, which is exactly why this file has no parser in it: a guard with no
# branch cannot be talked past by a payload shaped like a tool call.
cat >/dev/null

echo "secretary-guard: this session holds no tools at all." >&2
echo "secretary-guard: your verbs are not tools here — ask for one by printing" >&2
echo "secretary-guard: {\"say\": \"...\", \"calls\": [{\"verb\": \"...\", \"args\": {...}}]}" >&2
echo "secretary-guard: and the box will run it and hand you the answer." >&2
exit 2
