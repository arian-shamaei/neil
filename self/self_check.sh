#!/bin/sh
# self_check.sh -- Verify all Neil components are functional.
# Exit 0 = healthy, Exit 1 = something broken (details on stdout).

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
ERRORS=0

check() {
    if ! eval "$2" > /dev/null 2>&1; then
        echo "FAIL: $1"
        ERRORS=$((ERRORS + 1))
    else
        echo "  ok: $1"
    fi
}

echo "=== Self Check ==="

# Core binaries
check "autoPrompter binary" "test -x $NEIL_HOME/tools/autoPrompter/autoprompt"
check "autoPrompter source" "test -f $NEIL_HOME/tools/autoPrompter/src/autoprompt.c"
check "zettel binary" "test -x $NEIL_HOME/memory/zettel/zettel"
check "zettel source" "test -f $NEIL_HOME/memory/zettel/src/zettel.c"

# Zettel functionality
check "zettel runs" "ZETTEL_HOME=$NEIL_HOME/memory/palace $NEIL_HOME/memory/zettel/zettel context"

# MemPalace venv
check "mempalace venv" "test -f $NEIL_HOME/memory/mempalace/.venv/bin/activate"
check "mempalace import" ". $NEIL_HOME/memory/mempalace/.venv/bin/activate && python3 -c 'import mempalace'"

# Essence files
for F in identity.md soul.md mission.md overview.md actions.md heartbeat.md; do
    check "essence/$F" "test -f $NEIL_HOME/essence/$F"
done

# Directory structure
check "palace/notes dir" "test -d $NEIL_HOME/memory/palace/notes"
check "palace/index dir" "test -d $NEIL_HOME/memory/palace/index"
check "services/registry" "test -d $NEIL_HOME/services/registry"
check "services/vault" "test -d $NEIL_HOME/services/vault"
check "inputs/watchers" "test -d $NEIL_HOME/inputs/watchers"
check "outputs/channels" "test -d $NEIL_HOME/outputs/channels"
check "self dir" "test -d $NEIL_HOME/self"

# Scripts executable
check "handler.sh" "test -x $NEIL_HOME/services/handler.sh"
check "observe.sh" "test -x $NEIL_HOME/tools/autoPrompter/observe.sh"
check "heartbeat.sh" "test -x $NEIL_HOME/tools/autoPrompter/heartbeat.sh"

# systemd
check "autoprompt service" "systemctl is-active autoprompt"

# Queue dirs
check "queue dir" "test -d $NEIL_HOME/tools/autoPrompter/queue"
check "active dir" "test -d $NEIL_HOME/tools/autoPrompter/active"
check "history dir" "test -d $NEIL_HOME/tools/autoPrompter/history"

# Claude binary
check "claude binary" "test -x $HOME/.local/bin/claude"

echo ""
if [ $ERRORS -eq 0 ]; then
    echo "ALL CHECKS PASSED"
    exit 0
else
    echo "FAILED: $ERRORS checks"
    exit 1
fi
