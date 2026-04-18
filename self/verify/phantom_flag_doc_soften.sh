#!/bin/bash
# Verify: essence/overview.md describes neil_os_enabled accurately.
#
# Ground truth (as of 2026-04-17, after Steps 1/1.5/3 shipped):
#   - neil_os_enabled IS parsed at autoprompt.c startup
#   - neil_os_enabled IS logged at startup
#   - neil_os_enabled IS enforced at beat_router.sh:20-26
#
# This verify passes when overview.md's claim about the kill switch
# is consistent with the shipped behavior. It fails if overview.md
# still describes the flag as merely "documented" with no enforcement,
# OR if it describes enforcement that doesn't exist in source.
#
# Approval-gated: Step 2 requires operator approval to edit essence/.
# This script does NOT modify anything; it only reports whether the
# docs match shipped behavior. If verify-fail, the work to do is:
# propose a diff to overview.md and wait for operator approval.

set -e
OVERVIEW="${NEIL_HOME:-$HOME/.neil}/essence/overview.md"
ROUTER="${NEIL_HOME:-$HOME/.neil}/tools/beat_router/beat_router.sh"

if [[ ! -f "$OVERVIEW" ]]; then
    echo "FAIL: essence/overview.md not found at $OVERVIEW" >&2
    exit 1
fi

# Must mention the flag by name somewhere
if ! grep -q "neil_os_enabled" "$OVERVIEW"; then
    echo "FAIL: overview.md does not mention neil_os_enabled at all" >&2
    exit 2
fi

# Must mention kill switch
if ! grep -qi "kill switch\|kill_switch" "$OVERVIEW"; then
    echo "FAIL: overview.md does not describe the kill switch concept" >&2
    exit 3
fi

# The flag must actually BE enforced in beat_router for the docs to be honest
if ! grep -q "neil_os_enabled" "$ROUTER"; then
    echo "FAIL: docs reference neil_os_enabled but beat_router.sh does not read it" >&2
    exit 4
fi

# Negative check: docs should NOT describe the flag as "unimplemented" or "not yet"
# since it is now enforced. If they still do, they're stale.
if grep -qiE "neil_os_enabled.{0,80}(not yet|unimplemented|todo|future|planned)" "$OVERVIEW"; then
    echo "FAIL: overview.md still describes neil_os_enabled as unimplemented, but it is shipped" >&2
    grep -inE "neil_os_enabled.{0,80}(not yet|unimplemented|todo|future|planned)" "$OVERVIEW" >&2
    exit 5
fi

echo "verify-ok: overview.md mentions neil_os_enabled and kill switch, enforcement exists in beat_router.sh"
exit 0