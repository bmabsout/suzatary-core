#!/usr/bin/env bash
# PreToolUse hook for the unattended triage session: DEFAULT DENY.
#
# Why this exists: `permissions.allow` is not an exclusive allowlist. Under
# `defaultMode: dontAsk` with `allow: ["mcp__suzatary__*"]`, a tool that is
# neither allowed nor denied is still present AND CALLABLE. Measured, not
# theorised: an agent in exactly the triage invocation called `Monitor` —
# which takes a `command` and runs it in the same shell environment as Bash —
# and got a shell, so the deny list was doing all of the containment and
# `allow` was doing none.
#
# That is the failure this repo's own principle predicts: `permissions.deny` is
# a blocklist of NAMES, and `Monitor` is shell execution that is not named
# `Bash`. A blocklist cannot be finished, because the next release adds a name
# nobody here has heard of yet.
#
# So this hook inverts it. It does not ask what a tool is called or what it
# does; it permits exactly one namespace and denies everything else, including
# tools that did not exist when this was written. That property is why the hook
# is the fence and the deny list is only the belt.
#
# Exit codes per the hooks contract: 0 = allow, 2 = block (stderr goes back to
# the agent). Parse failure = deny; an unparseable call is not a permitted call.
set -uo pipefail

ALLOWED_PREFIX="mcp__suzatary__"

# ToolSearch is the one non-suzatary tool permitted, and it is permitted
# because denying it does not harden the agent, it BLINDS it. Measured: the
# suzatary verbs do not arrive in the session's initial tool
# list at all — the MCP server is still "pending" at init, and its verbs are
# surfaced as DEFERRED tools whose schemas load through ToolSearch. With
# ToolSearch denied the agent reported NO_TOOL and could not call `context`;
# with it available the same agent returned the real category vocabulary.
#
# Permitting it costs nothing, because ToolSearch only fetches a schema. It
# cannot invoke anything: every call that results from a lookup arrives back
# at this same hook and is checked against the namespace like any other. So a
# lookup can tell the agent that `Monitor` exists and what its arguments are,
# and the call still gets exit 2.
ALLOWED_EXACT="ToolSearch"

IN=$(cat)

# EVERY occurrence, not the first. `tool_input` is attacker-influenced — it
# carries the body of an email — so a payload can contain the literal text
# `"tool_name": "mcp__suzatary__show"` and, if the outer key order ever put
# tool_input first, a `head -1` parse would read the forgery instead of the
# real call. Requiring ALL matches to be in the namespace makes an injected
# key harmless in the only direction that matters: a forged entry can turn an
# allow into a deny (fail closed), never a deny into an allow.
NAMES=$(printf '%s' "$IN" | grep -o '"tool_name"[[:space:]]*:[[:space:]]*"[^"]*"' \
  | sed 's/.*:[[:space:]]*"//; s/"$//')

if [ -z "$NAMES" ]; then
  echo "tool-guard: could not parse tool_name from the tool call — denying by default" >&2
  exit 2
fi

while IFS= read -r NAME; do
  [ -n "$NAME" ] || continue
  case "$NAME" in
    "$ALLOWED_PREFIX"*) ;;
    "$ALLOWED_EXACT") ;;
    *)
      echo "tool-guard: '$NAME' is not a suzatary verb — denied." >&2
      echo "tool-guard: this session may only call ${ALLOWED_PREFIX}* tools. Reading files," >&2
      echo "tool-guard: running commands and reaching the network are not available to you;" >&2
      echo "tool-guard: use the suzatary verbs instead (start with ${ALLOWED_PREFIX}context)." >&2
      exit 2
      ;;
  esac
done <<EOF
$NAMES
EOF

exit 0
