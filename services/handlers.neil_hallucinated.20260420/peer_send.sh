#!/bin/bash
# peer_send dispatcher -- queue a prompt file to a peer Neil via SSH
# Matches spawn_vm.sh conventions: associative-array args, JSON output, exit codes.
set -e
ACTION="${1:-}"
shift || true
declare -A P
for arg in "$@"; do
    k="${arg%%=*}"; v="${arg#*=}"; P[$k]="$v"
done

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
STATE="$NEIL_HOME/state"
KEYS="$NEIL_HOME/keys"
PEERS="$STATE/peers.json"
KEY="$KEYS/peer_ed25519"
LOG="$STATE/cluster_activity.jsonl"

mkdir -p "$STATE"
[ -f "$PEERS" ] || echo "{}" > "$PEERS"

log_event() {
    printf '{"ts":"%s","action":"%s","peer":"%s","status":"%s","detail":%s}\n' \
        "$(date -Iseconds)" "$1" "$2" "$3" "$4" >> "$LOG"
}

lookup_peer() {
    local name="$1"
    python3 -c "
import json, sys
with open('$PEERS') as f: data = json.load(f)
hit = data.get('$name')
if not hit: sys.exit(1)
print(hit.get('host',''))
print(hit.get('user','seal'))
print(hit.get('queue_path','$NEIL_HOME/tools/autoPrompter/queue'))
"
}

case "$ACTION" in
    send)
        peer="${P[peer]:-}"
        message="${P[message]:-}"
        [ -z "$peer" ] && { echo '{"error":"peer required"}'; exit 2; }
        [ -z "$message" ] && { echo '{"error":"message required"}'; exit 2; }

        if ! info=$(lookup_peer "$peer" 2>/dev/null); then
            log_event "send" "$peer" "not_found" '"peer not in registry"'
            echo "{\"error\":\"peer not in registry: $peer\"}"
            exit 3
        fi
        host=$(echo "$info" | sed -n '1p')
        user=$(echo "$info" | sed -n '2p')
        qdir=$(echo "$info" | sed -n '3p')

        [ -f "$KEY" ] || { echo '{"error":"peer key missing at '"$KEY"'"}'; exit 4; }

        fname="peer_$(date +%s)_${RANDOM}_chat.md"
        tmp=$(mktemp)
        printf '%s\n' "$message" > "$tmp"

        if scp -i "$KEY" -o StrictHostKeyChecking=accept-new \
               -o BatchMode=yes -o ConnectTimeout=10 \
               "$tmp" "${user}@${host}:${qdir}/${fname}" 2>/dev/null; then
            rm -f "$tmp"
            log_event "send" "$peer" "delivered" "\"$fname\""
            printf '{"peer":"%s","host":"%s","file":"%s","status":"delivered"}\n' \
                "$peer" "$host" "$fname"
        else
            rc=$?
            rm -f "$tmp"
            log_event "send" "$peer" "scp_failed" "\"rc=$rc\""
            printf '{"peer":"%s","host":"%s","status":"scp_failed","rc":%d}\n' \
                "$peer" "$host" "$rc"
            exit 5
        fi
        ;;
    list)
        cat "$PEERS"
        ;;
    add)
        peer="${P[name]:-}"
        host="${P[host]:-}"
        user="${P[user]:-seal}"
        qpath="${P[queue_path]:-$NEIL_HOME/tools/autoPrompter/queue}"
        [ -z "$peer" ] && { echo '{"error":"name required"}'; exit 2; }
        [ -z "$host" ] && { echo '{"error":"host required"}'; exit 2; }
        python3 -c "
import json
with open('$PEERS') as f: data = json.load(f)
data['$peer'] = {'host':'$host','user':'$user','queue_path':'$qpath','added':'$(date -Iseconds)'}
with open('$PEERS','w') as f: json.dump(data, f, indent=2)
"
        log_event "add" "$peer" "registered" "\"$host\""
        printf '{"peer":"%s","host":"%s","status":"registered"}\n' "$peer" "$host"
        ;;
    remove)
        peer="${P[name]:-}"
        [ -z "$peer" ] && { echo '{"error":"name required"}'; exit 2; }
        python3 -c "
import json
with open('$PEERS') as f: data = json.load(f)
data.pop('$peer', None)
with open('$PEERS','w') as f: json.dump(data, f, indent=2)
"
        log_event "remove" "$peer" "unregistered" "null"
        printf '{"peer":"%s","status":"unregistered"}\n' "$peer"
        ;;
    ping)
        peer="${P[peer]:-}"
        [ -z "$peer" ] && { echo '{"error":"peer required"}'; exit 2; }
        if ! info=$(lookup_peer "$peer" 2>/dev/null); then
            echo "{\"error\":\"peer not in registry: $peer\"}"
            exit 3
        fi
        host=$(echo "$info" | sed -n '1p')
        user=$(echo "$info" | sed -n '2p')
        if ssh -i "$KEY" -o StrictHostKeyChecking=accept-new \
               -o BatchMode=yes -o ConnectTimeout=5 \
               "${user}@${host}" "echo pong" >/dev/null 2>&1; then
            log_event "ping" "$peer" "pong" "null"
            printf '{"peer":"%s","host":"%s","status":"pong"}\n' "$peer" "$host"
        else
            log_event "ping" "$peer" "unreachable" "null"
            printf '{"peer":"%s","host":"%s","status":"unreachable"}\n' "$peer" "$host"
            exit 5
        fi
        ;;
    *)
        echo "{\"error\":\"unknown action: $ACTION\",\"valid\":[\"send\",\"list\",\"add\",\"remove\",\"ping\"]}"
        exit 2
        ;;
esac