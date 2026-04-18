#!/bin/bash
# spawn_vm -- self-healing autonomous VM spawner.
#
# First call sets up LXD + keypair + layout (needs sudo once). Every call
# after is fast. No separate installer, no placeholders, no re-login needed.
#
#   spawn_vm create  <name>    launch + bootstrap + register
#   spawn_vm destroy <name>    stop + delete + deregister
#   spawn_vm list              show registered peers
#   spawn_vm status  <name>    liveness + IP + registry entry

set -e

# Snap binaries aren't on PATH in non-login / non-interactive shells.
export PATH="/snap/bin:$PATH"

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
PEERS_JSON="$NEIL_HOME/state/peers.json"
KEY_PRIV="$NEIL_HOME/keys/peer_ed25519"
KEY_PUB="$NEIL_HOME/keys/peer_ed25519.pub"
IMAGE="${NEIL_VM_IMAGE:-ubuntu:24.04}"

# Peer Neils must NOT run as root: claude-agent-sdk refuses
# --dangerously-skip-permissions with root EUID. Always provision a
# normal user inside the container.
PEER_USER="neil"
PEER_HOME="/home/$PEER_USER"

die() { echo "[spawn_vm] ERROR: $*" >&2; exit 1; }
log() { echo "[spawn_vm] $*"; }

# Call lxc. Root always has socket access; otherwise wrap in sg lxd so
# group membership takes effect without re-login.
LXC() {
    if [ "$EUID" -eq 0 ] || id -nG 2>/dev/null | grep -qw lxd; then
        lxc "$@"
    else
        sg lxd -c "lxc $(printf '%q ' "$@")"
    fi
}

ensure_ready() {
    if ! command -v lxc >/dev/null 2>&1; then
        log "first run: installing LXD (sudo)..."
        sudo snap install lxd
    fi

    if ! id -nG "$USER" 2>/dev/null | grep -qw lxd; then
        log "first run: adding $USER to lxd group..."
        sudo usermod -aG lxd "$USER"
    fi

    if ! sudo lxc network show lxdbr0 >/dev/null 2>&1; then
        log "first run: lxd init --auto..."
        sudo lxd init --auto
    fi

    mkdir -p "$NEIL_HOME/keys" "$NEIL_HOME/state"

    if [ ! -f "$KEY_PRIV" ]; then
        log "first run: generating peer keypair..."
        ssh-keygen -t ed25519 -N "" -C "neil-peer@$(hostname)" -f "$KEY_PRIV" >/dev/null
        chmod 600 "$KEY_PRIV"
    fi

    [ -f "$PEERS_JSON" ] || echo '{}' > "$PEERS_JSON"
}

registry_set() {
    local name="$1" ip="$2" image="$3" status="$4"
    python3 - <<PY
import json, datetime, pathlib
p = pathlib.Path("$PEERS_JSON")
d = json.loads(p.read_text() or "{}")
d["$name"] = {
    "ip": "$ip", "image": "$image", "status": "$status",
    "created_at": datetime.datetime.utcnow().isoformat() + "Z",
}
p.write_text(json.dumps(d, indent=2))
PY
}

registry_del() {
    local name="$1"
    python3 - <<PY
import json, pathlib
p = pathlib.Path("$PEERS_JSON")
d = json.loads(p.read_text() or "{}")
d.pop("$name", None)
p.write_text(json.dumps(d, indent=2))
PY
}

wait_for_ip() {
    local name="$1" ip
    for _ in $(seq 1 30); do
        ip=$(LXC list "$name" -c4 --format csv 2>/dev/null | awk '{print $1}')
        if [ -n "$ip" ]; then echo "$ip"; return 0; fi
        sleep 1
    done
    return 1
}

wait_for_ssh() {
    local ip="$1"
    for _ in $(seq 1 30); do
        ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
            -o BatchMode=yes -o ConnectTimeout=2 \
            -i "$KEY_PRIV" "$PEER_USER@$ip" true 2>/dev/null && return 0
        sleep 1
    done
    return 1
}

push_substrate() {
    # Phase 4: push a COMPLETE Neil substrate into the container so the peer
    # can run its own heartbeat loop -- same architecture as parent, just
    # configured for its role.  All files land under $PEER_HOME/.neil.
    local name="$1"
    local blueprint_bin="$HOME/.local/bin/neil-blueprint"
    [ -x "$blueprint_bin" ] || blueprint_bin="$NEIL_HOME/blueprint/target/release/neil-blueprint"

    LXC exec "$name" -- bash -c "
        mkdir -p $PEER_HOME/.neil/state \
                 $PEER_HOME/.neil/bin \
                 $PEER_HOME/.neil/essence \
                 $PEER_HOME/.neil/tools/autoPrompter/agent \
                 $PEER_HOME/.neil/services/registry \
                 $PEER_HOME/.claude
    "

    # 1. Blueprint binary (host-level location, readable by all)
    if [ -x "$blueprint_bin" ]; then
        LXC file push "$blueprint_bin" "$name/usr/local/bin/neil-blueprint" \
            --mode 0755 >/dev/null 2>&1 || true
    fi

    # 2. Essence
    if [ -d "$NEIL_HOME/essence" ]; then
        for f in "$NEIL_HOME/essence"/*.md; do
            [ -f "$f" ] || continue
            LXC file push "$f" "$name$PEER_HOME/.neil/essence/$(basename "$f")" \
                --mode 0644 >/dev/null 2>&1 || true
        done
    fi

    # 3. Agent runner
    local agent_py="$NEIL_HOME/tools/autoPrompter/agent/neil_agent.py"
    if [ -f "$agent_py" ]; then
        LXC file push "$agent_py" \
            "$name$PEER_HOME/.neil/tools/autoPrompter/agent/neil_agent.py" \
            --mode 0755 >/dev/null 2>&1 || true
    fi

    # 4. Service handler + registry
    if [ -f "$NEIL_HOME/services/handler.sh" ]; then
        LXC file push "$NEIL_HOME/services/handler.sh" \
            "$name$PEER_HOME/.neil/services/handler.sh" --mode 0755 >/dev/null 2>&1 || true
    fi
    if [ -d "$NEIL_HOME/services/registry" ]; then
        for f in "$NEIL_HOME/services/registry"/*.md; do
            [ -f "$f" ] || continue
            LXC file push "$f" \
                "$name$PEER_HOME/.neil/services/registry/$(basename "$f")" \
                --mode 0644 >/dev/null 2>&1 || true
        done
    fi

    # 5. Claude credentials
    if [ -f "$HOME/.claude/.credentials.json" ]; then
        LXC file push "$HOME/.claude/.credentials.json" \
            "$name$PEER_HOME/.claude/.credentials.json" --mode 0600 >/dev/null 2>&1 || true
    fi

    # 6. Python venv + claude-agent-sdk
    LXC exec "$name" -- bash -c "
        cd $PEER_HOME/.neil/tools/autoPrompter/agent
        if [ ! -d .venv ]; then
            python3 -m venv .venv
            .venv/bin/pip install --quiet --disable-pip-version-check claude-agent-sdk 2>&1 | tail -2
        fi
    "

    # 7. State seed + role config + .claude.json stub
    local persona="${PARAM_persona:-minimal}"
    local memory_mode="${PARAM_memory_mode:-read_only_parent}"
    local parent_node="${NEIL_NODE_ID:-$(hostname)}"
    local initial="${PARAM_initial_intention:-}"
    LXC exec "$name" -- bash -c "
        [ -f $PEER_HOME/.neil/state/intentions.json ]    || echo '[]' > $PEER_HOME/.neil/state/intentions.json
        [ -f $PEER_HOME/.neil/state/heartbeat_log.json ] || echo '[]' > $PEER_HOME/.neil/state/heartbeat_log.json
        [ -f $PEER_HOME/.neil/state/peers.json ]         || echo '{}' > $PEER_HOME/.neil/state/peers.json
        [ -f $PEER_HOME/.neil/state/proposed_memories.json ] || echo '[]' > $PEER_HOME/.neil/state/proposed_memories.json
        [ -f $PEER_HOME/.claude.json ]                   || echo '{}' > $PEER_HOME/.claude.json
        cat > $PEER_HOME/.neil/state/spawn_config.json <<EOF
{
  \"node_name\":         \"$name\",
  \"parent_node\":       \"$parent_node\",
  \"persona\":           \"$persona\",
  \"memory_mode\":       \"$memory_mode\",
  \"initial_intention\": \"$initial\",
  \"spawned_at\":        \"\$(date -u +%Y-%m-%dT%H:%M:%SZ)\"
}
EOF
        chown -R $PEER_USER:$PEER_USER $PEER_HOME
    "
}

cmd_create() {
    local name="$1"
    [ -z "$name" ] && die "usage: spawn_vm create <name>"
    LXC info "$name" >/dev/null 2>&1 && die "$name already exists"

    log "launching $name ($IMAGE)..."
    LXC launch "$IMAGE" "$name" >/dev/null

    log "waiting for network..."
    local ip; ip=$(wait_for_ip "$name") || {
        LXC delete "$name" --force >/dev/null 2>&1 || true
        die "timed out waiting for IP"
    }
    log "  $name @ $ip"

    # Register immediately as "provisioning" so the cluster panel sees
    # the peer in real time while apt/ssh setup runs.
    registry_set "$name" "$ip" "$IMAGE" "provisioning"

    log "installing sshd + python3 + user..."
    LXC exec "$name" -- bash -c "
        export DEBIAN_FRONTEND=noninteractive
        apt-get update -qq >/dev/null 2>&1
        apt-get install -y -qq openssh-server python3 python3-venv python3-pip sudo >/dev/null 2>&1
        systemctl enable --now ssh >/dev/null 2>&1
        id -u $PEER_USER >/dev/null 2>&1 || useradd -m -s /bin/bash $PEER_USER
    "

    log "injecting Neil pubkey (user=$PEER_USER)..."
    LXC file push "$KEY_PUB" "$name$PEER_HOME/.ssh/authorized_keys" \
        --mode 0600 --create-dirs >/dev/null
    LXC exec "$name" -- bash -c "
        chown -R $PEER_USER:$PEER_USER $PEER_HOME/.ssh
        chmod 700 $PEER_HOME/.ssh
    "

    log "waiting for sshd..."
    wait_for_ssh "$ip" || {
        registry_set "$name" "$ip" "$IMAGE" "ssh-timeout"
        die "sshd didn't come up"
    }

    log "pushing Neil substrate..."
    push_substrate "$name"

    registry_set "$name" "$ip" "$IMAGE" "ready"
    log "READY  $name  ip=$ip  ssh -i $KEY_PRIV $PEER_USER@$ip"
}

cmd_destroy() {
    local name="$1"
    [ -z "$name" ] && die "usage: spawn_vm destroy <name>"
    LXC info "$name" >/dev/null 2>&1 || { registry_del "$name"; die "$name does not exist"; }
    log "destroying $name..."
    LXC stop "$name" --force >/dev/null 2>&1 || true
    LXC delete "$name" >/dev/null
    registry_del "$name"
    log "gone"
}

cmd_list() {
    python3 - <<PY
import json, pathlib
p = pathlib.Path("$PEERS_JSON")
d = json.loads(p.read_text() or "{}")
if not d:
    print("(no peers registered)"); raise SystemExit
print(f"{'NAME':<20}  {'IP':<16}  {'STATUS':<12}  IMAGE")
for n, v in sorted(d.items()):
    print(f"{n:<20}  {v.get('ip','?'):<16}  {v.get('status','?'):<12}  {v.get('image','?')}")
PY
}

cmd_status() {
    local name="$1"
    [ -z "$name" ] && die "usage: spawn_vm status <name>"
    echo "--- lxc info ---"
    LXC info "$name" 2>&1 | head -15 || true
    echo "--- registry entry ---"
    python3 - <<PY
import json, pathlib
p = pathlib.Path("$PEERS_JSON")
d = json.loads(p.read_text() or "{}")
print(json.dumps(d.get("$name", {}), indent=2))
PY
}

ensure_ready

# Support env-var dispatch from handler.sh (NEIL_ACTION, PARAM_name, ...)
# as well as positional argv. If NEIL_ACTION is set and no argv passed,
# rewrite argv so both call paths share the same downstream logic.
if [ -n "${NEIL_ACTION:-}" ] && [ "$#" -eq 0 ]; then
    set -- "$NEIL_ACTION" "${PARAM_name:-}"
fi

ACTION="${1:-}"; shift || true
case "$ACTION" in
    create)  cmd_create  "$@" ;;
    destroy) cmd_destroy "$@" ;;
    list)    cmd_list        ;;
    status)  cmd_status "$@" ;;
    *) die "usage: spawn_vm {create|destroy|list|status} [name]" ;;
esac
