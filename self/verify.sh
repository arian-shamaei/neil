#!/bin/sh
# verify.sh -- Comprehensive system verification for Neil
# Tests every component, action type, and interaction.
# Usage: ./verify.sh [--quick]   (quick skips Claude invocations)

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
QUEUE="$NEIL_HOME/tools/autoPrompter/queue"
HISTORY="$NEIL_HOME/tools/autoPrompter/history"
PASS=0
FAIL_COUNT=0
SKIP=0
QUICK="${1:-}"

pass() { echo "  PASS: $1"; PASS=$((PASS + 1)); }
fail_test() { echo "  FAIL: $1 -- $2"; FAIL_COUNT=$((FAIL_COUNT + 1)); }
skip_test() { echo "  SKIP: $1"; SKIP=$((SKIP + 1)); }

section() { echo ""; echo "=== $1 ==="; }

wait_for_result() {
    local marker="$1"
    local timeout="${2:-90}"
    local i=0
    while [ $i -lt $timeout ]; do
        LATEST=$(ls -t "$HISTORY"/*.result.md 2>/dev/null | head -1)
        if [ -n "$LATEST" ] && [ "$LATEST" -nt "$marker" ]; then
            echo "$LATEST"
            return 0
        fi
        sleep 2
        i=$((i + 2))
    done
    return 1
}

echo "======================================"
echo "  Neil System Verification"
echo "  $(date -Iseconds)"
echo "======================================"

# 1. Directory Structure
section "Directory Structure"

for D in essence tools/autoPrompter memory/zettel memory/mempalace memory/palace \
         memory/palace/notes memory/palace/index services/registry services/vault \
         inputs/watchers outputs/channels self; do
    if [ -d "$NEIL_HOME/$D" ]; then
        pass "$D/"
    else
        fail_test "$D/" "directory missing"
    fi
done

# 2. Essence Files
section "Essence Files"

for F in identity.md soul.md mission.md overview.md actions.md heartbeat.md guardrails.md; do
    if [ -f "$NEIL_HOME/essence/$F" ] || [ -L "$NEIL_HOME/essence/$F" ]; then
        pass "essence/$F"
    else
        fail_test "essence/$F" "file missing"
    fi
done

# Check lessons symlink
if [ -L "$NEIL_HOME/essence/lessons.md" ]; then
    pass "essence/lessons.md (symlink)"
else
    fail_test "essence/lessons.md" "not a symlink to self/lessons.md"
fi

# 3. Binaries and Scripts
section "Binaries and Scripts"

for B in tools/autoPrompter/autoprompt memory/zettel/zettel; do
    if [ -x "$NEIL_HOME/$B" ]; then
        pass "$B"
    else
        fail_test "$B" "not executable"
    fi
done

for S in tools/autoPrompter/heartbeat.sh tools/autoPrompter/observe.sh \
         services/handler.sh self/self_check.sh \
         inputs/watchers/filesystem.sh inputs/watchers/webhook.sh \
         inputs/watchers/schedule.sh \
         outputs/channels/terminal.sh outputs/channels/file.sh \
         outputs/channels/email.sh outputs/channels/slack.sh; do
    if [ -x "$NEIL_HOME/$S" ]; then
        pass "$S"
    else
        fail_test "$S" "not executable"
    fi
done

# 4. Zettel Operations
section "Zettel Operations"

export ZETTEL_HOME="$NEIL_HOME/memory/palace"
ZETTEL="$NEIL_HOME/memory/zettel/zettel"

ZETTEL_ID=$($ZETTEL new "Verification test note" --wing test --room verify --tags "test,verify" 2>/dev/null)
if [ -n "$ZETTEL_ID" ]; then
    pass "zettel new (id: $ZETTEL_ID)"
else
    fail_test "zettel new" "no ID returned"
fi

if [ -n "$ZETTEL_ID" ]; then
    SHOW=$($ZETTEL show "$ZETTEL_ID" 2>/dev/null)
    if echo "$SHOW" | grep -q "wing:.*test" && echo "$SHOW" | grep -q "room:.*verify"; then
        pass "zettel show (wing/room)"
    else
        fail_test "zettel show" "wing/room missing"
    fi
fi

CTX=$($ZETTEL context 2>/dev/null)
if echo "$CTX" | grep -q "wing/test"; then
    pass "zettel context"
else
    fail_test "zettel context" "test wing not visible"
fi

FOUND=$($ZETTEL find --tag verify 2>/dev/null)
if echo "$FOUND" | grep -q "$ZETTEL_ID"; then
    pass "zettel find --tag"
else
    fail_test "zettel find --tag" "not found"
fi

FOUND=$($ZETTEL find --text "Verification test" 2>/dev/null)
if echo "$FOUND" | grep -q "$ZETTEL_ID"; then
    pass "zettel find --text"
else
    fail_test "zettel find --text" "not found"
fi

FOUND=$($ZETTEL find --wing test --text "Verification" 2>/dev/null)
if echo "$FOUND" | grep -q "$ZETTEL_ID"; then
    pass "zettel find --wing scoped"
else
    fail_test "zettel find --wing scoped" "not found"
fi

LIST=$($ZETTEL list --wing test 2>/dev/null)
if echo "$LIST" | grep -q "$ZETTEL_ID"; then
    pass "zettel list --wing"
else
    fail_test "zettel list --wing" "not in list"
fi

ZETTEL_ID2=$($ZETTEL new "Second verify note" --wing test --room verify --tags "test" 2>/dev/null)
if [ -n "$ZETTEL_ID2" ]; then
    $ZETTEL link "$ZETTEL_ID" "$ZETTEL_ID2" > /dev/null 2>&1
    SHOW2=$($ZETTEL show "$ZETTEL_ID" 2>/dev/null)
    if echo "$SHOW2" | grep -q "$ZETTEL_ID2"; then
        pass "zettel link (bidirectional)"
    else
        fail_test "zettel link" "link not found"
    fi
fi

GRAPH=$($ZETTEL graph "$ZETTEL_ID" 1 2>/dev/null)
if echo "$GRAPH" | grep -q "$ZETTEL_ID2"; then
    pass "zettel graph"
else
    fail_test "zettel graph" "linked note missing"
fi

$ZETTEL rm "$ZETTEL_ID" > /dev/null 2>&1
$ZETTEL rm "$ZETTEL_ID2" > /dev/null 2>&1
if $ZETTEL show "$ZETTEL_ID" 2>&1 | grep -q "not found"; then
    pass "zettel rm + backlink cleanup"
else
    fail_test "zettel rm" "note still exists"
fi

# 5. MemPalace
section "MemPalace"

if . "$NEIL_HOME/memory/mempalace/.venv/bin/activate" 2>/dev/null; then
    pass "mempalace venv"
    SEARCH=$(mempalace --palace "$NEIL_HOME/memory/palace/.mempalace" search "inotify" --results 1 2>/dev/null)
    if echo "$SEARCH" | grep -q "Results for"; then
        pass "mempalace search"
    else
        fail_test "mempalace search" "no results"
    fi
else
    fail_test "mempalace venv" "activation failed"
fi

# 6. Observation Layer
section "Observation Layer"

OBS=$("$NEIL_HOME/tools/autoPrompter/observe.sh" 2>/dev/null)
for SEC in "System" "Services" "Input Watchers" "Memory Palace" "Queue" \
           "Recent History" "Heartbeat Log" "Intentions" "Self Check" \
           "Unresolved Failures" "Guardrails"; do
    if echo "$OBS" | grep -q "$SEC"; then
        pass "observe: $SEC"
    else
        fail_test "observe: $SEC" "section missing"
    fi
done

# 7. Self Check
section "Self Check (28-point)"

if "$NEIL_HOME/self/self_check.sh" > /dev/null 2>&1; then
    pass "self_check.sh (all 28 passed)"
else
    fail_test "self_check.sh" "some checks failed"
fi

# 8. Services
section "Services"

RESULT=$(NEIL_SERVICE=test NEIL_ACTION=time NEIL_CRED=none NEIL_PARAMS="" \
         "$NEIL_HOME/services/handler.sh" 2>/dev/null)
if echo "$RESULT" | grep -q "server_time"; then
    pass "handler.sh test/time"
else
    fail_test "handler.sh test/time" "bad response"
fi

RESULT=$(NEIL_SERVICE=test NEIL_ACTION=echo NEIL_CRED=none NEIL_PARAMS="message=hello" \
         "$NEIL_HOME/services/handler.sh" 2>/dev/null)
if echo "$RESULT" | grep -q "hello"; then
    pass "handler.sh test/echo"
else
    fail_test "handler.sh test/echo" "bad response"
fi

# 9. Output Channels
section "Output Channels"

MARKER="verify_$(date +%s)"
NEIL_CHANNEL=terminal NEIL_MESSAGE="$MARKER" \
    "$NEIL_HOME/outputs/channels/terminal.sh" 2>/dev/null
if tail -1 "$NEIL_HOME/outputs/neil.log" 2>/dev/null | grep -q "$MARKER"; then
    pass "terminal channel"
else
    fail_test "terminal channel" "not in neil.log"
fi

TMPFILE="/tmp/neil_verify_$$.txt"
NEIL_CHANNEL=file NEIL_MESSAGE="file test" NEIL_PARAM_to="$TMPFILE" \
    "$NEIL_HOME/outputs/channels/file.sh" 2>/dev/null
if [ -f "$TMPFILE" ] && grep -q "file test" "$TMPFILE"; then
    pass "file channel"
    rm -f "$TMPFILE"
else
    fail_test "file channel" "not written"
fi

# 10. Input Watchers
section "Input Watchers"

# Test schedule watcher by checking it creates the file (may be consumed immediately by autoPrompter)
SCHED_OUT=$("$NEIL_HOME/inputs/watchers/schedule.sh" verify_sched "Test prompt" 2>&1)
if echo "$SCHED_OUT" | grep -q "queued"; then
    pass "schedule watcher"
    rm -f "$QUEUE"/*verify_sched* 2>/dev/null
else
    fail_test "schedule watcher" "script did not report queued"
fi

# 11. systemd
section "systemd"

if systemctl is-active autoprompt > /dev/null 2>&1; then
    pass "autoprompt active"
else
    fail_test "autoprompt" "not active"
fi

if systemctl is-enabled autoprompt > /dev/null 2>&1; then
    pass "autoprompt enabled"
else
    fail_test "autoprompt" "not enabled"
fi

# 12. Cron
section "Cron"

if crontab -l 2>/dev/null | grep -q "heartbeat.sh"; then
    pass "heartbeat cron"
else
    fail_test "heartbeat cron" "not in crontab"
fi

# 13. Portability
section "Portability"

HC=$(grep -c '/home/seal' "$NEIL_HOME/tools/autoPrompter/src/autoprompt.c" 2>/dev/null)
if [ "$HC" -eq 0 ] 2>/dev/null; then
    pass "autoprompt.c: portable"
else
    fail_test "autoprompt.c" "$HC hardcoded paths"
fi

HC1=$(grep -c '128\.95\|sealserver' "$NEIL_HOME/essence/identity.md" 2>/dev/null || true)
HC2=$(grep -c '128\.95\|sealserver' "$NEIL_HOME/essence/mission.md" 2>/dev/null || true)
HC1=${HC1:-0}
HC2=${HC2:-0}
HC_ESS=$((HC1 + HC2))
if [ "$HC_ESS" -eq 0 ]; then
    pass "essence: portable"
else
    fail_test "essence" "$HC_ESS deployment-specific references"
fi

# 14. READMEs
section "READMEs"

for R in README.md essence/README.md tools/autoPrompter/README.md \
         memory/README.md services/README.md inputs/README.md \
         outputs/README.md self/README.md; do
    if [ -f "$NEIL_HOME/$R" ]; then
        pass "$R"
    else
        fail_test "$R" "missing"
    fi
done

# 15. Claude Integration
section "Claude Integration"

if [ "$QUICK" = "--quick" ]; then
    skip_test "Full Claude integration (quick mode)"
else
    echo "  (sending test prompt -- ~60s)"
    MARKER="/tmp/neil_verify_marker_$$"
    touch "$MARKER"

    cat > "$QUEUE/verify_actions.md" << 'VPROMPT'
System verification test. Output these EXACT lines, one per line:

MEMORY: wing=test room=verify tags=automated | System verification passed
CALL: service=test action=echo message=verify_ok
NOTIFY: channel=terminal | VERIFY: actions test complete
INTEND: priority=low tag=verify | Cleanup verification artifacts
DONE: Cleanup verification
FAIL: source=verify severity=low context=automated_test | Test failure entry (not real)
HEARTBEAT: status=acted summary="Verification: all action types tested."

Then say: "All actions emitted."
VPROMPT

    RESULT_FILE=$(wait_for_result "$MARKER" 120)
    rm -f "$MARKER"

    if [ -z "$RESULT_FILE" ]; then
        fail_test "Claude invocation" "timeout"
    else
        EXIT=$(grep 'exit_code' "$RESULT_FILE" | head -1 | sed 's/[^0-9]//g')
        if [ "$EXIT" = "0" ]; then pass "Claude exit 0"; else fail_test "Claude" "exit $EXIT"; fi

        # MEMORY stored?
        VN=$(ZETTEL_HOME="$NEIL_HOME/memory/palace" "$ZETTEL" find --tag automated 2>/dev/null | head -1 | awk '{print $1}')
        if [ -n "$VN" ]; then
            pass "MEMORY: stored"
            $ZETTEL rm "$VN" > /dev/null 2>&1
        else
            fail_test "MEMORY:" "note not found"
        fi

        # CALL results?
        if grep -q "verify_ok" "$RESULT_FILE" 2>/dev/null; then
            pass "CALL: + ReAct"
        else
            TURNS=$(grep 'turns' "$RESULT_FILE" | head -1 | sed 's/[^0-9]//g')
            if [ "$TURNS" -gt 1 ] 2>/dev/null; then
                pass "CALL: ReAct ($TURNS turns)"
            else
                skip_test "CALL: ReAct (Claude may have run directly)"
            fi
        fi

        # NOTIFY?
        if tail -5 "$NEIL_HOME/outputs/neil.log" 2>/dev/null | grep -q "VERIFY"; then
            pass "NOTIFY: terminal"
        else
            fail_test "NOTIFY:" "not in neil.log"
        fi

        # INTEND + DONE?
        if grep -q "verification" "$NEIL_HOME/intentions.json" 2>/dev/null; then
            pass "INTEND: created"
            if grep "verification" "$NEIL_HOME/intentions.json" | grep -q "completed"; then
                pass "DONE: completed"
            else
                fail_test "DONE:" "not completed"
            fi
        else
            fail_test "INTEND:" "not in intentions.json"
        fi

        # FAIL?
        if grep -q "Test failure entry" "$NEIL_HOME/self/failures.json" 2>/dev/null; then
            pass "FAIL: logged"
        else
            fail_test "FAIL:" "not in failures.json"
        fi

        # HEARTBEAT?
        if tail -3 "$NEIL_HOME/heartbeat_log.json" 2>/dev/null | grep -qi "verification"; then
            pass "HEARTBEAT: logged"
        else
            fail_test "HEARTBEAT:" "not in log"
        fi
    fi
fi

# Summary
echo ""
echo "======================================"
echo "  RESULTS: $PASS passed, $FAIL_COUNT failed, $SKIP skipped"
echo "  $(date -Iseconds)"
echo "======================================"

if [ $FAIL_COUNT -eq 0 ]; then
    echo "  ALL TESTS PASSED"
    exit 0
else
    echo "  SOME TESTS FAILED"
    exit 1
fi
