#!/usr/bin/env bash
# The runner cargo hands each test executable to when the merge gate runs some of the
# workspace's members (scripts/merge-gate.sh, SUZATARY_GATE_UNITS). Cargo builds every member's
# tests with the workspace's features, so each executable is the one a whole run runs, and it
# runs here with cargo's own environment and directory. An executable of a member not named
# exits 0 without running; one cargo names no member for runs.
#
#   cargo test --workspace --config "target.<host>.runner=['scripts/test-units.sh']"
set -euo pipefail
if [[ -z "${CARGO_PKG_NAME:-}" || " ${SUZATARY_GATE_UNITS:-} " == *" $CARGO_PKG_NAME "* ]]; then
  exec "$@"
fi
echo "test-units: not run: the inputs of $CARGO_PKG_NAME's tests did not move since they passed"
