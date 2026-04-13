#!/bin/sh
# filesystem.sh -- Watch a directory for new/changed files via inotify.
# Writes a prompt to Neil's queue when a file event occurs.
# Usage: ./filesystem.sh /path/to/watch [event_types]
# Event types: create, modify, delete, move (default: create,modify)

WATCH_DIR="${1:?Usage: $0 /path/to/watch [event_types]}"
EVENTS="${2:-create,modify}"
QUEUE="$HOME/.neil/tools/autoPrompter/queue"

if [ ! -d "$WATCH_DIR" ]; then
    echo "Error: $WATCH_DIR does not exist" >&2
    exit 1
fi

echo "[filesystem] watching $WATCH_DIR for $EVENTS"

inotifywait -m -r -e "$EVENTS" --format '%T %w%f %e' --timefmt '%Y-%m-%dT%H:%M:%S' "$WATCH_DIR" | \
while read TS FILEPATH EVENT; do
    # Skip temporary files and hidden files
    BASENAME=$(basename "$FILEPATH")
    case "$BASENAME" in
        .*|*~|*.tmp|*.swp) continue ;;
    esac

    SAFE_NAME=$(echo "$BASENAME" | tr ' /' '__')
    PROMPT_FILE="$QUEUE/$(date +%Y%m%dT%H%M%S)_fs_${SAFE_NAME}.md"

    cat > "$PROMPT_FILE" << PROMPT
[EVENT] source=filesystem type=$EVENT time=$TS

File: $FILEPATH
Event: $EVENT
Directory: $WATCH_DIR

Analyze this file change and decide what action to take.
PROMPT

    echo "[filesystem] event queued: $EVENT $FILEPATH"
done
