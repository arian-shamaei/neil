#!/bin/sh
# heartbeat.sh -- Drops a heartbeat prompt into the autoPrompter queue.
# Called by cron every 30 minutes.

QUEUE_DIR="$HOME/.neil/tools/autoPrompter/queue"
ESSENCE_DIR="$HOME/.neil/essence"
TS=$(date +%Y%m%dT%H%M%S)

# Read the heartbeat template from essence
HEARTBEAT_TEMPLATE="$ESSENCE_DIR/heartbeat.md"
if [ ! -f "$HEARTBEAT_TEMPLATE" ]; then
    echo "[heartbeat] no template at $HEARTBEAT_TEMPLATE" >&2
    exit 1
fi

# Guard: skip if heartbeat already queued (prevents backlog when autoPrompter is slow/stopped)
EXISTING=$(find "$QUEUE_DIR" -maxdepth 1 -name '*_heartbeat.md' -type f 2>/dev/null | wc -l)
if [ "$EXISTING" -gt 0 ]; then
    echo "[heartbeat] skipped: $EXISTING heartbeat(s) already queued"
    exit 0
fi

# Adaptive gate: check if this tick should fire based on activity patterns
GATE_SCRIPT="$HOME/.neil/tools/autoPrompter/adaptive_gate.sh"
if [ -x "$GATE_SCRIPT" ]; then
    if ! "$GATE_SCRIPT"; then
        exit 0
    fi
fi

# Write heartbeat prompt to queue
cp "$HEARTBEAT_TEMPLATE" "$QUEUE_DIR/${TS}_heartbeat.md"
echo "[heartbeat] queued: ${TS}_heartbeat.md"
