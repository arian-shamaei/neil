#!/bin/sh
# Invariant: when user_active is fresh (<300s), no heartbeat is QUEUED.
# (In-flight beats in active/ are pre-existing work; only queueing is gated.)
USER_ACTIVE="$HOME/.neil/state/user_active"
QUEUE="$HOME/.neil/tools/autoPrompter/queue"
if [ ! -f "$USER_ACTIVE" ]; then
    echo "OK  no user_active flag"; exit 0
fi
NOW=$(date +%s)
MTIME=$(stat -c %Y "$USER_ACTIVE" 2>/dev/null || echo 0)
AGE=$((NOW - MTIME))
if [ "$AGE" -ge 300 ]; then
    echo "OK  user_active stale (${AGE}s)"; exit 0
fi
HB_Q=$(find "$QUEUE" -maxdepth 1 -name '*_heartbeat*.md' 2>/dev/null | wc -l)
if [ "$HB_Q" -eq 0 ]; then
    echo "OK  user active ${AGE}s, 0 heartbeats queued (gate holds)"; exit 0
fi
echo "FAIL user active ${AGE}s, $HB_Q heartbeat(s) queued while active"; exit 1
