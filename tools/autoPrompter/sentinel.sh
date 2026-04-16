#!/bin/sh
# sentinel.sh -- System 1: Fast pattern-matching sentinel
#
# Runs every 5 minutes (cron). No Claude invocation. No API calls.
# Checks lightweight signals for actionable events.
# If something needs attention, queues an immediate heartbeat.
#
# This is the "fast brain" -- it detects events in seconds.
# The full heartbeat (System 2) does the deep thinking.
#
# Design: biological System 1/System 2 dual-process theory
#   System 1 (sentinel): fast, parallel, automatic, low-cost
#   System 2 (heartbeat): slow, serial, deliberate, expensive
#
# Signals checked (all are filesystem reads, zero network):
#   1. Queue has non-heartbeat prompts (user/webhook/watcher events)
#   2. Vision inbox has new images
#   3. New unresolved failures appeared
#   4. Intentions became overdue
#   5. Filesystem watcher dropped an event
#
# Cost: ~50ms per run. Compare to heartbeat: ~60 min per run.

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
STATE_FILE="$NEIL_HOME/tools/autoPrompter/sentinel_state.json"
QUEUE_DIR="$NEIL_HOME/tools/autoPrompter/queue"
LOG_FILE="$NEIL_HOME/heartbeat_log.json"
SENTINEL_LOG="$NEIL_HOME/tools/autoPrompter/sentinel.log"

# --- Gather signals ---

# Signal 1: Non-heartbeat prompts in queue (user activity, webhooks, etc)
QUEUE_EVENTS=$(find "$QUEUE_DIR" -maxdepth 1 -type f ! -name '*_heartbeat.md' 2>/dev/null | wc -l)

# Signal 2: Vision inbox has unprocessed images
VISION_COUNT=0
if [ -d "$NEIL_HOME/vision/inbox" ]; then
    VISION_COUNT=$(find "$NEIL_HOME/vision/inbox" -maxdepth 1 -type f 2>/dev/null | wc -l)
fi

# Signal 3: New unresolved failures since last check
FAILURE_COUNT=0
if [ -f "$NEIL_HOME/failures.json" ]; then
    FAILURE_COUNT=$(grep -c '"resolved":false' "$NEIL_HOME/failures.json" 2>/dev/null || echo 0)
fi

# Signal 4: Overdue intentions (due time passed)
OVERDUE_COUNT=0
if [ -f "$NEIL_HOME/intentions.json" ]; then
    NOW_EPOCH=$(date +%s)
    # Count pending intentions with due dates in the past
    OVERDUE_COUNT=$(awk -v now="$NOW_EPOCH" '
        /"status":"pending"/ && /"due":"[^"]*"/ {
            match($0, /"due":"([^"]*)"/, a)
            cmd = "date -d \"" a[1] "\" +%s 2>/dev/null"
            cmd | getline due_epoch
            close(cmd)
            if (due_epoch > 0 && due_epoch < now) count++
        }
        END { print count+0 }
    ' "$NEIL_HOME/intentions.json" 2>/dev/null || echo 0)
fi

# Signal 5: Active autoprompt processing (don't interrupt)
ACTIVE_COUNT=$(find "$NEIL_HOME/tools/autoPrompter/active" -maxdepth 1 -type f 2>/dev/null | wc -l)

# --- Build signal fingerprint ---
FINGERPRINT="${QUEUE_EVENTS}:${VISION_COUNT}:${FAILURE_COUNT}:${OVERDUE_COUNT}"

# --- Compare to last run ---
LAST_FINGERPRINT=""
if [ -f "$STATE_FILE" ]; then
    LAST_FINGERPRINT=$(sed -n 's/.*"fingerprint":"\([^"]*\)".*/\1/p' "$STATE_FILE")
fi

# --- Decision: should we escalate? ---
ESCALATE=0
REASON=""

# Don't escalate if a beat is already running
if [ "$ACTIVE_COUNT" -gt 0 ]; then
    ESCALATE=0
    REASON="beat-active"
# Don't escalate if a heartbeat is already queued
elif find "$QUEUE_DIR" -maxdepth 1 -name '*_heartbeat.md' -type f 2>/dev/null | grep -q .; then
    ESCALATE=0
    REASON="heartbeat-queued"
# Escalate: user/webhook event in queue
elif [ "$QUEUE_EVENTS" -gt 0 ]; then
    # These are handled by autoprompt directly, sentinel just notes it
    ESCALATE=0
    REASON="events-in-queue-already"
# Escalate: new vision images
elif [ "$VISION_COUNT" -gt 0 ] && [ "$FINGERPRINT" != "$LAST_FINGERPRINT" ]; then
    ESCALATE=1
    REASON="vision-inbox:${VISION_COUNT}"
# Escalate: new failures appeared
elif [ "$FAILURE_COUNT" -gt 0 ] && [ "$FINGERPRINT" != "$LAST_FINGERPRINT" ]; then
    ESCALATE=1
    REASON="new-failures:${FAILURE_COUNT}"
# Escalate: intentions became overdue
elif [ "$OVERDUE_COUNT" -gt 0 ] && [ "$FINGERPRINT" != "$LAST_FINGERPRINT" ]; then
    ESCALATE=1
    REASON="overdue-intentions:${OVERDUE_COUNT}"
fi

# --- Save state ---
printf '{"fingerprint":"%s","timestamp":"%s","escalated":%s,"reason":"%s"}\n' \
    "$FINGERPRINT" "$(date -Iseconds)" \
    "$([ "$ESCALATE" -eq 1 ] && echo 'true' || echo 'false')" \
    "$REASON" > "$STATE_FILE"

# --- Act ---
if [ "$ESCALATE" -eq 1 ]; then
    TS=$(date +%Y%m%dT%H%M%S)
    # Queue an immediate heartbeat (bypasses adaptive gate)
    HEARTBEAT_TEMPLATE="$NEIL_HOME/essence/heartbeat.md"
    if [ -f "$HEARTBEAT_TEMPLATE" ]; then
        cp "$HEARTBEAT_TEMPLATE" "$QUEUE_DIR/${TS}_sentinel_heartbeat.md"
        printf '[sentinel] ESCALATED: %s -> queued %s_sentinel_heartbeat.md\n' \
            "$REASON" "$TS" >> "$SENTINEL_LOG"
    fi
else
    # Log quietly (rotate log if > 1000 lines)
    if [ -f "$SENTINEL_LOG" ] && [ "$(wc -l < "$SENTINEL_LOG")" -gt 1000 ]; then
        tail -500 "$SENTINEL_LOG" > "${SENTINEL_LOG}.tmp"
        mv "${SENTINEL_LOG}.tmp" "$SENTINEL_LOG"
    fi
    printf '[sentinel] %s ok fp=%s %s\n' \
        "$(date +%H:%M:%S)" "$FINGERPRINT" "${REASON:+reason=$REASON}" >> "$SENTINEL_LOG"
fi

exit 0
