#!/bin/sh
# handler.sh -- Service call broker
# Called by autoPrompter with env vars:
#   NEIL_SERVICE  - service name (e.g., github)
#   NEIL_ACTION   - action name (e.g., list-repos)
#   NEIL_CRED     - credential from vault
#   NEIL_PARAMS   - key=value pairs

set -e

GWS_BIN="$HOME/.local/bin/gws"

# Parse params into variables
# Handles: key=value key="quoted value" key='quoted value'
eval_params() {
    local params="$NEIL_PARAMS"
    while [ -n "$params" ]; do
        params="${params#"${params%%[![:space:]]*}"}"
        [ -z "$params" ] && break
        key="${params%%=*}"
        params="${params#*=}"
        case "$params" in
            \"*)
                params="${params#\"}"
                val="${params%%\"*}"
                params="${params#*\"}"
                ;;
            \'*)
                params="${params#\'}"
                val="${params%%\'*}"
                params="${params#*\'}"
                ;;
            *)
                val="${params%% *}"
                case "$params" in
                    *\ *) params="${params#* }" ;;
                    *) params="" ;;
                esac
                ;;
        esac
        export "PARAM_$key=$val"
    done
}
eval_params

case "$NEIL_SERVICE" in
    spawn_temp)
        case "$NEIL_ACTION" in
            run)
                NEIL_TASK="$PARAM_task" \
                NEIL_VERIFY="$PARAM_verify" \
                NEIL_MAX_SEC="${PARAM_max_sec:-300}" \
                NEIL_MEMORY="${PARAM_memory:-read_only}" \
                NEIL_PERSONA="${PARAM_persona:-minimal}" \
                NEIL_HOME="$HOME/.neil" \
                python3 "$HOME/.neil/tools/temp_neil/spawn.py" 2>&1
                ;;
            *)
                echo "ERROR: unknown action for spawn_temp: $NEIL_ACTION"
                exit 1
                ;;
        esac
        ;;

    test)
        case "$NEIL_ACTION" in
            echo)
                echo "{\"service\":\"test\",\"action\":\"echo\",\"message\":\"$PARAM_message\"}"
                ;;
            time)
                echo "{\"service\":\"test\",\"action\":\"time\",\"server_time\":\"$(date -Iseconds)\"}"
                ;;
            ip)
                IP=$(curl -s ifconfig.me 2>/dev/null || echo "unknown")
                echo "{\"service\":\"test\",\"action\":\"ip\",\"address\":\"$IP\"}"
                ;;
            *)
                echo "ERROR: unknown action '$NEIL_ACTION' for service 'test'"
                exit 1
                ;;
        esac
        ;;

    gsheets)
        case "$NEIL_ACTION" in
            read)
                $GWS_BIN sheets +read \
                    --spreadsheet "$PARAM_sheet" \
                    --range "'$PARAM_range'" 2>&1
                ;;
            append)
                $GWS_BIN sheets +append \
                    --spreadsheet "$PARAM_sheet" \
                    --range "'$PARAM_range'" \
                    --values "$PARAM_values" 2>&1
                ;;
            update)
                $GWS_BIN sheets spreadsheets.values update \
                    --params "{\"spreadsheetId\":\"$PARAM_sheet\",\"range\":\"$PARAM_range\",\"valueInputOption\":\"USER_ENTERED\"}" \
                    --json "{\"values\":[[\"$PARAM_values\"]]}" 2>&1
                ;;
            info)
                $GWS_BIN sheets spreadsheets get \
                    --params "{\"spreadsheetId\":\"$PARAM_sheet\"}" 2>&1 | \
                    head -50
                ;;
            *)
                echo "ERROR: unknown action '$NEIL_ACTION' for service 'gsheets'"
                exit 1
                ;;
        esac
        ;;

    gdrive)
        case "$NEIL_ACTION" in
            list)
                QUERY="${PARAM_query:-}"
                if [ -n "$QUERY" ]; then
                    $GWS_BIN drive files list \
                        --params "{\"q\":\"$QUERY\",\"pageSize\":10}" 2>&1
                elif [ -n "$PARAM_folder" ]; then
                    $GWS_BIN drive files list \
                        --params "{\"q\":\"'$PARAM_folder' in parents\",\"pageSize\":20}" 2>&1
                else
                    $GWS_BIN drive files list \
                        --params "{\"pageSize\":10,\"orderBy\":\"modifiedTime desc\"}" 2>&1
                fi
                ;;
            read)
                $GWS_BIN drive files export \
                    --params "{\"fileId\":\"$PARAM_file\",\"mimeType\":\"text/plain\"}" 2>&1
                ;;
            search)
                $GWS_BIN drive files list \
                    --params "{\"q\":\"fullText contains '$PARAM_query'\",\"pageSize\":10}" 2>&1
                ;;
            *)
                echo "ERROR: unknown action '$NEIL_ACTION' for service 'gdrive'"
                exit 1
                ;;
        esac
        ;;

    github)
        BASE="https://api.github.com"
        AUTH="Authorization: Bearer $NEIL_CRED"
        ACCEPT="Accept: application/vnd.github+json"

        case "$NEIL_ACTION" in
            list-repos)
                curl -s -H "$AUTH" -H "$ACCEPT" "$BASE/user/repos?per_page=10&sort=updated"
                ;;
            get-repo)
                curl -s -H "$AUTH" -H "$ACCEPT" "$BASE/repos/$PARAM_repo"
                ;;
            list-issues)
                STATE="${PARAM_state:-open}"
                curl -s -H "$AUTH" -H "$ACCEPT" "$BASE/repos/$PARAM_repo/issues?state=$STATE"
                ;;
            create-issue)
                curl -s -X POST -H "$AUTH" -H "$ACCEPT" \
                    -d "{\"title\":\"$PARAM_title\",\"body\":\"$PARAM_body\"}" \
                    "$BASE/repos/$PARAM_repo/issues"
                ;;
            get-pull)
                curl -s -H "$AUTH" -H "$ACCEPT" "$BASE/repos/$PARAM_repo/pulls/$PARAM_number"
                ;;
            *)
                echo "ERROR: unknown action '$NEIL_ACTION' for service '$NEIL_SERVICE'"
                exit 1
                ;;
        esac
        ;;

    weather)
        curl -s "https://api.openweathermap.org/data/2.5/weather?q=$PARAM_location&appid=$NEIL_CRED&units=metric"
        ;;

    # --- plugin: plugins (built-in) ---
    plugins)
        case "$NEIL_ACTION" in
            list)
                "$HOME/.neil/plugins/install.sh" list 2>&1
                ;;
            available)
                "$HOME/.neil/plugins/install.sh" available 2>&1
                ;;
            install)
                "$HOME/.neil/plugins/install.sh" add "$PARAM_name" 2>&1
                ;;
            remove)
                "$HOME/.neil/plugins/install.sh" remove "$PARAM_name" 2>&1
                ;;
            info)
                "$HOME/.neil/plugins/install.sh" info "$PARAM_name" 2>&1
                ;;
            *)
                echo "ERROR: unknown action '$NEIL_ACTION' for plugins"
                exit 1
                ;;
        esac
        ;;

    # --- plugin: web-search ---
    web-search)
        MAX="${PARAM_max:-5}"
        ENCODED=$(printf '%s' "$PARAM_query" | sed 's/ /+/g')
        curl -s "https://html.duckduckgo.com/html/?q=$ENCODED" 2>&1 | \
            grep -oP 'class="result__a"[^>]*href="\K[^"]+' | \
            head -n "$MAX" | while read URL; do
                TITLE=$(curl -s "$URL" 2>/dev/null | grep -oP '<title>\K[^<]+' | head -1)
                echo "- [$TITLE]($URL)"
            done
        ;;

    # --- plugin: wolfram ---
    wolfram)
        ENCODED=$(printf '%s' "$PARAM_input" | sed 's/ /+/g')
        curl -s "https://api.wolframalpha.com/v1/result?appid=$NEIL_CRED&i=$ENCODED" 2>&1
        ;;

    # --- service: vision ---
    vision)
        case "$NEIL_ACTION" in
            look)
                $HOME/.neil/vision/capture.sh auto 2>&1
                ;;
            screenshot)
                $HOME/.neil/vision/capture.sh screenshot 2>&1
                ;;
            pane)
                $HOME/.neil/vision/capture.sh pane "$PARAM_target" 2>&1
                ;;
            camera)
                $HOME/.neil/vision/capture.sh camera "$PARAM_url" 2>&1
                ;;
            inbox)
                ls -t $HOME/.neil/vision/inbox/ 2>/dev/null | head -5
                if [ -z "$(ls $HOME/.neil/vision/inbox/ 2>/dev/null)" ]; then
                    echo "(inbox empty)"
                fi
                ;;
            list)
                $HOME/.neil/vision/capture.sh list 2>&1
                ;;
            *)
                echo "ERROR: unknown action '$NEIL_ACTION' for vision"
                exit 1
                ;;
        esac
        ;;

    spawn_vm)
        sh "$HOME/.neil/tools/spawn_vm/spawn_vm.sh"
        ;;

    peer_send)
        # CALL: peer_send peer=<name> message=<text>
        PEER="$PARAM_peer"
        MSG="$PARAM_message"
        if [ -z "$PEER" ] || [ -z "$MSG" ]; then
            echo "{\"service\":\"peer_send\",\"error\":\"missing peer or message\"}" >&2
            exit 1
        fi
        PEER_IP=$(python3 -c "import json; d=json.load(open('$HOME/.neil/state/peers.json')); rec=d.get('$PEER',{}); print(rec.get('ip','') if rec.get('status')=='ready' else '')" 2>/dev/null)
        if [ -z "$PEER_IP" ]; then
            echo "{\"service\":\"peer_send\",\"error\":\"peer '$PEER' not ready or not found\"}" >&2
            exit 1
        fi
        TS=$(date -u +%Y%m%dT%H%M%SZ)
        SENDER="${NEIL_NODE_ID:-$(hostname)}"
        TMP=$(mktemp --suffix=.md)
        printf '%s\n' "$MSG" > "$TMP"
        scp -q -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
            -i "$HOME/.neil/keys/peer_ed25519" \
            "$TMP" "neil@$PEER_IP:/home/neil/.neil/tools/autoPrompter/queue/${TS}_from_${SENDER}.md"
        SCP_RC=$?
        rm -f "$TMP"
        if [ $SCP_RC -ne 0 ]; then
            echo "{\"service\":\"peer_send\",\"error\":\"scp failed to $PEER@$PEER_IP\"}" >&2
            exit 1
        fi
        # Log to cluster_activity.jsonl
        python3 -c "
import json, pathlib, datetime
p = pathlib.Path('$HOME/.neil/state/cluster_activity.jsonl')
p.parent.mkdir(parents=True, exist_ok=True)
with p.open('a') as f:
    f.write(json.dumps({
        'ts':     datetime.datetime.utcnow().isoformat(timespec='seconds')+'Z',
        'event':  'peer_send',
        'sender': '$SENDER',
        'peer':   '$PEER',
        'peer_ip':'$PEER_IP',
        'bytes':  len('''$MSG''')
    }) + '\n')
"
        echo "{\"service\":\"peer_send\",\"peer\":\"$PEER\",\"peer_ip\":\"$PEER_IP\",\"queued\":\"${TS}_from_${SENDER}.md\"}"
        ;;



    *)
        echo "ERROR: no handler for service '$NEIL_SERVICE'"
        exit 1
        ;;
esac

# Clear credential from env immediately
unset NEIL_CRED
