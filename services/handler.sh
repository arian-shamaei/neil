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
    # Parse $NEIL_PARAMS into PARAM_* env vars. Uses Python shlex to handle
    # escaped quotes inside quoted values correctly.
    [ -z "$NEIL_PARAMS" ] && return 0
    TMP_PARAMS=$(mktemp)
    NEIL_PARAMS="$NEIL_PARAMS" python3 -c "
import shlex, os, sys
raw = os.environ.get('NEIL_PARAMS', '')
if not raw.strip():
    sys.exit(0)
try:
    tokens = shlex.split(raw)
except Exception as e:
    sys.stderr.write(f'[handler] eval_params shlex error: {e}\n')
    sys.exit(0)
for t in tokens:
    if '=' in t:
        k, v = t.split('=', 1)
        print(f'export PARAM_{k}={shlex.quote(v)}')
" > "$TMP_PARAMS"
    . "$TMP_PARAMS"
    rm -f "$TMP_PARAMS"
}
eval_params

# validate_params: compare PARAM_* env names against key= tokens declared
# anywhere in registry/$NEIL_SERVICE.md. Unknown params produce a loud FAIL
# line in outputs/neil.log but do NOT abort dispatch (preserves backward
# compatibility; goal is detection, not enforcement).
validate_params() {
    REG="$HOME/.neil/services/registry/${NEIL_SERVICE}.md"
    [ -f "$REG" ] || return 0
    VALID_PARAMS=$(REG_FILE="$REG" python3 -c '
import re, os, sys
try:
    content = open(os.environ["REG_FILE"]).read()
except Exception:
    sys.exit(0)
found = set()
# Any bare "ident=" token anywhere in the registry file. Excludes dispatch
# keys (service, action) and the service: / phase: / category: YAML header
# keys. This is the union across every CALL example, table row, and prose
# reference in the file.
for m in re.finditer(r"\b([a-z_][a-z0-9_]*)=", content):
    found.add(m.group(1))
for skip in ("service", "action", "category", "phase", "status"):
    found.discard(skip)
print(" ".join(sorted(found)))
' 2>/dev/null || echo "")
    [ -z "$VALID_PARAMS" ] && return 0
    UNKNOWN=""
    for ENTRY in $(set | grep "^PARAM_" | sed "s/=.*//"); do
        PNAME=$(echo "$ENTRY" | sed "s/^PARAM_//")
        PNAME_LC=$(echo "$PNAME" | tr "[:upper:]" "[:lower:]")
        case " $VALID_PARAMS " in
            *" $PNAME_LC "*) ;;
            *) UNKNOWN="$UNKNOWN $PNAME" ;;
        esac
    done
    if [ -n "$UNKNOWN" ]; then
        TS=$(date -Iseconds 2>/dev/null || date)
        MSG="validate_params service=$NEIL_SERVICE action=$NEIL_ACTION unknown:$UNKNOWN declared=($VALID_PARAMS)"
        mkdir -p "$HOME/.neil/outputs"
        echo "[$TS] FAIL source=handler severity=medium | $MSG" >> "$HOME/.neil/outputs/neil.log"
        echo "[handler] $MSG" >&2
    fi
}
validate_params

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

    # --- service: gstack ---
    # Bridge to gstack skill prompts (see services/registry/gstack.md).
    # action = skill name (e.g. retro, plan-eng-review)
    # params: context="<text passed as user prompt>"
    gstack)
        SKILL=$NEIL_ACTION
        # Reject any skill name with non-safe chars (path traversal defense).
        # Skills are alphanumeric + underscore + hyphen only — matches the
        # naming convention every gstack skill on disk uses.
        case "$SKILL" in
            *[!a-zA-Z0-9_-]*|"")
                echo "{\"service\":\"gstack\",\"error\":\"invalid skill name (must match [a-zA-Z0-9_-]+)\"}" >&2
                exit 1
                ;;
        esac
        CONTEXT="${PARAM_context:-}"
        SKILL_FILE="$HOME/.neil/skills/gstack/$SKILL/SKILL.md"
        # Reject symlinks: SKILL_FILE must be a regular file, not a link to elsewhere.
        if [ -L "$SKILL_FILE" ]; then
            echo "{\"service\":\"gstack\",\"error\":\"skill file is a symlink — refusing for safety\"}" >&2
            exit 1
        fi
        if [ ! -f "$SKILL_FILE" ]; then
            echo "{\"service\":\"gstack\",\"error\":\"skill '$SKILL' not installed at $SKILL_FILE\"}" >&2
            exit 1
        fi
        if [ -z "$CONTEXT" ]; then
            echo "{\"service\":\"gstack\",\"error\":\"missing context param\"}" >&2
            exit 1
        fi
        # Run neil_agent.py with the gstack skill as system prompt and
        # caller's context as user prompt. If PARAM_cwd is provided and is
        # an existing directory, run the agent inside that directory so
        # gstack's Bash invocations (git diff, ls, etc) see the right repo.
        SYS=$(cat "$SKILL_FILE")
        CWD="${PARAM_cwd:-$HOME}"
        if [ ! -d "$CWD" ]; then
            echo "{\"service\":\"gstack\",\"error\":\"cwd '$CWD' is not a directory\"}" >&2
            exit 1
        fi
        OUT=$(cd "$CWD" && NEIL_HOME="$HOME/.neil" NEIL_MAX_TURNS=25 \
            "$HOME/.neil/tools/autoPrompter/agent/.venv/bin/python" \
            "$HOME/.neil/tools/autoPrompter/agent/neil_agent.py" \
            --system-prompt "$SYS" -p "$CONTEXT" 2>&1)
        AGENT_RC=$?
        # Log invocation
        python3 - "$HOME/.neil/state/cluster_activity.jsonl" "$SKILL" "$AGENT_RC" "$OUT" <<'LOG'
import json, pathlib, sys, datetime
p, skill, rc, out = sys.argv[1:5]
pp = pathlib.Path(p); pp.parent.mkdir(parents=True, exist_ok=True)
with pp.open("a") as f:
    f.write(json.dumps({
        "ts":          datetime.datetime.utcnow().isoformat(timespec="seconds")+"Z",
        "event":       "gstack_invoke" if rc == "0" else "gstack_invoke_fail",
        "skill":       skill,
        "agent_rc":    int(rc),
        "out_chars":   len(out),
        "out_head":    out[:300],
    }) + "\n")
LOG
        if [ $AGENT_RC -ne 0 ]; then
            echo "{\"service\":\"gstack\",\"skill\":\"$SKILL\",\"error\":\"agent rc=$AGENT_RC\"}" >&2
            printf '%s\n' "$OUT" >&2
            exit 1
        fi
        echo "{\"service\":\"gstack\",\"skill\":\"$SKILL\",\"out_chars\":$(printf '%s' "$OUT" | wc -c)}"
        # Emit the skill's output so Neil can continue reasoning on it
        printf '\n=== gstack/%s output ===\n%s\n' "$SKILL" "$OUT"
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
        # CALL: peer_send peer=<name> message=<text> [action=queue|exec]
        # action=exec (default): Runs neil_agent.py on peer synchronously over
        #                        SSH, captures reply, emits as PROMPT:.
        # action=queue:          Drops a prompt .md file into the peer's
        #                        autoPrompter queue via scp and returns
        #                        immediately. Peer processes on next autoprompt
        #                        cycle. Organic-conversation primitive.
        PEER="$PARAM_peer"
        MSG="$PARAM_message"
        # Action source: explicit PARAM_action wins over NEIL_ACTION env.
        ACTION="${PARAM_action:-${NEIL_ACTION:-exec}}"
        if [ -z "$PEER" ] || [ -z "$MSG" ]; then
            echo "{\"service\":\"peer_send\",\"error\":\"missing peer or message\"}" >&2
            exit 1
        fi
        PEER_IP=$(python3 -c "import json; d=json.load(open('$HOME/.neil/state/peers.json')); rec=d.get('$PEER',{}); print(rec.get('ip','') if rec.get('status')=='ready' else '')" 2>/dev/null)
        if [ -z "$PEER_IP" ]; then
            echo "{\"service\":\"peer_send\",\"error\":\"peer '$PEER' not ready or not found\"}" >&2
            exit 1
        fi
        SENDER="${NEIL_NODE_ID:-$(hostname)}"

        # ── async queue-drop branch ──
        if [ "$ACTION" = "queue" ]; then
            TS=$(date -u +%Y%m%dT%H%M%S)
            TMP=$(mktemp --suffix=.md)
            printf '%s\n' "$MSG" > "$TMP"
            DEST="/home/neil/.neil/tools/autoPrompter/queue/${TS}_from_${SENDER}.md"
            scp -q -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
                -o BatchMode=yes -o ConnectTimeout=5 \
                -i "$HOME/.neil/keys/peer_ed25519" \
                "$TMP" "neil@$PEER_IP:$DEST"
            SCP_RC=$?
            BYTES=$(stat -c '%s' "$TMP" 2>/dev/null || echo 0)
            rm -f "$TMP"
            python3 - "$HOME/.neil/state/cluster_activity.jsonl" "$SENDER" "$PEER" "$PEER_IP" "$SCP_RC" "$BYTES" "$DEST" <<'LOG'
import json, pathlib, sys, datetime
p, sender, peer, ip, rc, bytes_, dest = sys.argv[1:8]
pp = pathlib.Path(p); pp.parent.mkdir(parents=True, exist_ok=True)
with pp.open("a") as f:
    f.write(json.dumps({
        "ts":     datetime.datetime.utcnow().isoformat(timespec="seconds")+"Z",
        "event":  "peer_send_queued" if rc == "0" else "peer_send_queue_fail",
        "sender": sender, "peer": peer, "peer_ip": ip,
        "dest":   dest,
        "bytes":  int(bytes_),
        "rc":     int(rc),
    }) + "\n")
LOG
            if [ "$SCP_RC" -ne 0 ]; then
                echo "{\"service\":\"peer_send\",\"action\":\"queue\",\"error\":\"scp rc=$SCP_RC\"}" >&2
                exit 1
            fi
            echo "{\"service\":\"peer_send\",\"action\":\"queue\",\"peer\":\"$PEER\",\"peer_ip\":\"$PEER_IP\",\"dest\":\"$DEST\",\"bytes\":$BYTES}"
            exit 0
        fi

        # Escape the message for embedding in a single-quoted ssh arg.
        MSG_ESC=$(printf '%s' "$MSG" | sed "s/'/'\\\\''/g")
        SYS_ESC="You are peer Neil '$PEER' on a spawned VM. A sibling Neil ('$SENDER') is sending you this message. Read your ~/.neil/essence/ and ~/.neil/state/spawn_config.json for your role. Reply concisely via the same peer_send channel if a response is expected."

        # Log the outbound (before execution so failures are visible in activity log)
        python3 - "$HOME/.neil/state/cluster_activity.jsonl" "$SENDER" "$PEER" "$PEER_IP" "sent" "$MSG" <<'LOG'
import json, pathlib, sys, datetime
p, sender, peer, ip, event, msg = sys.argv[1:7]
pp = pathlib.Path(p); pp.parent.mkdir(parents=True, exist_ok=True)
with pp.open("a") as f:
    f.write(json.dumps({
        "ts":      datetime.datetime.utcnow().isoformat(timespec="seconds")+"Z",
        "event":   "peer_send_" + event,
        "sender":  sender, "peer": peer, "peer_ip": ip,
        "bytes":   len(msg),
    }) + "\n")
LOG

        # Exec neil_agent.py on peer synchronously
        REPLY=$(ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
                    -o BatchMode=yes -o ConnectTimeout=10 \
                    -i "$HOME/.neil/keys/peer_ed25519" \
                    "neil@$PEER_IP" \
                    "NEIL_HOME=/home/neil/.neil /home/neil/.neil/tools/autoPrompter/agent/.venv/bin/python /home/neil/.neil/tools/autoPrompter/agent/neil_agent.py --system-prompt '$SYS_ESC' -p '$MSG_ESC' 2>&1" 2>&1)
        SSH_RC=$?

        # Log completion
        python3 - "$HOME/.neil/state/cluster_activity.jsonl" "$SENDER" "$PEER" "$PEER_IP" "$SSH_RC" "$REPLY" <<'LOG'
import json, pathlib, sys, datetime
p, sender, peer, ip, rc, reply = sys.argv[1:7]
pp = pathlib.Path(p)
with pp.open("a") as f:
    f.write(json.dumps({
        "ts":         datetime.datetime.utcnow().isoformat(timespec="seconds")+"Z",
        "event":      "peer_send_complete" if rc == "0" else "peer_send_fail",
        "sender":     sender, "peer": peer, "peer_ip": ip,
        "ssh_rc":     int(rc),
        "reply_chars": len(reply),
        "reply_head":  reply[:300],
    }) + "\n")
LOG

        if [ "$SSH_RC" -ne 0 ]; then
            echo "{\"service\":\"peer_send\",\"peer\":\"$PEER\",\"error\":\"ssh/agent rc=$SSH_RC\",\"reply_head\":$(python3 -c "import json,sys; print(json.dumps(sys.argv[1][:300]))" "$REPLY")}" >&2
            exit 1
        fi
        echo "{\"service\":\"peer_send\",\"peer\":\"$PEER\",\"peer_ip\":\"$PEER_IP\",\"reply_chars\":$(printf '%s' "$REPLY" | wc -c)}"
        # Emit peer's reply as a PROMPT: so Neil can continue reasoning on it
        printf '\nPROMPT: [peer=%s reply] %s\n' "$PEER" "$REPLY"
        ;;



    peer_transfer)
        # CALL: peer_transfer peer=<name> direction=push|pull source=<path> dest=<path> [recursive=true]
        PEER="$PARAM_peer"
        DIR="${PARAM_direction:-push}"
        SRC="$PARAM_source"
        DST="$PARAM_dest"
        REC="${PARAM_recursive:-false}"
        if [ -z "$PEER" ] || [ -z "$SRC" ] || [ -z "$DST" ]; then
            echo "{\"service\":\"peer_transfer\",\"error\":\"missing peer/source/dest\"}" >&2
            exit 1
        fi
        PEER_IP=$(python3 -c "import json; d=json.load(open('$HOME/.neil/state/peers.json')); rec=d.get('$PEER',{}); print(rec.get('ip','') if rec.get('status')=='ready' else '')" 2>/dev/null)
        if [ -z "$PEER_IP" ]; then
            echo "{\"service\":\"peer_transfer\",\"error\":\"peer '$PEER' not ready or not found\"}" >&2
            exit 1
        fi
        SENDER="${NEIL_NODE_ID:-$(hostname)}"
        FLAGS="-q -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -i $HOME/.neil/keys/peer_ed25519"
        [ "$REC" = "true" ] && FLAGS="$FLAGS -r"

        case "$DIR" in
            push)
                scp $FLAGS "$SRC" "neil@$PEER_IP:$DST"
                SCP_RC=$?
                ;;
            pull)
                scp $FLAGS "neil@$PEER_IP:$SRC" "$DST"
                SCP_RC=$?
                ;;
            *)
                echo "{\"service\":\"peer_transfer\",\"error\":\"bad direction '$DIR' (want push|pull)\"}" >&2
                exit 1
                ;;
        esac

        # Best-effort byte count
        BYTES=0
        if [ $SCP_RC -eq 0 ]; then
            if [ "$DIR" = "push" ]; then
                BYTES=$(du -sb "$SRC" 2>/dev/null | awk '{print $1}')
            else
                BYTES=$(du -sb "$DST" 2>/dev/null | awk '{print $1}')
            fi
        fi
        [ -z "$BYTES" ] && BYTES=0

        python3 - "$HOME/.neil/state/cluster_activity.jsonl" "$SENDER" "$PEER" "$PEER_IP" "$DIR" "$SRC" "$DST" "$BYTES" "$SCP_RC" <<'LOG'
import json, pathlib, sys, datetime
p, sender, peer, ip, direction, src, dst, bytes_, rc = sys.argv[1:10]
pp = pathlib.Path(p); pp.parent.mkdir(parents=True, exist_ok=True)
with pp.open("a") as f:
    f.write(json.dumps({
        "ts":         datetime.datetime.utcnow().isoformat(timespec="seconds")+"Z",
        "event":      f"peer_transfer_{direction}" if rc == "0" else f"peer_transfer_{direction}_fail",
        "sender":     sender, "peer": peer, "peer_ip": ip,
        "source":     src, "dest": dst,
        "bytes":      int(bytes_), "rc": int(rc),
    }) + "\n")
LOG

        if [ $SCP_RC -ne 0 ]; then
            echo "{\"service\":\"peer_transfer\",\"peer\":\"$PEER\",\"direction\":\"$DIR\",\"error\":\"scp rc=$SCP_RC\",\"source\":\"$SRC\",\"dest\":\"$DST\"}" >&2
            exit 1
        fi
        echo "{\"service\":\"peer_transfer\",\"peer\":\"$PEER\",\"direction\":\"$DIR\",\"bytes\":$BYTES,\"source\":\"$SRC\",\"dest\":\"$DST\"}"
        ;;

    *)
        echo "ERROR: no handler for service '$NEIL_SERVICE'"
        exit 1
        ;;
esac

# Clear credential from env immediately
unset NEIL_CRED
