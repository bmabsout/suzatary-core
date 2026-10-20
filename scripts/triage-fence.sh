#!/usr/bin/env bash
# THE TRIAGE AGENT'S FENCE, AS IT RUNS: scripts/triage-fence.sh <repo> prints
# the settings JSON the agent is started with (`claude --settings <json>`),
# after proving the hook it registers both denies and allows, or prints nothing
# and exits 1.
#
# .claude/triage-settings.json is a template: each hook's command is a
# placeholder this script fills with the guard's ABSOLUTE path under <repo>,
# the path it then probes, so what is checked is what runs, never a second
# spelling (`${CLAUDE_PROJECT_DIR}` was one: Claude Code resolves the project
# directory itself, and it need not be the directory the preflight checked).
#
# FAIL CLOSED. Claude Code blocks a call only when a hook exits 2; a hook that
# is missing, not executable or crashing exits otherwise and lets the call
# through. So the registered command is `'<guard>' || exit 2`: the guard's own
# 0 allows, and every other way it can end blocks.
#
# The probes run that registered command through `sh -c`, as Claude Code runs
# it, read back from the JSON this script prints. Both directions are required,
# because the two failures are opposite and both silent: a fence that denies
# nothing runs the agent uncontained, and one that denies everything runs it
# blind (the verbs arrive through ToolSearch, and a toolless agent narrates a
# triage it never performed).
set -euo pipefail

repo="${1:?usage: triage-fence.sh <repo>}"
template="$repo/.claude/triage-settings.json"
tool_guard="$repo/scripts/tool-guard.sh"
edit_guard="$repo/scripts/edit-guard.sh"

die() { echo "triage-fence: $1 — refusing to run the agent" >&2; exit 1; }

[ -r "$template" ] || die "$template is missing"
for guard in "$tool_guard" "$edit_guard"; do
  [ -x "$guard" ] || die "$guard is missing or not executable"
  case "$guard" in
    /*) ;;
    *) die "$guard is not an absolute path" ;;
  esac
  case "$guard" in
    *[\'\"\\\|#]*) die "$guard holds a quote, a backslash, a bar or a hash, which the hook's command cannot carry" ;;
  esac
done

settings="$(sed \
  -e "s|@TOOL_GUARD@|'$tool_guard' \|\| exit 2|g" \
  -e "s|@EDIT_GUARD@|'$edit_guard' \|\| exit 2|g" \
  "$template")"
case "$settings" in
  *@*_GUARD@*) die "$template holds a placeholder this script does not fill" ;;
esac

# The catch-all hook's command, as the JSON registers it.
hook="$(printf '%s\n' "$settings" \
  | sed -n "s#.*\"command\": *\"\\('[^']*/tool-guard.sh' || exit 2\\)\".*#\\1#p")"
[ "$(printf '%s\n' "$hook" | grep -c .)" -eq 1 ] \
  || die "the settings register the tool guard $(printf '%s\n' "$hook" | grep -c .) times, not once"
[ "$hook" = "'$tool_guard' || exit 2" ] \
  || die "the settings register $hook, not the guard this script checks"

if printf '%s' '{"tool_name":"Bash","tool_input":{"command":"id"}}' | sh -c "$hook" >/dev/null 2>&1; then
  die "the registered hook ALLOWED a Bash call: the fence is broken"
fi
if ! printf '%s' '{"tool_name":"mcp__suzatary__context","tool_input":{}}' | sh -c "$hook" >/dev/null 2>&1; then
  die "the registered hook DENIED its own verbs: the agent would be blind"
fi

printf '%s\n' "$settings"
