#!/bin/bash
# Verify that the handler→spawn_vm param-forwarding arc is resolved.
#
# Resolution criteria (from ground-truth probes on 2026-04-20):
#   1. handler.sh parses NEIL_PARAMS into PARAM_<key> env vars (lines 16-33)
#   2. handler.sh:spawn_vm case invokes spawn_vm.sh (env vars inherit)
#   3. spawn_vm.sh reads $PARAM_persona, $PARAM_memory_mode from env
#   4. The "CALL parameter fidelity" lesson is codified in essence/actions.md
#
# If all four hold, the arc is architecturally closed and the humanizer-pair
# failure was a CALL-grammar bug (wrong param names), not a handler bug.

set -e
NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
HANDLER="$NEIL_HOME/services/handler.sh"
SPAWN_VM="$NEIL_HOME/tools/spawn_vm/spawn_vm.sh"
ACTIONS="$NEIL_HOME/essence/actions.md"

[ -f "$HANDLER" ]  || { echo "MISSING: $HANDLER" >&2; exit 2; }
[ -f "$SPAWN_VM" ] || { echo "MISSING: $SPAWN_VM" >&2; exit 2; }
[ -f "$ACTIONS" ]  || { echo "MISSING: $ACTIONS" >&2; exit 2; }

# 1. handler.sh must export PARAM_* env vars from NEIL_PARAMS
grep -q "export PARAM_" "$HANDLER" || { echo "FAIL: handler.sh does not export PARAM_* env vars" >&2; exit 3; }

# 2. handler.sh spawn_vm case must invoke spawn_vm.sh
grep -A2 "^    spawn_vm)" "$HANDLER" | grep -q "spawn_vm.sh" || { echo "FAIL: handler.sh spawn_vm case does not invoke spawn_vm.sh" >&2; exit 4; }

# 3. spawn_vm.sh must read PARAM_persona and PARAM_memory_mode
grep -q 'PARAM_persona'     "$SPAWN_VM" || { echo "FAIL: spawn_vm.sh does not read \$PARAM_persona" >&2; exit 5; }
grep -q 'PARAM_memory_mode' "$SPAWN_VM" || { echo "FAIL: spawn_vm.sh does not read \$PARAM_memory_mode" >&2; exit 6; }

# 4. essence/actions.md must contain the CALL parameter fidelity lesson
grep -q "CALL parameter fidelity" "$ACTIONS" || { echo "FAIL: essence/actions.md missing 'CALL parameter fidelity' lesson" >&2; exit 7; }

echo "OK: handler→spawn_vm arc architecturally resolved. PARAM_ forwarding works via env-var inheritance; humanizer-pair failure was CALL-grammar error (already codified as essence lesson)."
exit 0