#!/bin/sh
# capture.sh -- Adaptive visual capture for Neil
# Auto-detects available capture methods and uses the best one.
# Usage:
#   capture.sh                  auto-detect and capture
#   capture.sh screenshot       desktop screenshot
#   capture.sh pane [name]      tmux pane text dump
#   capture.sh camera [url]     camera snapshot
#   capture.sh window <wm_id>   specific window
#   capture.sh clipboard        clipboard image
#   capture.sh list             list available methods

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
CAPTURES="$NEIL_HOME/vision/captures"
TS=$(date +%Y%m%dT%H%M%S)

mkdir -p "$CAPTURES"

# Prune old captures (keep last 50)
ls -t "$CAPTURES"/* 2>/dev/null | tail -n +51 | xargs rm -f 2>/dev/null

capture_screenshot() {
    OUT="$CAPTURES/${TS}_screenshot.png"

    if command -v scrot > /dev/null 2>&1; then
        scrot "$OUT" && echo "$OUT"
    elif command -v import > /dev/null 2>&1; then
        import -window root "$OUT" && echo "$OUT"
    elif command -v gnome-screenshot > /dev/null 2>&1; then
        gnome-screenshot -f "$OUT" && echo "$OUT"
    elif command -v xdg-screenshot > /dev/null 2>&1; then
        xdg-screenshot "$OUT" && echo "$OUT"
    elif command -v fbcat > /dev/null 2>&1; then
        fbcat > "$OUT" && echo "$OUT"
    else
        echo "ERROR: no screenshot tool available (install scrot or imagemagick)"
        return 1
    fi
}

capture_pane() {
    PANE="${1:-}"
    OUT="$CAPTURES/${TS}_pane.txt"

    if ! command -v tmux > /dev/null 2>&1; then
        echo "ERROR: tmux not installed"
        return 1
    fi

    if [ -n "$PANE" ]; then
        tmux capture-pane -t "$PANE" -p > "$OUT" 2>&1
    else
        # Capture all visible panes
        for SESS in $(tmux list-sessions -F '#{session_name}' 2>/dev/null); do
            for WIN in $(tmux list-windows -t "$SESS" -F '#{window_index}' 2>/dev/null); do
                for P in $(tmux list-panes -t "$SESS:$WIN" -F '#{pane_index}' 2>/dev/null); do
                    echo "=== $SESS:$WIN.$P ===" >> "$OUT"
                    tmux capture-pane -t "$SESS:$WIN.$P" -p >> "$OUT" 2>/dev/null
                    echo "" >> "$OUT"
                done
            done
        done
    fi

    if [ -s "$OUT" ]; then
        echo "$OUT"
    else
        rm -f "$OUT"
        echo "ERROR: no tmux panes captured"
        return 1
    fi
}

capture_camera() {
    URL="${1:-}"
    OUT="$CAPTURES/${TS}_camera.jpg"

    if [ -n "$URL" ]; then
        curl -s -o "$OUT" "$URL" && echo "$OUT"
    elif command -v fswebcam > /dev/null 2>&1; then
        fswebcam -r 640x480 --no-banner "$OUT" 2>/dev/null && echo "$OUT"
    elif command -v ffmpeg > /dev/null 2>&1; then
        ffmpeg -f v4l2 -i /dev/video0 -frames:v 1 "$OUT" 2>/dev/null && echo "$OUT"
    else
        echo "ERROR: no camera available"
        return 1
    fi
}

capture_window() {
    WID="$1"
    OUT="$CAPTURES/${TS}_window.png"

    if command -v import > /dev/null 2>&1; then
        import -window "$WID" "$OUT" && echo "$OUT"
    else
        echo "ERROR: imagemagick import not available"
        return 1
    fi
}

capture_clipboard() {
    OUT="$CAPTURES/${TS}_clipboard.png"

    if command -v xclip > /dev/null 2>&1; then
        xclip -selection clipboard -t image/png -o > "$OUT" 2>/dev/null
        if [ -s "$OUT" ]; then
            echo "$OUT"
        else
            rm -f "$OUT"
            echo "ERROR: no image in clipboard"
            return 1
        fi
    elif command -v xsel > /dev/null 2>&1; then
        xsel --clipboard --output > "$OUT" 2>/dev/null
        if [ -s "$OUT" ]; then echo "$OUT"; else rm -f "$OUT"; echo "ERROR: no image"; return 1; fi
    else
        echo "ERROR: xclip/xsel not available"
        return 1
    fi
}

capture_auto() {
    # Try in order of usefulness
    # 1. Check inbox first
    INBOX="$NEIL_HOME/vision/inbox"
    INBOX_FILE=$(ls -t "$INBOX"/* 2>/dev/null | head -1)
    if [ -n "$INBOX_FILE" ]; then
        # Move to captures with timestamp
        DEST="$CAPTURES/${TS}_inbox_$(basename "$INBOX_FILE")"
        mv "$INBOX_FILE" "$DEST"
        echo "$DEST"
        return 0
    fi

    # 2. tmux pane (most common in terminal)
    if command -v tmux > /dev/null 2>&1 && tmux list-sessions > /dev/null 2>&1; then
        capture_pane
        return $?
    fi

    # 3. Screenshot (if display available)
    if [ -n "$DISPLAY" ] || [ -n "$WAYLAND_DISPLAY" ]; then
        capture_screenshot
        return $?
    fi

    # 4. Framebuffer (headless with console)
    if [ -r /dev/fb0 ] && command -v fbcat > /dev/null 2>&1; then
        capture_screenshot
        return $?
    fi

    echo "ERROR: no capture method available. Drop an image in $INBOX"
    return 1
}

list_methods() {
    echo "Available capture methods:"
    echo "  inbox       : always (drop files in vision/inbox/)"

    if command -v tmux > /dev/null 2>&1; then
        SESSIONS=$(tmux list-sessions 2>/dev/null | wc -l)
        echo "  pane        : tmux ($SESSIONS sessions)"
    else
        echo "  pane        : NOT AVAILABLE (install tmux)"
    fi

    if [ -n "$DISPLAY" ] || [ -n "$WAYLAND_DISPLAY" ]; then
        echo "  screenshot  : display detected"
    else
        echo "  screenshot  : NO DISPLAY"
    fi

    for TOOL in scrot import gnome-screenshot fbcat; do
        if command -v $TOOL > /dev/null 2>&1; then
            echo "    tool: $TOOL"
        fi
    done

    if command -v fswebcam > /dev/null 2>&1 || [ -e /dev/video0 ]; then
        echo "  camera      : available"
    else
        echo "  camera      : NOT AVAILABLE"
    fi

    if command -v xclip > /dev/null 2>&1; then
        echo "  clipboard   : available"
    else
        echo "  clipboard   : NOT AVAILABLE"
    fi
}

case "${1:-auto}" in
    screenshot)  capture_screenshot ;;
    pane)        shift; capture_pane "$@" ;;
    camera)      shift; capture_camera "$@" ;;
    window)      shift; capture_window "$@" ;;
    clipboard)   capture_clipboard ;;
    list)        list_methods ;;
    auto)        capture_auto ;;
    *)           echo "Usage: capture.sh [screenshot|pane|camera|window|clipboard|list|auto]" ;;
esac
