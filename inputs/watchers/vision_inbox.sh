#!/bin/sh
# vision_inbox.sh -- Watch vision/inbox/ for new images
# When an image is dropped, queue a prompt for Neil to analyze it.

INBOX="$HOME/.neil/vision/inbox"
STAGING="$HOME/.neil/vision/staging"
QUEUE="$HOME/.neil/tools/autoPrompter/queue"

mkdir -p "$INBOX" "$STAGING"

echo "[vision] watching $INBOX for images..."

inotifywait -m -e close_write,moved_to --format '%T %f' --timefmt '%Y-%m-%dT%H:%M:%S' "$INBOX" | \
while read TS FILENAME; do
    case "$FILENAME" in
        *.png|*.jpg|*.jpeg|*.gif|*.bmp|*.webp|*.txt)
            # Stage immediately to prevent race condition -- file may be
            # removed before Claude processes the prompt (can take minutes)
            STAGED="${TS}_${FILENAME}"
            if cp "$INBOX/$FILENAME" "$STAGING/$STAGED" 2>/dev/null; then
                STAGED_PATH="$STAGING/$STAGED"
                echo "[vision] staged: $FILENAME -> $STAGED"
            else
                echo "[vision] WARN: $FILENAME gone before staging, skipping"
                continue
            fi

            PROMPT_FILE="$QUEUE/$(date +%Y%m%dT%H%M%S)_vision.md"
            cat > "$PROMPT_FILE" << PROMPT
[EVENT] source=vision type=new_image time=$TS

A new image was dropped in your vision inbox: $FILENAME
Staged copy: $STAGED_PATH
Original: $INBOX/$FILENAME

Look at the staged copy of this image and describe what you see.
If it contains information relevant to your work, store it as a MEMORY.
After processing, you may remove the staged copy.
PROMPT
            echo "[vision] image queued for analysis: $FILENAME"
            ;;
    esac
done
