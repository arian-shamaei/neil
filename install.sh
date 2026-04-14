#!/bin/sh
# install.sh -- Install openclaw (Neil the SEAL) on a fresh Linux machine
# Usage: curl -sL <url>/install.sh | sh
#    or: sh install.sh [--neil-home /path] [--no-systemd] [--no-cron] [--no-blueprint]
#
# Prerequisites: gcc, python3, git, claude CLI (Anthropic)
# Optional: cargo (for blueprint TUI), rclone (for cloud mirror)

set -e

# ── Defaults ──────────────────────────────────────────────────────────
NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
INSTALL_SYSTEMD=true
INSTALL_CRON=true
INSTALL_BLUEPRINT=true
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ── Parse args ────────────────────────────────────────────────────────
while [ $# -gt 0 ]; do
    case "$1" in
        --neil-home)    NEIL_HOME="$2"; shift 2 ;;
        --no-systemd)   INSTALL_SYSTEMD=false; shift ;;
        --no-cron)      INSTALL_CRON=false; shift ;;
        --no-blueprint) INSTALL_BLUEPRINT=false; shift ;;
        -h|--help)
            echo "Usage: install.sh [--neil-home /path] [--no-systemd] [--no-cron] [--no-blueprint]"
            exit 0 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# ── Colors ────────────────────────────────────────────────────────────
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { printf "${CYAN}[openclaw]${NC} %s\n" "$1"; }
ok()    { printf "${GREEN}  [ok]${NC} %s\n" "$1"; }
warn()  { printf "${YELLOW}  [warn]${NC} %s\n" "$1"; }
fail()  { printf "${RED}  [FAIL]${NC} %s\n" "$1"; }

# ── Seal ──────────────────────────────────────────────────────────────
cat << 'SEAL'

         .-"""-.
        /        \
       |  O    O  |
       |    __    |
       |   /  \   |
        \  '=='  /
    ~~~  '-....-'  ~~~
  ~  ~  Neil the SEAL  ~  ~
     ~  openclaw v0.1  ~

SEAL

# ── Prerequisite check ───────────────────────────────────────────────
info "Checking prerequisites..."
MISSING=""

command -v gcc    >/dev/null 2>&1 || MISSING="$MISSING gcc"
command -v python3 >/dev/null 2>&1 || MISSING="$MISSING python3"
command -v git    >/dev/null 2>&1 || MISSING="$MISSING git"

if [ -n "$MISSING" ]; then
    fail "Missing required tools:$MISSING"
    echo "  Install with: sudo apt install$MISSING python3-venv"
    exit 1
fi

# Check for python3-venv
python3 -m venv --help >/dev/null 2>&1 || {
    fail "python3-venv not installed"
    echo "  Install with: sudo apt install python3-venv"
    exit 1
}

# Claude CLI -- warn but don't block (user can install later)
if ! command -v claude >/dev/null 2>&1; then
    warn "claude CLI not found. Install from: https://docs.anthropic.com/claude-code"
    warn "openclaw will install but won't run autonomously without it."
    HAS_CLAUDE=false
else
    ok "claude CLI found"
    HAS_CLAUDE=true
fi

# Cargo -- only needed for blueprint
if [ "$INSTALL_BLUEPRINT" = true ]; then
    if ! command -v cargo >/dev/null 2>&1; then
        warn "cargo not found -- skipping blueprint TUI build"
        INSTALL_BLUEPRINT=false
    fi
fi

ok "Prerequisites satisfied"

# ── Check for existing installation ──────────────────────────────────
if [ -d "$NEIL_HOME" ]; then
    if [ -f "$NEIL_HOME/essence/identity.md" ]; then
        warn "Existing installation found at $NEIL_HOME"
        printf "  Overwrite? This preserves memory/ and services/vault/. [y/N] "
        read -r REPLY
        case "$REPLY" in
            y|Y|yes) info "Upgrading..." ;;
            *)       info "Aborted."; exit 0 ;;
        esac
    fi
fi

# ── Create directory structure ────────────────────────────────────────
info "Creating directory structure at $NEIL_HOME..."

mkdir -p "$NEIL_HOME/essence"
mkdir -p "$NEIL_HOME/tools/autoPrompter/src"
mkdir -p "$NEIL_HOME/tools/autoPrompter/queue"
mkdir -p "$NEIL_HOME/tools/autoPrompter/active"
mkdir -p "$NEIL_HOME/tools/autoPrompter/history"
mkdir -p "$NEIL_HOME/memory/zettel/src"
mkdir -p "$NEIL_HOME/memory/palace/notes"
mkdir -p "$NEIL_HOME/memory/palace/index"
mkdir -p "$NEIL_HOME/memory/mempalace"
mkdir -p "$NEIL_HOME/services/registry"
mkdir -p "$NEIL_HOME/services/vault"
mkdir -p "$NEIL_HOME/inputs/watchers"
mkdir -p "$NEIL_HOME/outputs/channels"
mkdir -p "$NEIL_HOME/self"
mkdir -p "$NEIL_HOME/mirror"
mkdir -p "$NEIL_HOME/plugins/installed"
mkdir -p "$NEIL_HOME/plugins/available"
mkdir -p "$NEIL_HOME/vision/inbox"
mkdir -p "$NEIL_HOME/vision/captures"

ok "Directory structure created"

# ── Copy source files ─────────────────────────────────────────────────
info "Installing source files..."

# If running from the repo, copy from it. Otherwise expect a tarball layout.
if [ -f "$SCRIPT_DIR/essence/identity.md" ]; then
    SRC="$SCRIPT_DIR"
elif [ -f "./essence/identity.md" ]; then
    SRC="."
else
    fail "Cannot find source files. Run from the openclaw directory or extracted tarball."
    exit 1
fi

# Essence (always overwrite -- this is the latest persona)
cp "$SRC"/essence/*.md "$NEIL_HOME/essence/"
ok "Essence files installed"

# C sources
cp "$SRC/tools/autoPrompter/src/autoprompt.c" "$NEIL_HOME/tools/autoPrompter/src/"
cp "$SRC/tools/autoPrompter/Makefile" "$NEIL_HOME/tools/autoPrompter/"
cp "$SRC/memory/zettel/src/zettel.c" "$NEIL_HOME/memory/zettel/src/"
cp "$SRC/memory/zettel/Makefile" "$NEIL_HOME/memory/zettel/"
ok "C sources installed"

# Scripts
for SCRIPT in heartbeat.sh observe.sh; do
    if [ -f "$SRC/tools/autoPrompter/$SCRIPT" ]; then
        cp "$SRC/tools/autoPrompter/$SCRIPT" "$NEIL_HOME/tools/autoPrompter/"
        chmod +x "$NEIL_HOME/tools/autoPrompter/$SCRIPT"
    fi
done

# Service handler
if [ -f "$SRC/services/handler.sh" ]; then
    cp "$SRC/services/handler.sh" "$NEIL_HOME/services/"
    chmod +x "$NEIL_HOME/services/handler.sh"
fi

# Copy registry files (don't overwrite user additions)
if [ -d "$SRC/services/registry" ]; then
    for REG in "$SRC"/services/registry/*.md; do
        [ -f "$REG" ] || continue
        BASENAME=$(basename "$REG")
        if [ ! -f "$NEIL_HOME/services/registry/$BASENAME" ]; then
            cp "$REG" "$NEIL_HOME/services/registry/"
        fi
    done
fi

# Input watchers
for W in filesystem.sh schedule.sh webhook.sh vision_inbox.sh; do
    if [ -f "$SRC/inputs/watchers/$W" ]; then
        cp "$SRC/inputs/watchers/$W" "$NEIL_HOME/inputs/watchers/"
        chmod +x "$NEIL_HOME/inputs/watchers/$W"
    fi
done

# Output channels
for CH in terminal.sh file.sh email.sh slack.sh; do
    if [ -f "$SRC/outputs/channels/$CH" ]; then
        cp "$SRC/outputs/channels/$CH" "$NEIL_HOME/outputs/channels/"
        chmod +x "$NEIL_HOME/outputs/channels/$CH"
    fi
done

# Self scripts
for S in self_check.sh snapshot.sh verify.sh lessons.md; do
    if [ -f "$SRC/self/$S" ]; then
        cp "$SRC/self/$S" "$NEIL_HOME/self/"
        [ "${S##*.}" = "sh" ] && chmod +x "$NEIL_HOME/self/$S"
    fi
done

# Plugin manager
if [ -f "$SRC/plugins/install.sh" ]; then
    cp "$SRC/plugins/install.sh" "$NEIL_HOME/plugins/"
    chmod +x "$NEIL_HOME/plugins/install.sh"
fi

# Mirror sync script
if [ -f "$SRC/mirror/sync.sh" ]; then
    cp "$SRC/mirror/sync.sh" "$NEIL_HOME/mirror/"
    chmod +x "$NEIL_HOME/mirror/sync.sh"
fi

# Vision capture script
if [ -f "$SRC/vision/capture.sh" ]; then
    cp "$SRC/vision/capture.sh" "$NEIL_HOME/vision/"
    chmod +x "$NEIL_HOME/vision/capture.sh"
fi

# Config (don't overwrite existing)
if [ ! -f "$NEIL_HOME/config.toml" ]; then
    cp "$SRC/config.toml" "$NEIL_HOME/" 2>/dev/null || true
fi

ok "Scripts and config installed"

# ── Build C binaries ──────────────────────────────────────────────────
info "Building C binaries..."

cd "$NEIL_HOME/tools/autoPrompter"
rm -f autoprompt
make
ok "autoPrompter built"

cd "$NEIL_HOME/memory/zettel"
rm -f zettel
make
ok "zettel built"

cd "$HOME"

# ── Set up MemPalace ──────────────────────────────────────────────────
info "Setting up MemPalace (semantic search)..."

if [ -d "$NEIL_HOME/memory/mempalace/.venv" ]; then
    ok "MemPalace venv already exists"
else
    if [ -f "$SRC/memory/mempalace/setup.py" ] || [ -f "$SRC/memory/mempalace/pyproject.toml" ]; then
        cp -r "$SRC/memory/mempalace/"*.py "$NEIL_HOME/memory/mempalace/" 2>/dev/null || true
        cp -r "$SRC/memory/mempalace/"*.toml "$NEIL_HOME/memory/mempalace/" 2>/dev/null || true
        cp -r "$SRC/memory/mempalace/"*.cfg "$NEIL_HOME/memory/mempalace/" 2>/dev/null || true
        [ -d "$SRC/memory/mempalace/mempalace" ] && cp -r "$SRC/memory/mempalace/mempalace" "$NEIL_HOME/memory/mempalace/"
    fi

    cd "$NEIL_HOME/memory/mempalace"
    python3 -m venv .venv
    . .venv/bin/activate
    pip install -e . 2>/dev/null || pip install chromadb 2>/dev/null || {
        warn "MemPalace pip install failed -- semantic search may not work"
        warn "Fix manually: cd $NEIL_HOME/memory/mempalace && . .venv/bin/activate && pip install -e ."
    }
    deactivate 2>/dev/null || true
    cd "$HOME"
    ok "MemPalace venv created"
fi

# ── Initialize data ──────────────────────────────────────────────────
info "Initializing data..."

# Empty state files (don't overwrite existing)
[ -f "$NEIL_HOME/heartbeat_log.json" ] || echo '[]' > "$NEIL_HOME/heartbeat_log.json"
[ -f "$NEIL_HOME/intentions.json" ] || echo '{"intentions":[]}' > "$NEIL_HOME/intentions.json"
[ -f "$NEIL_HOME/self/failures.json" ] || echo '{"failures":[]}' > "$NEIL_HOME/self/failures.json"

# Initialize zettel index
export ZETTEL_HOME="$NEIL_HOME/memory/palace"
"$NEIL_HOME/memory/zettel/zettel" reindex 2>/dev/null || true

ok "Data initialized"

# ── Generate deployment.md ────────────────────────────────────────────
info "Generating deployment config..."

if [ ! -f "$NEIL_HOME/deployment.md" ]; then
    HOSTNAME=$(hostname 2>/dev/null || echo "unknown")
    IP=$(hostname -I 2>/dev/null | awk '{print $1}' || echo "unknown")
    RAM=$(free -h 2>/dev/null | awk '/^Mem:/ {print $2}' || echo "unknown")
    DISK=$(df -h "$HOME" 2>/dev/null | awk 'NR==2 {print $2}' || echo "unknown")

    cat > "$NEIL_HOME/deployment.md" << EOF
# Deployment Configuration

This file is per-installation. Not part of the portable persona.

## Host

- **Machine**: $HOSTNAME
- **IP**: $IP
- **RAM**: $RAM
- **Disk**: $DISK
- **User**: $(whoami)
- **NEIL_HOME**: $NEIL_HOME

## Operator

- **Name**: $(whoami)
- **Trust level**: full (single user)

## Services

- **systemd**: autoprompt.service (configure below)
- **cron**: heartbeat every 30 minutes
- **Claude**: $(command -v claude 2>/dev/null || echo "not installed")
EOF
    ok "deployment.md generated"
else
    ok "deployment.md already exists (preserved)"
fi

# ── Git init ──────────────────────────────────────────────────────────
info "Initializing git repository for snapshots..."

if [ -d "$NEIL_HOME/.git" ]; then
    ok "Git repo already exists"
else
    cd "$NEIL_HOME"
    git init
    # Install .gitignore
    if [ -f "$SRC/.gitignore" ]; then
        cp "$SRC/.gitignore" "$NEIL_HOME/.gitignore"
    else
        cat > "$NEIL_HOME/.gitignore" << 'GITIGNORE'
memory/mempalace/.venv/
memory/palace/.mempalace/
mirror/remotes/*/
tools/autoPrompter/autoprompt
tools/autoPrompter/active/
memory/zettel/zettel
tools/autoPrompter/queue/
tools/autoPrompter/history/
*.tmp
*.bak
*.swp
services/vault/
GITIGNORE
    fi
    git add -A
    git commit -m "Initial openclaw installation"
    cd "$HOME"
    ok "Git repo initialized with initial snapshot"
fi

# ── Blueprint TUI ────────────────────────────────────────────────────
if [ "$INSTALL_BLUEPRINT" = true ]; then
    info "Building Blueprint TUI..."
    if [ -d "$SRC/blueprint/src" ]; then
        mkdir -p "$NEIL_HOME/blueprint"
        cp -r "$SRC/blueprint/src" "$NEIL_HOME/blueprint/"
        cp "$SRC/blueprint/Cargo.toml" "$NEIL_HOME/blueprint/"
        cp "$SRC/blueprint/Cargo.lock" "$NEIL_HOME/blueprint/" 2>/dev/null || true

        cd "$NEIL_HOME/blueprint"
        cargo build --release 2>/dev/null && {
            cp target/release/neil-blueprint "$NEIL_HOME/blueprint/"
            ok "Blueprint TUI built"
        } || {
            warn "Blueprint build failed -- you can build later with: cd $NEIL_HOME/blueprint && cargo build --release"
        }
        cd "$HOME"
    else
        warn "Blueprint source not found in distribution"
    fi
else
    info "Skipping Blueprint TUI (use --no-blueprint=false or install cargo)"
fi

# ── systemd service ──────────────────────────────────────────────────
if [ "$INSTALL_SYSTEMD" = true ]; then
    info "Installing systemd service..."

    SERVICE_FILE="/etc/systemd/system/autoprompt.service"
    CLAUDE_PATH=$(command -v claude 2>/dev/null || echo "$HOME/.local/bin/claude")

    SERVICE_CONTENT="[Unit]
Description=autoPrompter - inotify prompt queue for Neil
After=network.target

[Service]
Type=simple
User=$(whoami)
WorkingDirectory=$HOME
ExecStart=$NEIL_HOME/tools/autoPrompter/autoprompt
Restart=always
RestartSec=5
Environment=HOME=$HOME
Environment=NEIL_HOME=$NEIL_HOME
Environment=ZETTEL_HOME=$NEIL_HOME/memory/palace
Environment=PATH=$(dirname "$CLAUDE_PATH"):/usr/local/bin:/usr/bin:/bin

StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target"

    if [ "$(id -u)" -eq 0 ]; then
        echo "$SERVICE_CONTENT" > "$SERVICE_FILE"
        systemctl daemon-reload
        systemctl enable autoprompt
        systemctl start autoprompt
        ok "autoprompt.service installed and started"
    else
        # Try with sudo
        echo "$SERVICE_CONTENT" | sudo tee "$SERVICE_FILE" > /dev/null 2>&1 && {
            sudo systemctl daemon-reload
            sudo systemctl enable autoprompt
            sudo systemctl start autoprompt
            ok "autoprompt.service installed and started (via sudo)"
        } || {
            warn "Could not install systemd service (no sudo access)"
            echo "  Run as root or manually install:"
            echo "  sudo tee $SERVICE_FILE << 'EOF'"
            echo "$SERVICE_CONTENT"
            echo "EOF"
            echo "  sudo systemctl daemon-reload && sudo systemctl enable --now autoprompt"
        }
    fi
fi

# ── Cron ──────────────────────────────────────────────────────────────
if [ "$INSTALL_CRON" = true ]; then
    info "Installing cron jobs..."

    HEARTBEAT_LINE="*/30 * * * * $NEIL_HOME/tools/autoPrompter/heartbeat.sh >> /tmp/heartbeat_cron.log 2>&1"
    SNAPSHOT_LINE="0 */6 * * * NEIL_HOME=$NEIL_HOME $NEIL_HOME/self/snapshot.sh auto >> /tmp/snapshot.log 2>&1"

    # Check if already installed
    EXISTING_CRON=$(crontab -l 2>/dev/null || true)
    NEEDS_HEARTBEAT=true
    NEEDS_SNAPSHOT=true

    echo "$EXISTING_CRON" | grep -q "heartbeat.sh" && NEEDS_HEARTBEAT=false
    echo "$EXISTING_CRON" | grep -q "snapshot.sh" && NEEDS_SNAPSHOT=false

    if [ "$NEEDS_HEARTBEAT" = true ] || [ "$NEEDS_SNAPSHOT" = true ]; then
        NEW_CRON="$EXISTING_CRON"
        [ "$NEEDS_HEARTBEAT" = true ] && NEW_CRON="$NEW_CRON
$HEARTBEAT_LINE"
        [ "$NEEDS_SNAPSHOT" = true ] && NEW_CRON="$NEW_CRON
$SNAPSHOT_LINE"
        echo "$NEW_CRON" | crontab -
        ok "Cron jobs installed"
    else
        ok "Cron jobs already installed"
    fi
fi

# ── Environment hint ──────────────────────────────────────────────────
info "Setting up environment..."

SHELL_RC=""
if [ -f "$HOME/.bashrc" ]; then
    SHELL_RC="$HOME/.bashrc"
elif [ -f "$HOME/.zshrc" ]; then
    SHELL_RC="$HOME/.zshrc"
fi

if [ -n "$SHELL_RC" ]; then
    if ! grep -q "NEIL_HOME" "$SHELL_RC" 2>/dev/null; then
        echo "" >> "$SHELL_RC"
        echo "# openclaw (Neil the SEAL)" >> "$SHELL_RC"
        echo "export NEIL_HOME=\"$NEIL_HOME\"" >> "$SHELL_RC"
        echo "export ZETTEL_HOME=\"$NEIL_HOME/memory/palace\"" >> "$SHELL_RC"
        ok "Environment variables added to $SHELL_RC"
    else
        ok "Environment variables already in $SHELL_RC"
    fi
fi

# ── Self-check ────────────────────────────────────────────────────────
info "Running self-check..."
echo ""
sh "$NEIL_HOME/self/self_check.sh" || true
echo ""

# ── Done ──────────────────────────────────────────────────────────────
cat << 'DONE'

    ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    ~  openclaw installed! :D  ~
    ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

DONE

info "NEIL_HOME: $NEIL_HOME"
info "To start the TUI: $NEIL_HOME/blueprint/neil-blueprint"
info "To check status: sh $NEIL_HOME/self/self_check.sh"
info "To view logs: journalctl -u autoprompt -f"
info "To trigger a heartbeat: sh $NEIL_HOME/tools/autoPrompter/heartbeat.sh"
echo ""

if [ "$HAS_CLAUDE" = false ]; then
    warn "Remember: install the Claude CLI to enable autonomous operation"
    warn "  https://docs.anthropic.com/claude-code"
fi

info "Neil is alive. :)"
