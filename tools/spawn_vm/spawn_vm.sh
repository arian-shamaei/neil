#!/bin/sh
# spawn_vm.sh -- dispatch for the spawn_vm service (Phase 2 scaffold).
#
# Env vars (set by handler.sh):
#   NEIL_ACTION   create | destroy | list | status
#   PARAM_*       action parameters (provider, size, region, name, id, dry_run)
#
# Phase 2 implements dry-run only. Phase 3+ will add real provider
# adapters (Hetzner/DO/Lambda) and a remote-bootstrap installer.

set -e

CLUSTER_DIR="${NEIL_HOME:-$HOME/.neil}/cluster"
NODES_DIR="$CLUSTER_DIR/nodes"
mkdir -p "$NODES_DIR"

DRY_RUN="${PARAM_dry_run:-1}"

iso_now() { date -u +%Y-%m-%dT%H:%M:%SZ; }

action_create() {
    PROVIDER="${PARAM_provider:-}"
    SIZE="${PARAM_size:-small}"
    REGION="${PARAM_region:-default}"
    NAME="${PARAM_name:-neil-child-$(date +%s)}"

    if [ -z "$PROVIDER" ]; then
        echo "ERROR: spawn_vm create requires provider="
        exit 1
    fi

    if [ "$DRY_RUN" != "1" ]; then
        echo "ERROR: real provisioning (dry_run=0) requires operator approval; refusing in Phase 2"
        exit 1
    fi

    ID="dryrun-$(date +%s)-$$"
    CREATED="$(iso_now)"
    REC="$NODES_DIR/$ID.json"

    cat > "$REC" <<EOF
{
  "id": "$ID",
  "provider": "dryrun",
  "name": "$NAME",
  "transport": {
    "kind": "ssh",
    "endpoint": "user@dryrun.example",
    "queue_path": "/home/seal/.neil/tools/autoPrompter/queue"
  },
  "status": "ready",
  "ip": null,
  "size": "$SIZE",
  "region": "$REGION",
  "created": "$CREATED",
  "last_heartbeat": null
}
EOF
    cat "$REC"
}

action_destroy() {
    ID="${PARAM_id:-}"
    if [ -z "$ID" ]; then
        echo "ERROR: spawn_vm destroy requires id="
        exit 1
    fi
    REC="$NODES_DIR/$ID.json"
    if [ ! -f "$REC" ]; then
        echo "ERROR: no cluster record for id=$ID"
        exit 1
    fi
    if [ "$DRY_RUN" != "1" ] && [ "${ID#dryrun-}" = "$ID" ]; then
        echo "ERROR: real destroy (dry_run=0) requires operator approval; refusing in Phase 2"
        exit 1
    fi
    rm -f "$REC"
    echo "{\"id\":\"$ID\",\"status\":\"destroyed\"}"
}

action_list() {
    echo "["
    first=1
    for f in "$NODES_DIR"/*.json; do
        [ -f "$f" ] || continue
        if [ $first -eq 1 ]; then first=0; else echo ","; fi
        cat "$f"
    done
    echo "]"
}

action_status() {
    ID="${PARAM_id:-}"
    if [ -z "$ID" ]; then
        echo "ERROR: spawn_vm status requires id="
        exit 1
    fi
    REC="$NODES_DIR/$ID.json"
    if [ ! -f "$REC" ]; then
        echo "ERROR: no cluster record for id=$ID"
        exit 1
    fi
    cat "$REC"
}

case "$NEIL_ACTION" in
    create)  action_create ;;
    destroy) action_destroy ;;
    list)    action_list ;;
    status)  action_status ;;
    *)
        echo "ERROR: unknown action for spawn_vm: $NEIL_ACTION"
        exit 1
        ;;
esac
