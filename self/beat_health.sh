#!/bin/sh
# beat_health.sh -- Analyze heartbeat reliability and surface trends
# Reads heartbeat_log.json. Outputs a compact health report.
# Called by observe.sh to give Neil visibility into its own reliability.
#
# Metrics:
#   - Success rate (last N beats)
#   - Current streak (consecutive successes or failures)
#   - Failure clustering (multiple failures in a row = worse than spread out)
#   - Average gap between beats (detect missed crons)
#   - Recommendation: adapt behavior if degraded

LOG="$HOME/.neil/heartbeat_log.json"

if [ ! -f "$LOG" ] || [ ! -s "$LOG" ]; then
    echo "health: unknown (no data)"
    exit 0
fi

TOTAL=$(wc -l < "$LOG")
# Use last 20 beats or all if fewer
WINDOW=20
if [ "$TOTAL" -lt "$WINDOW" ]; then
    WINDOW="$TOTAL"
fi

# Count statuses in window
ACTED=$(tail -"$WINDOW" "$LOG" | grep -c '"status":"acted"')
OK=$(tail -"$WINDOW" "$LOG" | grep -c '"status":"ok"')
UNKNOWN=$(tail -"$WINDOW" "$LOG" | grep -c '"status":"unknown"')
ERROR=$(tail -"$WINDOW" "$LOG" | grep -c '"status":"error"')

SUCCESS=$((ACTED + OK))
FAIL=$((UNKNOWN + ERROR))

if [ "$WINDOW" -gt 0 ]; then
    # Integer percentage
    RATE=$((SUCCESS * 100 / WINDOW))
else
    RATE=0
fi

# Current streak: count consecutive same-status from the end
LAST_STATUS=$(tail -1 "$LOG" | sed 's/.*"status":"\([^"]*\)".*/\1/')
STREAK=0
tail -"$WINDOW" "$LOG" | tac 2>/dev/null | while IFS= read -r line; do
    S=$(echo "$line" | sed 's/.*"status":"\([^"]*\)".*/\1/')
    if [ "$S" = "$LAST_STATUS" ]; then
        STREAK=$((STREAK + 1))
    else
        break
    fi
done
# Shell subshell workaround -- use temp file
STREAK_FILE=$(mktemp)
STREAK=0
tail -"$WINDOW" "$LOG" | tac 2>/dev/null | while IFS= read -r line; do
    S=$(echo "$line" | sed 's/.*"status":"\([^"]*\)".*/\1/')
    if [ "$S" = "$LAST_STATUS" ]; then
        STREAK=$((STREAK + 1))
        echo "$STREAK" > "$STREAK_FILE"
    else
        echo "$STREAK" > "$STREAK_FILE"
        break
    fi
done
STREAK=$(cat "$STREAK_FILE" 2>/dev/null || echo 0)
rm -f "$STREAK_FILE"

# Failure clustering: count max consecutive failures in window
MAX_FAIL_RUN=0
CURRENT_RUN=0
tail -"$WINDOW" "$LOG" | while IFS= read -r line; do
    S=$(echo "$line" | sed 's/.*"status":"\([^"]*\)".*/\1/')
    case "$S" in
        unknown|error)
            CURRENT_RUN=$((CURRENT_RUN + 1))
            if [ "$CURRENT_RUN" -gt "$MAX_FAIL_RUN" ]; then
                MAX_FAIL_RUN=$CURRENT_RUN
            fi
            ;;
        *)
            CURRENT_RUN=0
            ;;
    esac
done
# Same subshell workaround
CLUSTER_FILE=$(mktemp)
echo 0 > "$CLUSTER_FILE"
MAX_FAIL_RUN=0
CURRENT_RUN=0
tail -"$WINDOW" "$LOG" | sed 's/.*"status":"\([^"]*\)".*/\1/' | while IFS= read -r S; do
    MAX_FAIL_RUN=$(cat "$CLUSTER_FILE")
    case "$S" in
        unknown|error)
            CURRENT_RUN=$((CURRENT_RUN + 1))
            if [ "$CURRENT_RUN" -gt "$MAX_FAIL_RUN" ]; then
                echo "$CURRENT_RUN" > "$CLUSTER_FILE"
            fi
            ;;
        *)
            CURRENT_RUN=0
            ;;
    esac
done
MAX_FAIL_RUN=$(cat "$CLUSTER_FILE" 2>/dev/null || echo 0)
rm -f "$CLUSTER_FILE"

# Determine health grade
if [ "$RATE" -ge 90 ]; then
    GRADE="excellent"
elif [ "$RATE" -ge 70 ]; then
    GRADE="good"
elif [ "$RATE" -ge 50 ]; then
    GRADE="degraded"
else
    GRADE="critical"
fi

# Output compact report
echo "health: $GRADE (${RATE}% success, ${SUCCESS}/${WINDOW} beats)"
echo "  last: $LAST_STATUS (streak: $STREAK)"
echo "  failures: $FAIL/$WINDOW (max cluster: $MAX_FAIL_RUN consecutive)"

# Actionable recommendations
if [ "$RATE" -lt 50 ]; then
    echo "  WARNING: Critical reliability. Consider reducing prompt complexity."
elif [ "$RATE" -lt 70 ]; then
    echo "  CAUTION: Degraded reliability. Monitor for further decline."
fi

if [ "$MAX_FAIL_RUN" -ge 3 ]; then
    echo "  WARNING: $MAX_FAIL_RUN consecutive failures detected -- systemic issue likely."
fi

# Check for the new retry binary
BINARY="$HOME/.neil/tools/autoPrompter/autoprompt"
SOURCE="$HOME/.neil/tools/autoPrompter/src/autoprompt.c"
if [ -f "$BINARY" ] && [ -f "$SOURCE" ]; then
    if [ "$SOURCE" -nt "$BINARY" ]; then
        echo "  NOTE: Source newer than binary -- rebuild needed."
    fi
fi
