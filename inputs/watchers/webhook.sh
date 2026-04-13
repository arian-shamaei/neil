#!/bin/sh
# webhook.sh -- Listen for HTTP POST requests on a port.
# Each POST body becomes a prompt in Neil's queue.
# Usage: ./webhook.sh <port>
# Requires: ncat (from nmap) or socat

PORT="${1:-9800}"
QUEUE="$HOME/.neil/tools/autoPrompter/queue"

echo "[webhook] listening on port $PORT"

while true; do
    # Use ncat to accept one connection, read the HTTP request
    RESPONSE="HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok"

    BODY=$(ncat -l -p "$PORT" --recv-only 2>/dev/null | {
        # Read headers until blank line
        while IFS= read -r line; do
            line=$(echo "$line" | tr -d '\r')
            [ -z "$line" ] && break
            # Extract content-length
            case "$line" in
                Content-Length:*|content-length:*)
                    CL=$(echo "$line" | sed 's/[^0-9]//g')
                    ;;
            esac
        done
        # Read body
        if [ -n "$CL" ] && [ "$CL" -gt 0 ] 2>/dev/null; then
            head -c "$CL"
        else
            cat
        fi
    })

    if [ -n "$BODY" ]; then
        TS=$(date +%Y%m%dT%H%M%S)
        PROMPT_FILE="$QUEUE/${TS}_webhook.md"

        cat > "$PROMPT_FILE" << PROMPT
[EVENT] source=webhook type=http_post time=$(date -Iseconds) port=$PORT

$BODY
PROMPT

        echo "[webhook] event queued: ${#BODY} bytes from port $PORT"
    fi
done
