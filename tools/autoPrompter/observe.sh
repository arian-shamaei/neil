#!/bin/sh
# observe.sh -- Gather system observations for heartbeat
# Called by autoPrompter before Claude invocation.
# Output is plain text injected into the [OBSERVATIONS] section.

# Update seal pose for TUI
"$HOME/.neil/tools/update_seal_pose.sh" 2>/dev/null

echo "=== System ==="
echo "disk: $(df -h / | awk 'NR==2{print $5 " used, " $4 " free"}')"
echo "ram: $(free -h | awk 'NR==2{print $3 " used, " $7 " available"}')"
echo "load: $(uptime | sed 's/.*load average: //')"
echo "uptime: $(uptime -p)"

echo ""
echo "=== Services ==="
systemctl is-active autoprompt 2>/dev/null | xargs printf "autoprompt: %s\n"
pgrep -c claude 2>/dev/null | xargs printf "claude processes: %s\n"

echo ""
echo "=== Input Watchers ==="
# Daemon watchers: check systemd first, fall back to pgrep
for W in "$HOME/.neil/inputs/watchers/"*.sh; do
    [ -f "$W" ] || continue
    NAME=$(basename "$W" .sh)
    # schedule.sh is one-shot (cron), not a daemon
    case "$NAME" in
        schedule) echo "$NAME: cron-triggered (one-shot)"; continue ;;
    esac
    # Check systemd service (normalize underscores to hyphens for service name)
    SVC_NAME="neil-$(echo "$NAME" | tr '_' '-')-watcher"
    SVC_STATUS=$(systemctl is-active "$SVC_NAME" 2>/dev/null)
    if [ "$SVC_STATUS" = "active" ]; then
        echo "$NAME: running (systemd)"
    else
        # Fallback: check for manual process
        PID=$(pgrep -f "$W" 2>/dev/null | head -1)
        if [ -n "$PID" ]; then
            echo "$NAME: running (pid $PID)"
        else
            echo "$NAME: stopped"
        fi
    fi
done

echo ""
echo "=== Memory Palace ==="
export ZETTEL_HOME="$HOME/.neil/memory/palace"
$HOME/.neil/memory/zettel/zettel context 2>/dev/null

echo ""
echo "=== Queue ==="
QUEUE_COUNT=$(ls "$HOME/.neil/tools/autoPrompter/queue/" 2>/dev/null | wc -l)
ACTIVE_COUNT=$(ls "$HOME/.neil/tools/autoPrompter/active/" 2>/dev/null | wc -l)
echo "queue: $QUEUE_COUNT pending"
echo "active: $ACTIVE_COUNT processing"

echo ""
echo "=== Recent History (last 3) ==="
ls -t "$HOME/.neil/tools/autoPrompter/history/"*.result.md 2>/dev/null | head -3 | while read f; do
    NAME=$(basename "$f" .result.md)
    STATUS=$(grep 'exit_code' "$f" 2>/dev/null | head -1 | sed 's/.*: //' | tr -d '*')
    TURNS=$(grep 'turns' "$f" 2>/dev/null | head -1 | sed 's/.*: //' | tr -d '*')
    echo "  $NAME [exit:${STATUS:-?} turns:${TURNS:-1}]"
done

echo ""
echo "=== Heartbeat Log (last 3) ==="
tail -3 "$HOME/.neil/heartbeat_log.json" 2>/dev/null || echo "(empty)"

echo ""
echo "=== Intentions (pending) ==="
if [ -f "$HOME/.neil/intentions.json" ]; then
    grep '"status":"pending"' "$HOME/.neil/intentions.json" 2>/dev/null | while IFS= read -r line; do
        PRIO=$(echo "$line" | sed 's/.*"priority":"\([^"]*\)".*/\1/')
        DUE=$(echo "$line" | sed 's/.*"due":"\([^"]*\)".*/\1/')
        DESC=$(echo "$line" | sed 's/.*"description":"\([^"]*\)".*/\1/')
        TAG=$(echo "$line" | sed 's/.*"tag":"\([^"]*\)".*/\1/')
        if [ -n "$DUE" ]; then
            echo "  [$PRIO] $DESC (due: $DUE) ${TAG:+#$TAG}"
        else
            echo "  [$PRIO] $DESC ${TAG:+#$TAG}"
        fi
    done
    PENDING=$(grep -c '"status":"pending"' "$HOME/.neil/intentions.json" 2>/dev/null || echo 0)
    echo "total: $PENDING pending"
else
    echo "(none)"
fi

echo ""
echo "=== Self Check ==="
$HOME/.neil/self/self_check.sh 2>/dev/null | grep -E 'FAIL|FAILED|ALL CHECKS'

echo ""
echo "=== Unresolved Failures ==="
if [ -f "$HOME/.neil/self/failures.json" ] && [ -s "$HOME/.neil/self/failures.json" ]; then
    grep '"pending"' "$HOME/.neil/self/failures.json" 2>/dev/null | while IFS= read -r line; do
        SEV=$(echo "$line" | sed 's/.*"severity":"\([^"]*\)".*/\1/')
        SRC=$(echo "$line" | sed 's/.*"source":"\([^"]*\)".*/\1/')
        ERR=$(echo "$line" | sed 's/.*"error":"\([^"]*\)".*/\1/')
        echo "  [$SEV] $SRC: $ERR"
    done
    UNRESOLVED=$(grep -c '"pending"' "$HOME/.neil/self/failures.json" 2>/dev/null || echo 0)
    echo "total: $UNRESOLVED unresolved"
else
    echo "(none)"
fi

echo ""
echo "=== Beat Health ==="
$HOME/.neil/self/beat_health.sh 2>/dev/null || echo "health: unknown"

echo ""
echo "=== Guardrails ==="
# Daily beat count
TODAY=$(date +%Y-%m-%d)
TODAY_BEATS=$(grep -c "$TODAY" "$HOME/.neil/heartbeat_log.json" 2>/dev/null || echo 0)
echo "beats today: $TODAY_BEATS (no cap)"

# Loop detection: check if last 3 summaries are identical
if [ -f "$HOME/.neil/heartbeat_log.json" ]; then
    LAST3=$(tail -3 "$HOME/.neil/heartbeat_log.json" 2>/dev/null | sed 's/.*"summary":"\([^"]*\)".*/\1/' | sort -u | wc -l)
    if [ "$LAST3" -eq 1 ] 2>/dev/null; then
        echo "WARNING: LAST 3 BEATS IDENTICAL -- possible loop"
    fi
fi

# Quiet hours
HOUR=$(date +%H)
if [ "$HOUR" -ge 23 ] || [ "$HOUR" -lt 7 ]; then
    echo "QUIET HOURS ACTIVE -- heartbeat checks only"
fi

# Disk usage
DISK_PCT=$(df / | awk 'NR==2{print $5}' | tr -d '%')
if [ "$DISK_PCT" -ge 80 ]; then
    echo "WARNING: DISK USAGE ${DISK_PCT}%"
fi

# Pending intentions count
if [ -f "$HOME/.neil/intentions.json" ]; then
    PENDING=$(grep -c '"pending"' "$HOME/.neil/intentions.json" 2>/dev/null || echo 0)
    if [ "$PENDING" -ge 20 ]; then
        echo "WARNING: ${PENDING} pending intentions -- consolidate before adding more"
    fi
fi

echo ""
echo "=== Mirror Remotes ==="
if [ -d "$HOME/.neil/mirror/remotes" ]; then
    for DIR in "$HOME/.neil/mirror/remotes"/*/; do
        [ -d "$DIR" ] || continue
        NAME=$(basename "$DIR")
        REMOTE=$(cat "$DIR/.rclone_remote" 2>/dev/null || echo "?")
        LAST=$(cd "$DIR" && git log -1 --format="%ar: %s" 2>/dev/null || echo "never synced")
        echo "  $NAME ($REMOTE) -- $LAST"
    done
else
    echo "(none configured)"
fi

echo ""
echo "=== Blueprint TUI ==="
BP_STATE="$HOME/.neil/.blueprint_state.json"
if [ -f "$BP_STATE" ]; then
    RUNNING=$(cat "$BP_STATE" | sed -n 's/.*"running":\([^,}]*\).*/\1/p')
    if [ "$RUNNING" = "true" ]; then
        VIEW=$(cat "$BP_STATE" | sed -n 's/.*"view":"\([^"]*\)".*/\1/p')
        USER_ACTIVE=$(cat "$BP_STATE" | sed -n 's/.*"user_active":\([^,}]*\).*/\1/p')
        LAST_INPUT=$(cat "$BP_STATE" | sed -n 's/.*"last_input_time":"\([^"]*\)".*/\1/p')
        TERM_W=$(cat "$BP_STATE" | sed -n 's/.*"terminal_size":\[\([0-9]*\).*/\1/p')
        TERM_H=$(cat "$BP_STATE" | sed -n 's/.*"terminal_size":\[[0-9]*,\([0-9]*\).*/\1/p')
        SCROLL=$(cat "$BP_STATE" | sed -n 's/.*"scroll_offset":\([^,}]*\).*/\1/p')
        INPUT_BUF=$(cat "$BP_STATE" | sed -n 's/.*"input_buffer":"\([^"]*\)".*/\1/p')
        echo "status: running"
        echo "view: $VIEW"
        echo "terminal: ${TERM_W}x${TERM_H}"
        echo "user: ${USER_ACTIVE} (last input: ${LAST_INPUT})"
        if [ -n "$INPUT_BUF" ]; then
            echo "typing: $INPUT_BUF"
        fi
        if [ "$SCROLL" != "0" ]; then
            echo "scrolled: $SCROLL lines up"
        fi
    else
        echo "status: stopped"
    fi
else
    echo "status: not running"
fi

echo ""
echo "=== Vision ==="
INBOX_COUNT=$(ls "$HOME/.neil/vision/inbox/" 2>/dev/null | wc -l)
CAPTURE_COUNT=$(ls "$HOME/.neil/vision/captures/" 2>/dev/null | wc -l)
echo "inbox: $INBOX_COUNT pending"
echo "captures: $CAPTURE_COUNT stored"
if [ "$INBOX_COUNT" -gt 0 ]; then
    echo "NEW IMAGES TO REVIEW:"
    ls -t "$HOME/.neil/vision/inbox/" 2>/dev/null | head -5 | while read F; do
        echo "  - $F"
    done
fi

echo ""
echo "=== Stream ==="
if [ -f "\$HOME/.neil/.neil_stream" ]; then
    HEAD=\$(head -1 "\$HOME/.neil/.neil_stream" 2>/dev/null)
    STATUS=\$(echo "\$HEAD" | sed -n 's/.*"status":"\([^"]*\)".*/\1/p')
    SIZE=\$(wc -c < "\$HOME/.neil/.neil_stream" 2>/dev/null)
    echo "status: \$STATUS (\${SIZE} bytes)"
else
    echo "status: idle"
fi
