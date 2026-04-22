#!/bin/sh
# beat_router.sh -- decides which 3C mode this beat should be
#
# Appends a "Beat Directive" section to observe.sh output. See README.md
# for the decision tree and rationale.

set -eu

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
INTENTIONS="$NEIL_HOME/intentions.json"
FAILURES="$NEIL_HOME/self/failures.json"
HB_LOG="$NEIL_HOME/heartbeat_log.json"
ROTATION="$NEIL_HOME/self/study_rotation.txt"
APPROVALS_DIR="$NEIL_HOME/approvals"
ACTIVE_DIR="$NEIL_HOME/tools/autoPrompter/active"

# Check config flag; silently exit if disabled
if grep -q '^mode_routing = false' "$NEIL_HOME/config.toml" 2>/dev/null; then
    exit 0
fi

# OS-layer kill switch: if neil_os_enabled is false, beat_router (an OS-layer
# scheduler feature) must not emit directives. This honors the documented flag
# in essence/overview.md and config.toml [os] section. See ~/.neil/os/.
if grep -qE '^neil_os_enabled[[:space:]]*=[[:space:]]*(false|0)' "$NEIL_HOME/config.toml" 2>/dev/null; then
    # Flag is explicitly disabled; skip directive emission entirely
    exit 0
fi

# ---- detect prompt origin (cron heartbeat vs user chat) ----
# A cron heartbeat has a filename matching *_heartbeat.md in active/.
# Chat prompts use *_chat.md (or any other suffix). Intentions/failures
# marked "requires_chat_override":true are chat-gated and MUST NOT be
# selected when this beat is cron-originated -- their precondition
# (operator-authored OVERRIDE + CALL block) is structurally unavailable
# from cron context per guardrails.md.
is_cron_heartbeat=0
if [ -d "$ACTIVE_DIR" ]; then
    for f in "$ACTIVE_DIR"/*.md; do
        [ -e "$f" ] || continue
        case "$(basename "$f")" in
            *_heartbeat.md) is_cron_heartbeat=1 ;;
        esac
    done
fi

# ---- gather state ----

# Detect approval signals: any *.md in ~/.neil/approvals/ other than README.md
approval_signals_exist=0
if [ -d "$APPROVALS_DIR" ]; then
    for f in "$APPROVALS_DIR"/*.md; do
        [ -e "$f" ] || continue
        case "$(basename "$f")" in
            README.md) continue ;;
            *) approval_signals_exist=1; break ;;
        esac
    done
fi

# Pending intentions (skipping approval-gated and chat-gated when not applicable)
pending_count=0
oldest_pending=""
skipped_approval=0
skipped_chat_gated=0
if [ -f "$INTENTIONS" ]; then
    while IFS= read -r line; do
        [ -z "$line" ] && continue
        case "$line" in
            *'"status":"pending"'*)
                # Extract tag to check if this is approval-gated
                itag=$(echo "$line" | sed 's/.*"tag":"\([^"]*\)".*/\1/')
                if [ "$itag" = "approval" ] && [ "$approval_signals_exist" -eq 0 ]; then
                    skipped_approval=$((skipped_approval + 1))
                    continue
                fi
                # Skip chat-gated intentions from cron heartbeats. Precondition
                # (operator OVERRIDE + CALL block) cannot be met in cron context.
                case "$line" in
                    *'"requires_chat_override":true'*|*'"requires_chat_override": true'*)
                        if [ "$is_cron_heartbeat" -eq 1 ]; then
                            skipped_chat_gated=$((skipped_chat_gated + 1))
                            continue
                        fi
                        ;;
                esac
                pending_count=$((pending_count + 1))
                if [ -z "$oldest_pending" ]; then
                    oldest_pending=$(echo "$line" | sed 's/.*"description":"\([^"]*\)".*/\1/' | cut -c1-100)
                fi
                ;;
        esac
    done < "$INTENTIONS"
fi

# Unresolved failures (highest severity wins; skip approval-tagged and chat-gated)
failure_count=0
top_failure=""
top_severity="low"
skipped_approval_fail=0
skipped_chat_gated_fail=0
if [ -f "$FAILURES" ]; then
    sev_rank() {
        case "$1" in
            critical) echo 4 ;;
            high) echo 3 ;;
            medium) echo 2 ;;
            *) echo 1 ;;
        esac
    }
    top_rank=0
    while IFS= read -r line; do
        [ -z "$line" ] && continue
        case "$line" in
            *'"resolution":"pending"'*)
                # Skip approval-tagged failures when no approval signal present
                ftag=""
                case "$line" in
                    *'"tag":"'*) ftag=$(echo "$line" | sed 's/.*"tag":"\([^"]*\)".*/\1/') ;;
                esac
                if [ "$ftag" = "approval" ] && [ "$approval_signals_exist" -eq 0 ]; then
                    skipped_approval_fail=$((skipped_approval_fail + 1))
                    continue
                fi
                # Skip chat-gated failures from cron heartbeats.
                case "$line" in
                    *'"requires_chat_override":true'*|*'"requires_chat_override": true'*)
                        if [ "$is_cron_heartbeat" -eq 1 ]; then
                            skipped_chat_gated_fail=$((skipped_chat_gated_fail + 1))
                            continue
                        fi
                        ;;
                esac
                failure_count=$((failure_count + 1))
                sev=$(echo "$line" | sed 's/.*"severity":"\([^"]*\)".*/\1/')
                rank=$(sev_rank "$sev")
                if [ "$rank" -gt "$top_rank" ]; then
                    top_rank=$rank
                    top_severity="$sev"
                    top_failure=$(echo "$line" | sed 's/.*"error":"\([^"]*\)".*/\1/' | cut -c1-100)
                fi
                ;;
        esac
    done < "$FAILURES"
fi

# Classify last 5 beats by mode (newest first)
classify_action() {
    a=$(echo "$1" | tr '[:upper:]' '[:lower:]')
    case "$a" in
        *read*|*traced*|*mapped*|*studied*|*profiled*|*deep-read*|*examined*|*inspect*|*surveyed*|*honored*)
            echo CONFIGURATION ;;
        *verified*|*confirmed*|*tested*|*validated*|*diagnosed*)
            echo CHARACTERIZATION ;;
        *fixed*|*built*|*deployed*|*wrote*|*created*|*added*|*patched*|*integrated*|*shipped*|*drafted*|*ran*)
            echo CREATIVITY ;;
        *) echo UNKNOWN ;;
    esac
}

last5_modes=""  # space-separated, oldest-to-newest
last_mode=""
if [ -f "$HB_LOG" ]; then
    tail -5 "$HB_LOG" > /tmp/.br_last5 2>/dev/null || true
    if [ -s /tmp/.br_last5 ]; then
        while IFS= read -r line; do
            action=$(echo "$line" | sed 's/.*"action":"\([^"]*\)".*/\1/')
            m=$(classify_action "$action")
            last5_modes="$last5_modes $m"
            last_mode=$m
        done < /tmp/.br_last5
        rm -f /tmp/.br_last5
    fi
fi

# Last 3 modes (the 3 most recent)
last3=$(echo "$last5_modes" | awk '{n=NF; out=""; for(i=(n<3?1:n-2);i<=n;i++) out=out" "$i; print out}')

# ---- decision tree ----

mode=""
reason=""
target=""
required=""
forbidden=""

# Build a combined "skipped" tail for human-readable reasons
skip_tail=""
if [ "$skipped_approval" -gt 0 ] || [ "$skipped_chat_gated" -gt 0 ]; then
    skip_tail=" (skipped"
    [ "$skipped_approval" -gt 0 ]   && skip_tail="$skip_tail $skipped_approval approval-gated"
    [ "$skipped_chat_gated" -gt 0 ] && skip_tail="$skip_tail $skipped_chat_gated chat-gated"
    skip_tail="$skip_tail)"
fi
skip_tail_fail=""
if [ "$skipped_approval_fail" -gt 0 ] || [ "$skipped_chat_gated_fail" -gt 0 ]; then
    skip_tail_fail=" (skipped"
    [ "$skipped_approval_fail" -gt 0 ]   && skip_tail_fail="$skip_tail_fail $skipped_approval_fail approval-gated"
    [ "$skipped_chat_gated_fail" -gt 0 ] && skip_tail_fail="$skip_tail_fail $skipped_chat_gated_fail chat-gated"
    skip_tail_fail="$skip_tail_fail)"
fi

if [ "$pending_count" -gt 0 ]; then
    # Rule 1: pending intention
    mode="CREATIVITY"
    reason="$pending_count pending intention(s) -- execute oldest$skip_tail"
    target="$oldest_pending"
    required="DONE: (completed) or FAIL: (blocked) or INTEND: (refined followup)"
    forbidden="starting new initiative work while intentions pending"

elif [ "$failure_count" -gt 0 ]; then
    # Rule 2: unresolved failure
    mode="CREATIVITY"
    reason="$failure_count unresolved FAIL(s); highest severity: $top_severity$skip_tail_fail"
    target="$top_failure"
    required="DONE: (fixed) or FAIL: (still broken + diagnosis) or INTEND: (specific fix plan)"
    forbidden="ignoring the failure; starting unrelated initiative work"

elif echo "$last3" | grep -q "CREATIVITY CREATIVITY CREATIVITY" \
     && ! echo "$last5_modes" | grep -q "CONFIGURATION"; then
    # Rule 3: anti-proposal-spam gate
    mode="CONFIGURATION"
    reason="3 creativity beats without prior configuration -- anti-proposal-spam gate active"
    required="MEMORY: of ground truth; INTEND: if something actionable emerges"
    forbidden="writing code; proposing architectures; speculation without reading"

elif [ "$last_mode" = "CONFIGURATION" ]; then
    # Rule 4: last beat was CONFIG, characterize what it found
    mode="CHARACTERIZATION"
    reason="last beat was configuration -- verify findings before creating"
    target="the finding from last beat's MEMORY note"
    required="DONE: (works as thought) or FAIL: (real problem) or INTEND: (specific next work)"
    forbidden="restarting from scratch; ignoring the last beat"

else
    # Default: configuration + rotation
    mode="CONFIGURATION"
    total_skipped=$((skipped_approval + skipped_approval_fail + skipped_chat_gated + skipped_chat_gated_fail))
    if [ "$total_skipped" -gt 0 ]; then
        reason="no pending non-gated work ($total_skipped awaiting operator/chat); grounding before acting"
    else
        reason="no pending work; grounding before acting"
    fi
    required="MEMORY: of ground truth; INTEND: if something actionable emerges"
    forbidden="writing code; proposing architectures; speculation without reading"
fi

# ---- target rotation for CONFIGURATION mode ----

if [ "$mode" = "CONFIGURATION" ] && [ -z "$target" ]; then
    if [ ! -f "$ROTATION" ]; then
        mkdir -p "$(dirname "$ROTATION")"
        cat > "$ROTATION" <<ROTEOF
essence|0
autoprompter|0
agent|0
blueprint|0
memory|0
services|0
outputs|0
config|0
ROTEOF
    fi

    target_area=$(sort -t'|' -k2 -n "$ROTATION" | head -1 | cut -d'|' -f1)
    now=$(date +%s)

    tmp=$(mktemp)
    while IFS='|' read -r area ts; do
        if [ "$area" = "$target_area" ]; then
            echo "$area|$now"
        else
            echo "$area|$ts"
        fi
    done < "$ROTATION" > "$tmp"
    mv "$tmp" "$ROTATION"

    case "$target_area" in
        essence) target="~/.neil/essence/ -- identity, soul, mission, philosophy" ;;
        autoprompter) target="~/.neil/tools/autoPrompter/src/autoprompt.c -- pick one function you have not studied" ;;
        agent) target="~/.neil/tools/autoPrompter/agent/neil_agent.py -- tool definitions + streaming" ;;
        blueprint) target="~/.neil/blueprint/src/ -- a renderer you have not studied" ;;
        memory) target="~/.neil/memory/ -- zettel structure, mempalace index, palace layout" ;;
        services) target="~/.neil/services/registry/ -- service definitions and handler.sh" ;;
        outputs) target="~/.neil/outputs/ -- recent logs, channel scripts" ;;
        config) target="~/.neil/config.toml + cron -- invocation parameters and schedules" ;;
    esac
fi

# ---- emit directive ----

echo ""
echo "=== Beat Directive ==="
echo "mode: $mode"
echo "reason: $reason"
echo "target: $target"
echo "required: $required"
echo "forbidden: $forbidden"
