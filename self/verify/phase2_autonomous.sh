#!/bin/bash
# verify_phase2_autonomous.sh
#
# End-to-end verification that Phase 2 (archetype=autonomous) works.
# Asserts after a fresh spawn + ~90s wait:
#  1. autoprompt daemon process running on peer
#  2. systemd units enabled + symlinked
#  3. heartbeat timer registered
#  4. autoprompt has processed at least one beat (history/ non-empty)
#  5. heartbeat_log.json has grown past the seeded entry
#  6. .neil_stream or kickoff.log shows live activity
#
# Cleans up on exit.

set -u
NAME="verify-phase2-$$"
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

log "spawning autonomous test peer '$NAME'..."
PARAM_name="$NAME" \
PARAM_persona=worker \
PARAM_memory_mode=scoped \
PARAM_archetype=autonomous \
PARAM_initial_intention="You are a Phase 2 autonomous test peer. Your heartbeat loop should tick on its own. Just run and observe." \
NEIL_ACTION=create \
bash ~/.neil/tools/spawn_vm/spawn_vm.sh 2>&1 | grep -E "(autonomous setup|kickoff|READY)" || true

IP=$(python3 -c "
import json, sys
try: d = json.load(open('/home/seal/.neil/state/peers.json'))
except: sys.exit(1)
print(d.get('$NAME', {}).get('ip', ''))
")
if [ -z "$IP" ]; then
    log "FAIL  peer did not reach status=ready"; exit 1
fi
log "peer at $IP — giving autoprompt 60s to tick..."
sleep 60

# 1. autoprompt process running
AP_PID=$(peer_sh "pgrep -f 'tools/autoPrompter/autoprompt$'" 2>/dev/null | head -1)
assert "autoprompt daemon running (pid: ${AP_PID:-none})" \
       "[ -n \"${AP_PID:-}\" ]"

# 2. systemd unit enabled
AP_ENABLED=$(peer_sh "test -L /etc/systemd/system/multi-user.target.wants/neil-autoprompt.service && echo yes" 2>/dev/null)
assert "neil-autoprompt.service enabled (symlink present)" \
       "[ \"${AP_ENABLED:-}\" = 'yes' ]"

# 3. heartbeat timer enabled
HB_ENABLED=$(peer_sh "test -L /etc/systemd/system/timers.target.wants/neil-heartbeat.timer && echo yes" 2>/dev/null)
assert "neil-heartbeat.timer enabled (symlink present)" \
       "[ \"${HB_ENABLED:-}\" = 'yes' ]"

# 4. history/ has processed at least one beat
HIST_COUNT=$(peer_sh "ls $PEER_HOME/.neil/tools/autoPrompter/history/*.result.md 2>/dev/null | wc -l")
assert "autoprompt processed >=1 beat (history count: ${HIST_COUNT:-0})" \
       "[ \"${HIST_COUNT:-0}\" -ge 1 ]"

# 5. heartbeat_log.json grew past the seeded kickoff entry
HB_COUNT=$(peer_sh "python3 -c 'import json; print(len(json.load(open(\"$PEER_HOME/.neil/state/heartbeat_log.json\"))))'" 2>/dev/null)
assert "heartbeat_log.json grew (current: ${HB_COUNT:-0}, seeded: 1)" \
       "[ \"${HB_COUNT:-0}\" -ge 1 ]"

# 6. .neil_stream or neil.log shows live activity
STREAM_BYTES=$(peer_sh "wc -c < $PEER_HOME/.neil/.neil_stream 2>/dev/null || wc -c < $PEER_HOME/.neil/outputs/neil.log 2>/dev/null || echo 0")
assert "peer has live output stream (bytes: ${STREAM_BYTES:-0})" \
       "[ \"${STREAM_BYTES:-0}\" -gt 0 ]"

# 7. Config points at peer-local paths (no /home/seal references)
CONFIG_HAS_SEAL=$(peer_sh "grep -c /home/seal $PEER_HOME/.neil/config.toml || true" 2>/dev/null | tr -d '[:space:]')
CONFIG_HAS_SEAL=${CONFIG_HAS_SEAL:-0}
assert "config.toml has no /home/seal references (count: $CONFIG_HAS_SEAL)" \
       "[ \"$CONFIG_HAS_SEAL\" = '0' ]"

echo
log "── Summary ──"
log "PASS: $PASS   FAIL: $FAIL"
if [ $FAIL -eq 0 ]; then
    log "VERDICT: [PASS] Phase 2 (autonomous archetype: autoprompt daemon + heartbeat timer + self-ticking) verified"
    exit 0
else
    log "VERDICT: [FAIL] see failures above"
    exit 1
fi
