#!/bin/sh
# adaptive_gate.sh -- Decides whether this heartbeat tick should fire.
#
# Called by heartbeat.sh before queuing. Exit 0 = fire, exit 1 = skip.
#
# Modes (based on recent activity):
#   hot  - every tick (30 min): operator active, failures, pending intentions
#   warm - every 2nd tick (60 min): productive beats, nothing urgent
#   cool - every 3rd tick (90 min): quiet, beats were "ok" or light work
#   cold - every 4th tick (120 min): sustained idle, nothing for 4+ hours
#
# State persisted in adaptive_state.json.

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
STATE_FILE="$NEIL_HOME/tools/autoPrompter/adaptive_state.json"
LOG_FILE="$NEIL_HOME/heartbeat_log.json"
INTENTIONS_FILE="$NEIL_HOME/intentions.json"

# Initialize state file if missing
if [ ! -f "$STATE_FILE" ]; then
    printf '{"mode":"hot","skip_count":0,"last_fired":0}\n' > "$STATE_FILE"
fi

# Read current state
MODE=$(sed -n 's/.*"mode":"\([^"]*\)".*/\1/p' "$STATE_FILE")
SKIP_COUNT=$(sed -n 's/.*"skip_count":\([0-9]*\).*/\1/p' "$STATE_FILE")
[ -z "$MODE" ] && MODE="hot"
[ -z "$SKIP_COUNT" ] && SKIP_COUNT=0

# --- Determine what mode we should be in ---

# Check: operator active in last 15 minutes?
OPERATOR_ACTIVE=0
if [ -f "$LOG_FILE" ]; then
    LAST_CHAT=$(grep '"_chat.md"' "$LOG_FILE" | tail -1 | sed -n 's/.*"timestamp":"\([^"]*\)".*/\1/p')
    if [ -n "$LAST_CHAT" ]; then
        # Convert timestamp format 2026-04-15T14-23-20 to epoch-ish comparison
        CHAT_MINS=$(echo "$LAST_CHAT" | sed 's/[^0-9]//g')
        NOW_MINS=$(date +%Y%m%d%H%M%S)
        # Simple: if the last chat timestamp shares the same hour, operator is recent
        CHAT_HOUR=$(echo "$LAST_CHAT" | sed 's/.*T\([0-9]*\)-.*/\1/')
        NOW_HOUR=$(date +%H)
        CHAT_DATE=$(echo "$LAST_CHAT" | sed 's/T.*//')
        NOW_DATE=$(date +%Y-%m-%d)
        if [ "$CHAT_DATE" = "$NOW_DATE" ]; then
            # Same day -- check if within last 30 min by comparing raw timestamps
            CHAT_NUM=$(echo "$LAST_CHAT" | sed 's/[T-]//g')
            NOW_NUM=$(date +%Y%m%d%H%M%S)
            DIFF=$((NOW_NUM - CHAT_NUM))
            # Rough: if diff < 3000 (about 30 min in HHMMSS space), active
            [ "$DIFF" -lt 3000 ] 2>/dev/null && OPERATOR_ACTIVE=1
        fi
    fi
fi

# Check: any unresolved failures?
HAS_FAILURES=0
if [ -f "$NEIL_HOME/failures.json" ]; then
    UNRESOLVED=$(grep -c '"resolved":false' "$NEIL_HOME/failures.json" 2>/dev/null)
    [ "$UNRESOLVED" -gt 0 ] 2>/dev/null && HAS_FAILURES=1
fi

# Check: pending intentions?
HAS_INTENTIONS=0
if [ -f "$INTENTIONS_FILE" ]; then
    PENDING=$(grep -c '"status":"pending"' "$INTENTIONS_FILE" 2>/dev/null)
    [ "$PENDING" -gt 0 ] 2>/dev/null && HAS_INTENTIONS=1
fi

# Check: recent beat productivity
RECENT_STATUS="idle"
if [ -f "$LOG_FILE" ]; then
    LAST_3=$(tail -3 "$LOG_FILE")
    ACTED_COUNT=$(echo "$LAST_3" | grep -c '"status":"acted"')
    if [ "$ACTED_COUNT" -ge 2 ]; then
        RECENT_STATUS="productive"
    elif [ "$ACTED_COUNT" -ge 1 ]; then
        RECENT_STATUS="light"
    fi
fi

# Check: hours since last meaningful activity
HOURS_IDLE=0
if [ -f "$LOG_FILE" ]; then
    LAST_ACTED=$(grep '"status":"acted"' "$LOG_FILE" | tail -1 | sed -n 's/.*"timestamp":"\([^"]*\)".*/\1/p')
    if [ -n "$LAST_ACTED" ]; then
        ACTED_EPOCH=$(date -d "$(echo "$LAST_ACTED" | sed 's/\([0-9]*-[0-9]*-[0-9]*\)T\([0-9]*\)-\([0-9]*\)-\([0-9]*\)/\1 \2:\3:\4/')" +%s 2>/dev/null)
        NOW_EPOCH=$(date +%s)
        if [ -n "$ACTED_EPOCH" ] && [ -n "$NOW_EPOCH" ]; then
            HOURS_IDLE=$(( (NOW_EPOCH - ACTED_EPOCH) / 3600 ))
        fi
    fi
fi

# --- Decision logic ---
NEW_MODE="cool"  # default

if [ "$OPERATOR_ACTIVE" -eq 1 ] || [ "$HAS_FAILURES" -eq 1 ] || [ "$HAS_INTENTIONS" -eq 1 ]; then
    NEW_MODE="hot"
elif [ "$RECENT_STATUS" = "productive" ]; then
    NEW_MODE="warm"
elif [ "$HOURS_IDLE" -ge 4 ]; then
    NEW_MODE="cold"
elif [ "$RECENT_STATUS" = "idle" ]; then
    NEW_MODE="cool"
else
    NEW_MODE="warm"
fi

# --- Gate logic: should we fire this tick? ---
case "$NEW_MODE" in
    hot)  FIRE_EVERY=1 ;;
    warm) FIRE_EVERY=2 ;;
    cool) FIRE_EVERY=3 ;;
    cold) FIRE_EVERY=4 ;;
    *)    FIRE_EVERY=1 ;;
esac

SHOULD_FIRE=0
NEW_SKIP=$((SKIP_COUNT + 1))

if [ "$NEW_SKIP" -ge "$FIRE_EVERY" ]; then
    SHOULD_FIRE=1
    NEW_SKIP=0
fi

# Mode change always fires immediately (responsiveness)
if [ "$NEW_MODE" != "$MODE" ]; then
    SHOULD_FIRE=1
    NEW_SKIP=0
fi

# Save state
printf '{"mode":"%s","skip_count":%d,"last_fired":%d}\n' \
    "$NEW_MODE" "$NEW_SKIP" "$(date +%s)" > "$STATE_FILE"

if [ "$SHOULD_FIRE" -eq 1 ]; then
    echo "[adaptive] mode=$NEW_MODE firing (tick $NEW_SKIP/$FIRE_EVERY)"
    exit 0
else
    # USWS: Unihemispheric Slow-Wave Sleep
    # Alert hemisphere (input watchers) stays running.
    # Sleep hemisphere (consolidation) does background maintenance.
    # This way skipped ticks are productive without using Claude tokens.
    USWS_SCRIPT="$NEIL_HOME/tools/autoPrompter/usws_consolidate.sh"
    if [ -x "$USWS_SCRIPT" ]; then
        echo "[adaptive] mode=$NEW_MODE skipping heartbeat, running USWS consolidation"
        "$USWS_SCRIPT" 2>/dev/null &
    else
        echo "[adaptive] mode=$NEW_MODE skipping (tick $NEW_SKIP/$FIRE_EVERY)"
    fi
    exit 1
fi
