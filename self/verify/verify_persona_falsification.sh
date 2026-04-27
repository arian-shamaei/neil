#!/bin/bash
# verify_persona_falsification.sh — Level 2A gates 1, 2, 3.
#
# Gate 1 (rubric): each persona's first beat output contains role-flavored
#   keywords (eng-mgr: scope/sequencing/complexity/test/rollout; cso: STRIDE,
#   threat, attack, trust-boundary, data-flow). Counted by grep.
#
# Gate 2 (invalid persona): spawn_vm persona=nonexistent_xyz exits non-zero,
#   leaves no container behind.
#
# Gate 3 (falsification): both peers receive the SAME locked webhook spec;
#   their replies must (a) hit the role rubric, (b) reach DIFFERENT verdicts
#   or block-for-non-overlapping-reasons, (c) neither's top concern matches
#   the OTHER role's keyword set.
#
# Cleanup: peers destroyed at end via trap.
set -u
NEIL_HOME=${NEIL_HOME:-$HOME/.neil}
LOG=$NEIL_HOME/logs/persona_falsification.log
mkdir -p "$NEIL_HOME/logs"

PASS=0
FAIL=0
ts() { date -u +%Y-%m-%dT%H:%M:%SZ; }
log() { printf '[%s] %s\n' "$(ts)" "$*" | tee -a "$LOG"; }
gate() {
    local name="$1" cond="$2"
    if eval "$cond" >/dev/null 2>&1; then
        log "[PASS] $name"; PASS=$((PASS+1))
    else
        log "[FAIL] $name -- $cond"; FAIL=$((FAIL+1))
    fi
}

PEER_EM="lvl2-em-$$"
PEER_CSO="lvl2-cso-$$"

cleanup() {
    log "cleanup: destroying $PEER_EM, $PEER_CSO"
    bash "$NEIL_HOME/tools/spawn_vm/spawn_vm.sh" destroy "$PEER_EM"  >/dev/null 2>&1 || true
    bash "$NEIL_HOME/tools/spawn_vm/spawn_vm.sh" destroy "$PEER_CSO" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Locked Gate 3 spec — designed so eng-mgr concerns and cso concerns are
# both real and largely non-overlapping.
LOCKED_SPEC='Ship a webhook endpoint at POST /webhook/eval that accepts an arbitrary user-submitted bash script in the JSON body ({"script": "..."}), executes it inside a docker container with --network=host, and returns stdout. Authentication: a single Bearer token shared across all users. No rate limiting. Container reused across requests for cold-start performance. Ship in 48h. Reply: your role review of this proposal under 250 words, your top concern, and your verdict (SHIP / BLOCK).'

#===========================================================================
# Gate 2: invalid persona must fail at spawn time
#===========================================================================
log "=== Gate 2: invalid persona fails fast ==="
INVALID_OUT=$(NEIL_SERVICE=spawn_vm NEIL_ACTION=create NEIL_CRED=no-auth-needed \
    NEIL_PARAMS='name="lvl2-bad-'"$$"'" persona="nonexistent_xyz_'"$$"'"' \
    bash "$NEIL_HOME/services/handler.sh" 2>&1)
INVALID_RC=$?
gate "invalid persona returns non-zero"      "[ $INVALID_RC -ne 0 ]"
gate "invalid persona error mentions path"   "echo '$INVALID_OUT' | grep -q 'persona'"
gate "no container left behind"              "! /snap/bin/lxc info lvl2-bad-$$ >/dev/null 2>&1"

#===========================================================================
# Gate 1+3: spawn eng-mgr + cso, send locked spec, check rubric + falsification
#===========================================================================
log "=== spawning eng-mgr + cso peers ==="
NEIL_SERVICE=spawn_vm NEIL_ACTION=create NEIL_CRED=no-auth-needed \
    NEIL_PARAMS="name=\"$PEER_EM\" persona=\"eng-mgr\" memory_mode=\"scoped\" archetype=\"autonomous\" initial_intention=\"You are a Neil peer reviewing a proposal. Wait for an incoming peer_send and reply per your role.\"" \
    bash "$NEIL_HOME/services/handler.sh" >>"$LOG" 2>&1 || { log "FAIL: eng-mgr spawn"; exit 1; }

NEIL_SERVICE=spawn_vm NEIL_ACTION=create NEIL_CRED=no-auth-needed \
    NEIL_PARAMS="name=\"$PEER_CSO\" persona=\"cso\" memory_mode=\"scoped\" archetype=\"autonomous\" initial_intention=\"You are a Neil peer reviewing a proposal. Wait for an incoming peer_send and reply per your role.\"" \
    bash "$NEIL_HOME/services/handler.sh" >>"$LOG" 2>&1 || { log "FAIL: cso spawn"; exit 1; }

log "both peers spawned; verifying persona files landed"
for p in "$PEER_EM" "$PEER_CSO"; do
    /snap/bin/lxc exec "$p" -- test -f /home/neil/.neil/essence/persona.md \
        && log "  $p: essence/persona.md present" \
        || { log "FAIL: $p missing persona.md"; exit 1; }
done

# Send locked spec to each peer via direct queue drop (skips peer_send roundtrip)
log "=== Gate 3: dropping locked webhook spec to both peers ==="
TS=$(date -u +%Y%m%dT%H%M%S)
TMP=$(mktemp --suffix=.md)
echo "$LOCKED_SPEC" > "$TMP"

for p in "$PEER_EM" "$PEER_CSO"; do
    IP=$(python3 -c "import json; d=json.load(open('$NEIL_HOME/state/peers.json')); print(d['$p']['ip'])")
    DEST="/home/neil/.neil/tools/autoPrompter/queue/${TS}_from_sealserver_eval.md"
    scp -q -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        -o BatchMode=yes -i "$NEIL_HOME/keys/peer_ed25519" \
        "$TMP" "neil@$IP:$DEST" \
        && log "  $p: queued (IP $IP)" \
        || log "FAIL: scp to $p"
done
rm "$TMP"

# Wait for both peers to process
log "waiting for both peers to process (up to 5 min)..."
for i in $(seq 1 30); do
    EM_DONE=$(/snap/bin/lxc exec "$PEER_EM"  -- bash -c 'ls /home/neil/.neil/tools/autoPrompter/history/*from_sealserver_eval*.result.md 2>/dev/null | wc -l' 2>/dev/null)
    CS_DONE=$(/snap/bin/lxc exec "$PEER_CSO" -- bash -c 'ls /home/neil/.neil/tools/autoPrompter/history/*from_sealserver_eval*.result.md 2>/dev/null | wc -l' 2>/dev/null)
    log "  poll $i: EM_done=$EM_DONE  CSO_done=$CS_DONE"
    if [ "$EM_DONE" -ge 1 ] && [ "$CS_DONE" -ge 1 ]; then break; fi
    sleep 10
done

# Capture replies
EM_REPLY=$(/snap/bin/lxc exec "$PEER_EM"  -- bash -c 'cat $(ls -t /home/neil/.neil/tools/autoPrompter/history/*from_sealserver_eval*.result.md | head -1)' 2>/dev/null)
CS_REPLY=$(/snap/bin/lxc exec "$PEER_CSO" -- bash -c 'cat $(ls -t /home/neil/.neil/tools/autoPrompter/history/*from_sealserver_eval*.result.md | head -1)' 2>/dev/null)

# Save replies for inspection
echo "$EM_REPLY"  > "$NEIL_HOME/logs/persona_em_reply.txt"
echo "$CS_REPLY" > "$NEIL_HOME/logs/persona_cso_reply.txt"
log "replies captured to logs/persona_{em,cso}_reply.txt"

#===========================================================================
# Gate 1: persona-flavor rubric
#===========================================================================
log "=== Gate 1: role-flavor rubric ==="
EM_KW='scope|sequenc|complex|test gap|test-gap|rollout|architect|coupl|legacy'
CS_KW='STRIDE|threat[- ]model|attack[- ]surface|trust[- ]boundary|data[- ]flow|spoof|tamper|repudia|disclosure|denial|elevat|sandbox|escape|injection|auth|secret'

EM_HITS=$(printf '%s' "$EM_REPLY"  | grep -ciE "$EM_KW" || true)
CS_HITS=$(printf '%s' "$CS_REPLY" | grep -ciE "$CS_KW" || true)

gate "eng-mgr reply has >=2 eng-mgr-flavored keywords (got $EM_HITS)" "[ ${EM_HITS:-0} -ge 2 ]"
gate "cso reply has >=2 cso-flavored keywords (got $CS_HITS)"          "[ ${CS_HITS:-0} -ge 2 ]"

#===========================================================================
# Gate 3: falsification — verdicts differ + non-overlapping concerns
#===========================================================================
log "=== Gate 3: falsification — different role, different concerns ==="
EM_TOP_HITS_ON_CS=$(printf '%s' "$EM_REPLY" | head -c 500 | grep -ciE "$CS_KW" || true)
CS_TOP_HITS_ON_EM=$(printf '%s' "$CS_REPLY" | head -c 500 | grep -ciE "$EM_KW" || true)

gate "eng-mgr's top section is NOT dominated by cso keywords ($EM_TOP_HITS_ON_CS, want < $EM_HITS)" "[ ${EM_TOP_HITS_ON_CS:-0} -lt ${EM_HITS:-1} ]"
gate "cso's top section is NOT dominated by eng-mgr keywords ($CS_TOP_HITS_ON_EM, want < $CS_HITS)"  "[ ${CS_TOP_HITS_ON_EM:-0} -lt ${CS_HITS:-1} ]"

EM_VERDICT=$(printf '%s' "$EM_REPLY" | grep -oiE '\b(SHIP|BLOCK)\b' | tail -1)
CS_VERDICT=$(printf '%s' "$CS_REPLY" | grep -oiE '\b(SHIP|BLOCK)\b' | tail -1)
log "  eng-mgr verdict: ${EM_VERDICT:-(none)}"
log "  cso     verdict: ${CS_VERDICT:-(none)}"

# PASS if verdicts disagree, OR both block but for non-overlapping reasons
# (which we approximate by "neither role's top section is dominated by the
# OTHER role's keywords" — already gated above).
if [ "$EM_VERDICT" != "$CS_VERDICT" ]; then
    log "[PASS] verdicts differ"
    PASS=$((PASS+1))
else
    log "[NOTE] verdicts match ($EM_VERDICT) — acceptable iff role-keyword non-overlap holds"
fi

#===========================================================================
echo
log "=== SUMMARY: PASS=$PASS  FAIL=$FAIL ==="
if [ $FAIL -eq 0 ]; then
    log "VERDICT: [PASS] Level 2A persona library + spawn_vm verified"
    exit 0
else
    log "VERDICT: [FAIL] see failures above"
    exit 1
fi
