#!/bin/sh
# schedule.sh -- Drop a scheduled prompt into Neil's queue.
# Called by cron for time-based events beyond the heartbeat.
# Usage: ./schedule.sh <name> <message>
# Example cron: 0 9 * * 1 ~/.neil/inputs/watchers/schedule.sh weekly_review "Review this week's progress"

NAME="${1:?Usage: $0 <name> <message>}"
shift
MESSAGE="$*"
QUEUE="$HOME/.neil/tools/autoPrompter/queue"
TS=$(date +%Y%m%dT%H%M%S)

cat > "$QUEUE/${TS}_sched_${NAME}.md" << PROMPT
[EVENT] source=schedule type=scheduled_task time=$(date -Iseconds) name=$NAME

$MESSAGE
PROMPT

echo "[schedule] queued: $NAME"
