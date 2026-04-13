#!/bin/sh
# vision_inbox.sh -- Watch vision/inbox/ for new images
# When an image is dropped, queue a prompt for Neil to analyze it.

INBOX="$HOME/.neil/vision/inbox"
QUEUE="$HOME/.neil/tools/autoPrompter/queue"

mkdir -p "$INBOX"

echo "[vision] watching $INBOX for images..."

inotifywait -m -e close_write,moved_to --format '%T %f' --timefmt '%Y-%m-%dT%H:%M:%S' "$INBOX" | \
while read TS FILENAME; do
    case "$FILENAME" in
        *.png|*.jpg|*.jpeg|*.gif|*.bmp|*.webp|*.txt)
            PROMPT_FILE="$QUEUE/$(date +%Y%m%dT%H%M%S)_vision.md"
            cat > "$PROMPT_FILE" << PROMPT
[EVENT] source=vision type=new_image time=$TS

A new image was dropped in your vision inbox: $FILENAME
Path: $INBOX/$FILENAME

Look at this image and describe what you see. If it contains
information relevant to your work, store it as a MEMORY.
PROMPT
            echo "[vision] image queued for analysis: $FILENAME"
            ;;
    esac
done
