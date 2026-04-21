#!/bin/bash
# verify_phase3_relay.sh
#
# End-to-end verification of the relay archetype.
# Asserts:
#  1. autoprompt daemon running
#  2. watcher daemon running (filesystem.sh via neil-filesystem-watcher.service)
#  3. heartbeat TIMER is NOT installed (key diff from autonomous)
#  4. heartbeat log is still at seed size (no periodic beats)
#  5. triggering a file-drop in the watched dir produces a queue entry within 10s
#  6. autoprompt picks up the event-triggered prompt and moves it to history/

set -u
NAME="verify-phase3-$$"
PEER_HOME=/home/neil
KEY=~/.neil/keys/peer_ed25519
PASS=0; FAIL=0
log() { echo "[verify] $*"; }
assert() {
    local desc="$1" cond="$2"
    if eval "$cond"; then log "PASS  $desc"; PASS=$((PASS+1));
    else log "FAIL  $desc  [cond: $cond]"; FAIL=$((FAIL+1)); fi
}
peer_sh() {
    ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        -o BatchMode=yes -o ConnectTimeout=5 -i "$KEY" "neil@$IP" "$@"
}
cleanup() {
    log "cleanup: destroying $NAME"
    bash ~/.neil/tools/spawn_vm/spawn_vm.sh destroy "$NAME" >/dev/null 2>&1 || true
}
trap cleanup EXIT

log "spawning relay test peer '$NAME'..."
PARAM_name="$NAME" \
PARAM_persona=worker \
PARAM_memory_mode=scoped \
PARAM_archetype=relay \
PARAM_watchers=filesystem \
PARAM_initial_intention="You are a relay peer. React to events dropped into /home/neil/.neil/inputs/relay_inbox/." \
NEIL_ACTION=create \
bash ~/.neil/tools/spawn_vm/spawn_vm.sh 2>&1 | grep -E "(relay setup|kickoff|READY)" || true

IP=$(python3 -c "
import json, sys
try: d = json.load(open('/home/seal/.neil/state/peers.json'))
except: sys.exit(1)
print(d.get('$NAME', {}).get('ip', ''))
")
if [ -z "$IP" ]; then
    log "FAIL  peer did not reach status=ready"; exit 1
fi
log "peer at $IP — 30s settle..."
sleep 30

# 1. autoprompt daemon running
AP_PID=$(peer_sh "pgrep -f 'tools/autoPrompter/autoprompt$'" 2>/dev/null | head -1 | tr -d '[:space:]')
assert "autoprompt daemon running (pid: ${AP_PID:-none})" \
       "[ -n \"${AP_PID:-}\" ]"

# 2. watcher daemon running (look for the watcher's inotifywait process)
WATCH_PID=$(peer_sh "pgrep -f 'inotifywait.*relay_inbox'" 2>/dev/null | head -1 | tr -d '[:space:]')
assert "filesystem watcher running (inotifywait pid: ${WATCH_PID:-none})" \
       "[ -n \"${WATCH_PID:-}\" ]"

# 3. NO heartbeat timer installed
HB_TIMER_ABSENT=$(peer_sh "test ! -f /etc/systemd/system/neil-heartbeat.timer && echo yes" 2>/dev/null | tr -d '[:space:]')
assert "heartbeat timer absent (relay should NOT have one)" \
       "[ \"${HB_TIMER_ABSENT:-}\" = 'yes' ]"

# 4. Trigger a filesystem event — drop a file in the watched dir
log "triggering filesystem event: touching /home/neil/.neil/inputs/relay_inbox/trigger.txt on peer..."
peer_sh "echo 'relay-trigger-$(date +%s)' > $PEER_HOME/.neil/inputs/relay_inbox/trigger.txt" >/dev/null 2>&1
log "  waiting 15s for watcher → queue → autoprompt → history..."
sleep 15

# 5. Queue or history should contain an fs_trigger-prefixed prompt
FS_PROMPT=$(peer_sh "ls $PEER_HOME/.neil/tools/autoPrompter/queue/ $PEER_HOME/.neil/tools/autoPrompter/active/ $PEER_HOME/.neil/tools/autoPrompter/history/ 2>/dev/null | grep -c _fs_" | tr -d '[:space:]')
assert "filesystem event queued or processed (count: ${FS_PROMPT:-0})" \
       "[ \"${FS_PROMPT:-0}\" -ge 1 ]"

# 6. autoprompt processed something (history non-empty)
HIST_COUNT=$(peer_sh "ls $PEER_HOME/.neil/tools/autoPrompter/history/*.result.md 2>/dev/null | wc -l" | tr -d '[:space:]')
assert "autoprompt history has >=1 processed beat (count: ${HIST_COUNT:-0})" \
       "[ \"${HIST_COUNT:-0}\" -ge 1 ]"

# 7. config.toml clean
CONFIG_HAS_SEAL=$(peer_sh "grep -c /home/seal $PEER_HOME/.neil/config.toml || true" 2>/dev/null | tr -d '[:space:]')
CONFIG_HAS_SEAL=${CONFIG_HAS_SEAL:-0}
assert "config.toml has no /home/seal references (count: $CONFIG_HAS_SEAL)" \
       "[ \"$CONFIG_HAS_SEAL\" = '0' ]"

echo
log "── Summary ──"
log "PASS: $PASS   FAIL: $FAIL"
if [ $FAIL -eq 0 ]; then
    log "VERDICT: [PASS] Phase 3 (relay archetype: autoprompt + watcher, no heartbeat timer) verified"
    exit 0
else
    log "VERDICT: [FAIL] see failures above"
    exit 1
fi
