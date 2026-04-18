#!/bin/bash
# spawn_vm dispatcher -- skeleton / dry-run only
set -e
ACTION="${1:-}"
shift || true
declare -A P
for arg in "$@"; do
    k="${arg%%=*}"; v="${arg#*=}"; P[$k]="$v"
done
CLUSTER="${NEIL_HOME:-$HOME/.neil}/cluster"
INSTANCES="$CLUSTER/instances.json"
mkdir -p "$CLUSTER"
[ -f "$INSTANCES" ] || echo "[]" > "$INSTANCES"
synth_id() { echo "dry-$(echo -n "$1" | md5sum | cut -c1-12)"; }
case "$ACTION" in
    create)
        name="${P[name]:-}"
        [ -z "$name" ] && { echo '{"error":"name required"}'; exit 2; }
        dry="${P[dry_run]:-1}"
        provider="${P[provider]:-hetzner}"
        size="${P[size]:-cx22}"
        region="${P[region]:-hel1}"
        id="$(synth_id "$name")"
        if [ "$dry" = "1" ]; then
            ip="10.0.0.$((RANDOM % 254 + 1))"
            status="dry-provisioned"
        else
            echo '{"error":"live provisioning not yet wired; set dry_run=1"}'
            exit 3
        fi
        record=$(printf '{"id":"%s","name":"%s","provider":"%s","size":"%s","region":"%s","ip":"%s","status":"%s","created":"%s","last_heartbeat":null}' \
            "$id" "$name" "$provider" "$size" "$region" "$ip" "$status" "$(date -Iseconds)")
        python3 -c "
import json
with open('$INSTANCES') as f: data = json.load(f)
data.append(json.loads('''$record'''))
with open('$INSTANCES','w') as f: json.dump(data, f, indent=2)
" 2>/dev/null || true
        echo "$record"
        ;;
    destroy)
        id="${P[id]:-}"
        name="${P[name]:-}"
        [ -z "$id" ] && [ -z "$name" ] && { echo '{"error":"id or name required"}'; exit 2; }
        [ -z "$id" ] && id="$(synth_id "$name")"
        python3 -c "
import json
with open('$INSTANCES') as f: data = json.load(f)
data = [x for x in data if x.get('id') != '$id' and x.get('name') != '$name']
with open('$INSTANCES','w') as f: json.dump(data, f, indent=2)
" 2>/dev/null || true
        printf '{"id":"%s","status":"destroyed"}\n' "$id"
        ;;
    list)
        cat "$INSTANCES"
        ;;
    status)
        id="${P[id]:-}"
        name="${P[name]:-}"
        python3 -c "
import json
with open('$INSTANCES') as f: data = json.load(f)
hit = next((x for x in data if x.get('id')=='$id' or x.get('name')=='$name'), None)
print(json.dumps(hit) if hit else '{\"error\":\"not found\"}')
"
        ;;
    *)
        echo "{\"error\":\"unknown action: $ACTION\",\"valid\":[\"create\",\"destroy\",\"list\",\"status\"]}"
        exit 2
        ;;
esac