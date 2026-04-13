#!/bin/sh
# snapshot.sh -- Create, list, and restore Neil system snapshots
# Usage:
#   snapshot.sh save [message]    -- create a snapshot
#   snapshot.sh list [n]          -- show last n snapshots (default 10)
#   snapshot.sh diff [n]          -- diff current state vs n commits ago
#   snapshot.sh restore <hash>    -- restore to a specific snapshot
#   snapshot.sh auto              -- save only if something changed

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
cd "$NEIL_HOME"

cmd_save() {
    MSG="${1:-Manual snapshot $(date -Iseconds)}"
    git add -A 2>/dev/null
    if git diff --cached --quiet 2>/dev/null; then
        echo "[snapshot] no changes to save"
        return 0
    fi
    git commit -q -m "$MSG"
    HASH=$(git rev-parse --short HEAD)
    echo "[snapshot] saved: $HASH -- $MSG"
}

cmd_auto() {
    git add -A 2>/dev/null
    if git diff --cached --quiet 2>/dev/null; then
        return 0
    fi
    # Count changed files for the message
    CHANGED=$(git diff --cached --stat | tail -1)
    git commit -q -m "auto: $CHANGED ($(date +%Y-%m-%dT%H:%M))"
    HASH=$(git rev-parse --short HEAD)
    echo "[snapshot] auto-saved: $HASH -- $CHANGED"
}

cmd_list() {
    N="${1:-10}"
    echo "Snapshots (last $N):"
    git log --oneline -n "$N" --format="  %h  %ar  %s"
}

cmd_diff() {
    N="${1:-1}"
    git diff "HEAD~$N" HEAD --stat
    echo ""
    git diff "HEAD~$N" HEAD
}

cmd_restore() {
    HASH="$1"
    if [ -z "$HASH" ]; then
        echo "Usage: snapshot.sh restore <hash>"
        echo ""
        cmd_list 5
        return 1
    fi

    # Save current state first
    cmd_save "Pre-restore backup (restoring to $HASH)"

    # Restore
    git checkout "$HASH" -- . 2>&1
    echo "[snapshot] restored to $HASH"
    echo "[snapshot] IMPORTANT: rebuild binaries after restore:"
    echo "  cd ~/.neil/tools/autoPrompter && make"
    echo "  cd ~/.neil/memory/zettel && make"
}

case "${1:-}" in
    save)    shift; cmd_save "$*" ;;
    auto)    cmd_auto ;;
    list)    shift; cmd_list "$@" ;;
    diff)    shift; cmd_diff "$@" ;;
    restore) shift; cmd_restore "$@" ;;
    *)
        echo "Usage: snapshot.sh <save|auto|list|diff|restore> [args]"
        echo ""
        echo "  save [message]     Create a named snapshot"
        echo "  auto               Save only if changes exist"
        echo "  list [n]           Show last n snapshots"
        echo "  diff [n]           Diff vs n commits ago"
        echo "  restore <hash>     Restore to a snapshot (saves current first)"
        ;;
esac
