#!/bin/sh
# update_seal_pose.sh -- Set seal pose based on system state
# Called by observe.sh (or heartbeat) to keep the TUI seal reactive.

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
POSE_FILE="$NEIL_HOME/.seal_pose.json"
HEARTBEAT_LOG="$NEIL_HOME/heartbeat_log.json"
FAILURES="$NEIL_HOME/memory/palace/failures"

# Defaults
EYES="open"
MOUTH="smile"
BODY="float"
INDICATOR="none"
LABEL="~ neil ~"

# --- Determine mood from system state ---

HOUR=$(date +%H)

# Quiet hours = sleeping
if [ "$HOUR" -ge 23 ] || [ "$HOUR" -lt 7 ]; then
    EYES="closed"
    MOUTH="relaxed"
    BODY="sleep"
    INDICATOR="zzz"
    LABEL="~ zzz ~"
    write_and_exit=1
fi

if [ -z "$write_and_exit" ]; then
    # Check for failures
    FAIL_COUNT=0
    if [ -d "$FAILURES" ]; then
        FAIL_COUNT=$(find "$FAILURES" -name '*.md' -newer "$FAILURES/../.last_resolved" 2>/dev/null | wc -l)
    fi

    # Check queue depth
    QUEUE_COUNT=0
    QUEUE_DIR="$NEIL_HOME/tools/autoPrompter/queue"
    if [ -d "$QUEUE_DIR" ]; then
        QUEUE_COUNT=$(find "$QUEUE_DIR" -name '*.md' 2>/dev/null | wc -l)
    fi

    # Check last heartbeat status
    LAST_STATUS=""
    if [ -f "$HEARTBEAT_LOG" ]; then
        LAST_STATUS=$(tail -1 "$HEARTBEAT_LOG" | sed -n 's/.*"status":"\([^"]*\)".*/\1/p')
    fi

    # Check if actively processing
    CLAUDE_PROCS=$(pgrep -c claude 2>/dev/null || echo 0)

    # Actively working
    if [ "$CLAUDE_PROCS" -gt 0 ]; then
        EYES="focused"
        MOUTH="neutral"
        BODY="swim"
        INDICATOR="thought"
        LABEL="~ working ~"
    # Errors present
    elif [ "$FAIL_COUNT" -gt 0 ]; then
        EYES="wide"
        MOUTH="frown"
        BODY="surface"
        INDICATOR="alert"
        LABEL="~ alert ~"
    # Queue backed up
    elif [ "$QUEUE_COUNT" -gt 3 ]; then
        EYES="wide"
        MOUTH="neutral"
        BODY="swim"
        INDICATOR="bubbles"
        LABEL="~ busy ~"
    # Last beat had an error
    elif [ "$LAST_STATUS" = "error" ]; then
        EYES="half"
        MOUTH="frown"
        BODY="surface"
        INDICATOR="alert"
        LABEL="~ hmm ~"
    # All good, idle
    elif [ "$LAST_STATUS" = "ok" ]; then
        EYES="open"
        MOUTH="smile"
        BODY="float"
        INDICATOR="bubbles"
        LABEL="~ neil ~"
    # Just acted
    elif [ "$LAST_STATUS" = "acted" ]; then
        EYES="open"
        MOUTH="smile"
        BODY="swim"
        INDICATOR="music"
        LABEL="~ done ~"
    fi
fi

# Write pose
cat > "$POSE_FILE" << POSEJSON
{
  "eyes": "$EYES",
  "mouth": "$MOUTH",
  "body": "$BODY",
  "indicator": "$INDICATOR",
  "label": "$LABEL"
}
POSEJSON
