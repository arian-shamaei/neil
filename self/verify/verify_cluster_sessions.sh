#!/bin/bash
# verify_cluster_sessions.sh — static + live checks on the cluster-sessions stack.
#
# Static checks (no side effects):
#   - Every bin/ tool exists, is executable, and passes bash -n or python -m py_compile
#   - observe.sh contains the "Open Peer Sessions" block
#   - essence/actions.md contains the "Peer sessions" rule
#   - services/handler.sh contains the async queue branch (action=queue)
#
# Live checks (writes to /tmp only):
#   - neil-session-scan runs on main, produces valid JSON at state/sessions.json
#   - A synthetic "[SESSION test_<ts>]" prompt file in a temp dir is recognized
#   - observe.sh emits the block (grep the rendered output)
#
# Exit code: 0 if every check passes, 1 otherwise.
set -u
NEIL_HOME=${NEIL_HOME:-$HOME/.neil}
PASS=0; FAIL=0
note() { printf '%s %s\n' "$1" "$2"; }
check() { local desc="$1" cond="$2"; if eval "$cond" >/dev/null 2>&1; then note '[PASS]' "$desc"; PASS=$((PASS+1)); else note '[FAIL]' "$desc -- $cond"; FAIL=$((FAIL+1)); fi; }

echo "=== Static checks ==="

check "bin/neil-session-scan exists"      "[ -x $NEIL_HOME/bin/neil-session-scan ]"
check "bin/neil-creds-sync exists"        "[ -x $NEIL_HOME/bin/neil-creds-sync ]"
check "bin/neil-cluster-watch exists"     "[ -x $NEIL_HOME/bin/neil-cluster-watch ]"
check "bin/neil-pingpong exists"          "[ -x $NEIL_HOME/bin/neil-pingpong ]"

check "neil-session-scan py_compile"      "python3 -m py_compile $NEIL_HOME/bin/neil-session-scan"
check "neil-cluster-watch bash -n"        "bash -n $NEIL_HOME/bin/neil-cluster-watch"
check "neil-pingpong bash -n"             "bash -n $NEIL_HOME/bin/neil-pingpong"
check "neil-creds-sync bash -n"           "bash -n $NEIL_HOME/bin/neil-creds-sync"

check "observe.sh has Open Peer Sessions block"  "grep -q 'Open Peer Sessions' $NEIL_HOME/tools/autoPrompter/observe.sh"
check "observe.sh references neil-session-scan"  "grep -q 'neil-session-scan' $NEIL_HOME/tools/autoPrompter/observe.sh"
check "essence/actions.md has peer-sessions rule" "grep -q 'Peer sessions' $NEIL_HOME/essence/actions.md"
check "essence/actions.md has BALL IN YOUR COURT" "grep -q 'BALL IN YOUR COURT' $NEIL_HOME/essence/actions.md"
check "handler.sh has action=queue branch"        "grep -q 'async queue-drop branch' $NEIL_HOME/services/handler.sh"
check "handler.sh has peer_send_queued event"     "grep -q 'peer_send_queued' $NEIL_HOME/services/handler.sh"

echo
echo "=== Live scanner ==="

TMPD=$(mktemp -d)
# Simulate an inbound session message: a '*_from_testpeer.md' file in a
# throwaway history-style directory, with a [SESSION ...] marker in its body.
NEIL_HOME_TEST=$TMPD
mkdir -p "$TMPD/state" "$TMPD/tools/autoPrompter/history"
SID="sess_test_$(date +%s)"
cat > "$TMPD/tools/autoPrompter/history/$(date -u +%Y%m%dT%H%M%S)_from_testpeer.md" <<EOF
[SESSION $SID] Hello from testpeer. The detector question remains open.
EOF

NEIL_HOME=$TMPD "$NEIL_HOME/bin/neil-session-scan" --print >"$TMPD/scan.out" 2>&1

check "scanner produces state/sessions.json"    "[ -s '$TMPD/state/sessions.json' ]"
check "scanner picks up synthetic session id"   "grep -q '$SID' '$TMPD/state/sessions.json'"
check "scanner marks ball in court for inbound" "grep -q 'ball_in_my_court.*true' '$TMPD/state/sessions.json'"
check "scanner --print shows BALL IN YOUR COURT line" "grep -q '<<' '$TMPD/scan.out'"

echo
echo "=== Live observe.sh block ==="

# Observe.sh inherits HOME; copy the synthetic state into a throwaway HOME
# and run observe.sh against it. We only care that the Open Peer Sessions
# block renders with our synthetic session.
mkdir -p "$TMPD/home/.neil"
cp -r "$TMPD/state" "$TMPD/home/.neil/"
cp -r "$TMPD/tools" "$TMPD/home/.neil/"
cp -r "$NEIL_HOME/bin" "$TMPD/home/.neil/"
cp "$NEIL_HOME/tools/autoPrompter/observe.sh" "$TMPD/home/.neil/tools/autoPrompter/observe.sh" 2>/dev/null || true
# Strip any main-only bits that reference NEIL_HOME specifically
HOME="$TMPD/home" bash "$NEIL_HOME/tools/autoPrompter/observe.sh" >"$TMPD/observe.out" 2>&1 || true

check "observe.sh renders Open Peer Sessions header" "grep -q 'Open Peer Sessions' '$TMPD/observe.out'"
check "observe.sh renders synthetic session id"      "grep -q '$SID' '$TMPD/observe.out'"

rm -rf "$TMPD"

echo
echo "=== Summary ==="
echo "PASS: $PASS   FAIL: $FAIL"
if [ $FAIL -eq 0 ]; then
    echo "VERDICT: [PASS] cluster-sessions stack verified"
    exit 0
else
    echo "VERDICT: [FAIL] see failures above"
    exit 1
fi
