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
            -i "$KEY_PRIV" "root@$ip" true 2>/dev/null && return 0
        sleep 1
    done
    return 1
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

    log "installing sshd + python3..."
    LXC exec "$name" -- bash -c '
        export DEBIAN_FRONTEND=noninteractive
        apt-get update -qq >/dev/null 2>&1
        apt-get install -y -qq openssh-server python3 python3-venv python3-pip >/dev/null 2>&1
        systemctl enable --now ssh >/dev/null 2>&1
    '

    log "injecting Neil pubkey..."
    LXC file push "$KEY_PUB" "$name/root/.ssh/authorized_keys" \
        --mode 0600 --create-dirs >/dev/null
    LXC exec "$name" -- chmod 700 /root/.ssh

    log "waiting for sshd..."
    wait_for_ssh "$ip" || {
        registry_set "$name" "$ip" "$IMAGE" "ssh-timeout"
        die "sshd didn't come up"
    }

    registry_set "$name" "$ip" "$IMAGE" "ready"
    log "READY  $name  ip=$ip  ssh -i $KEY_PRIV root@$ip"
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
