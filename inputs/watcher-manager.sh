#!/bin/sh
# watcher-manager.sh -- Manage Neil's input watchers
# Usage: ./watcher-manager.sh <start|stop|status|enable|disable>
#
# Daemon watchers are managed via systemd.
# schedule.sh is one-shot (cron) and not managed here.

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
ACTION="${1:?Usage: $0 <start|stop|status|enable|disable>}"

# Daemon watchers with their systemd service names
DAEMONS="neil-vision-inbox-watcher neil-webhook-watcher"

case "$ACTION" in
    start)
        for SVC in $DAEMONS; do
            echo "Starting $SVC..."
            sudo systemctl start "$SVC" 2>&1
        done
        ;;
    stop)
        for SVC in $DAEMONS; do
            echo "Stopping $SVC..."
            sudo systemctl stop "$SVC" 2>&1
        done
        ;;
    enable)
        sudo systemctl daemon-reload
        for SVC in $DAEMONS; do
            echo "Enabling $SVC..."
            sudo systemctl enable "$SVC" 2>&1
        done
        ;;
    disable)
        for SVC in $DAEMONS; do
            echo "Disabling $SVC..."
            sudo systemctl disable "$SVC" 2>&1
        done
        ;;
    status)
        echo "=== Daemon Watchers ==="
        for SVC in $DAEMONS; do
            STATUS=$(systemctl is-active "$SVC" 2>/dev/null || echo "not-installed")
            ENABLED=$(systemctl is-enabled "$SVC" 2>/dev/null || echo "not-installed")
            printf "  %-30s active=%-12s enabled=%s\n" "$SVC" "$STATUS" "$ENABLED"
        done
        echo ""
        echo "=== One-shot Watchers ==="
        echo "  schedule.sh: cron-triggered (check 'crontab -l' for schedule)"
        echo ""
        echo "=== Process Check ==="
        for W in "$NEIL_HOME/inputs/watchers/"*.sh; do
            [ -f "$W" ] || continue
            NAME=$(basename "$W" .sh)
            # Skip one-shot scripts
            case "$NAME" in schedule) continue ;; esac
            PID=$(pgrep -f "$W" 2>/dev/null | head -1)
            if [ -n "$PID" ]; then
                echo "  $NAME: running (pid $PID)"
            else
                echo "  $NAME: not running"
            fi
        done
        ;;
    *)
        echo "Usage: $0 <start|stop|status|enable|disable>"
        exit 1
        ;;
esac
