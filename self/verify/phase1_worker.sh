#!/bin/bash
# verify_phase1_worker.sh
#
# End-to-end verification that Phase 1 (archetype=worker + transfer_paths
# + seeded state) works as designed. Exits 0 on pass, 1 on fail.
#
# Asserts, after a fresh spawn of a test peer:
#  1. peers.json has the peer with status=ready
#  2. Transferred path (/home/seal/.neil/projects/humanizer) landed at
#     /home/neil/.neil/projects/humanizer with SPEC.md readable
#  3. intentions.json on peer is NON-EMPTY and contains a structured
#     entry with our initial_intention marker string
#  4. heartbeat_log.json on peer has >=1 entry with prompt=peer_kickoff
#  5. spawn_config.json on peer has archetype=worker
#  6. ready.md exists on peer and mentions SPEC.md's first H1 line
#
# Cleans up the test peer at the end regardless of pass/fail.

set -u
NAME="verify-phase1-$$"
PEER_HOME=/home/neil
KEY=~/.neil/keys/peer_ed25519
MARKER="VERIFY_PHASE1_MARKER_$(date +%s)"
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

log "spawning test peer '$NAME'..."
PARAM_name="$NAME" \
PARAM_persona=worker \
PARAM_memory_mode=scoped \
PARAM_archetype=worker \
PARAM_initial_intention="$MARKER Read SPEC.md and acknowledge." \
PARAM_transfer_paths="/home/seal/.neil/projects/humanizer" \
NEIL_ACTION=create \
bash ~/.neil/tools/spawn_vm/spawn_vm.sh 2>&1 | tee /tmp/verify_phase1_spawn.log \
    | grep -E "(transfer_paths|kickoff|READY|FAIL)" || true

IP=$(python3 -c "
import json, sys
try: d = json.load(open('/home/seal/.neil/state/peers.json'))
except: sys.exit(1)
p = d.get('$NAME', {})
if p.get('status') == 'ready': print(p.get('ip', ''))
")
if [ -z "$IP" ]; then
    log "FAIL  peer did not reach status=ready"; exit 1
fi
log "peer ready at $IP"

# 1. Peer in registry
assert "peers.json has $NAME with status=ready" "[ -n \"$IP\" ]"

# 2. Transferred path landed
assert "projects/humanizer dir exists on peer" \
       "peer_sh '[ -d $PEER_HOME/.neil/projects/humanizer ]'"
assert "SPEC.md is readable on peer" \
       "peer_sh '[ -r $PEER_HOME/.neil/projects/humanizer/SPEC.md ]'"
assert "author_corpus/mamishev_clean.jsonl readable on peer" \
       "peer_sh '[ -r $PEER_HOME/.neil/projects/humanizer/author_corpus/mamishev_clean.jsonl ]'"

# 3. intentions.json seeded
INT_COUNT=$(peer_sh "python3 -c 'import json; print(len(json.load(open(\"$PEER_HOME/.neil/state/intentions.json\"))))'" 2>/dev/null)
assert "intentions.json has >=1 entry (got: ${INT_COUNT:-0})" \
       "[ \"${INT_COUNT:-0}\" -ge 1 ]"

INT_HAS_MARKER=$(peer_sh "grep -c '$MARKER' $PEER_HOME/.neil/state/intentions.json" 2>/dev/null || echo 0)
assert "intentions.json contains our initial_intention marker" \
       "[ \"${INT_HAS_MARKER:-0}\" -ge 1 ]"

# 4. heartbeat_log seeded with kickoff
HB_COUNT=$(peer_sh "python3 -c 'import json; print(len(json.load(open(\"$PEER_HOME/.neil/state/heartbeat_log.json\"))))'" 2>/dev/null)
assert "heartbeat_log.json has >=1 entry (got: ${HB_COUNT:-0})" \
       "[ \"${HB_COUNT:-0}\" -ge 1 ]"

HB_HAS_KICKOFF=$(peer_sh "grep -c 'peer_kickoff' $PEER_HOME/.neil/state/heartbeat_log.json" 2>/dev/null || echo 0)
assert "heartbeat_log.json contains peer_kickoff beat" \
       "[ \"${HB_HAS_KICKOFF:-0}\" -ge 1 ]"

# 5. spawn_config has archetype
ARCH=$(peer_sh "python3 -c 'import json; print(json.load(open(\"$PEER_HOME/.neil/state/spawn_config.json\")).get(\"archetype\", \"\"))'" 2>/dev/null)
assert "spawn_config.json has archetype=worker (got: ${ARCH:-none})" \
       "[ \"${ARCH:-}\" = 'worker' ]"

# 6. ready.md written with SPEC.md evidence
assert "ready.md exists on peer" \
       "peer_sh '[ -s $PEER_HOME/.neil/state/ready.md ]'"

RM_HAS_SPEC=$(peer_sh "grep -c 'Humanizer' $PEER_HOME/.neil/state/ready.md" 2>/dev/null || echo 0)
assert "ready.md mentions 'Humanizer' (SPEC.md H1 evidence)" \
       "[ \"${RM_HAS_SPEC:-0}\" -ge 1 ]"

echo
log "── Summary ──"
log "PASS: $PASS"
log "FAIL: $FAIL"
if [ $FAIL -eq 0 ]; then
    log "VERDICT: [PASS] Phase 1 (worker archetype + transfer_paths + seeded state) verified"
    exit 0
else
    log "VERDICT: [FAIL] see failures above"
    exit 1
fi
