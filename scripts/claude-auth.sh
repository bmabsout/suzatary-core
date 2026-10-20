# shellcheck shell=bash
# The credential `claude -p` runs the triage agent under, for scripts/hourly-triage.sh to
# source: the year-long token `claude setup-token` mints, else an Anthropic API key, each
# resolved through secretspec so neither sits in a file the agent can read. claude reads
# either from its environment, and `suzatary init` prints the step that sets whichever the
# deployment's model needs. Returns 1 when neither is set.
claude_auth() {
  local token key
  token=$(secretspec get CLAUDE_CODE_OAUTH_TOKEN --reason "hourly triage: headless claude auth" 2>/dev/null || true)
  if [ -n "$token" ]; then
    export CLAUDE_CODE_OAUTH_TOKEN="$token"
    return 0
  fi
  key=$(secretspec get ANTHROPIC_API_KEY --reason "hourly triage: headless claude auth by API key" 2>/dev/null || true)
  if [ -n "$key" ]; then
    export ANTHROPIC_API_KEY="$key"
    return 0
  fi
  return 1
}
