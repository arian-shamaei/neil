#!/bin/sh
# modes_overrides_logged.sh
#
# Invariants:
#   (a) Every user chat prompt with "OVERRIDE: mode=<...>" on its first
#       non-blank line must produce a matching "MODE_OVERRIDE: source=user
#       mode=<...>" line in its result, OR a FAIL with reason "override_malformed".
#   (b) No cron heartbeat ("*_heartbeat.md") result may contain a
#       MODE_OVERRIDE line — that would be self-escalation.
#
# Exit: 0 = pass, 1 = violation, 2 = no history to check yet.

HIST=$HOME/.neil/tools/autoPrompter/history
[ -d "$HIST" ] || { echo "pending: no history dir"; exit 2; }

VIOL=0

# --- Invariant (b): no MODE_OVERRIDE in cron heartbeats ---
for r in "$HIST"/*_heartbeat.md.result.md; do
    [ -e "$r" ] || continue
    if grep -q "^MODE_OVERRIDE:" "$r"; then
        echo "VIOLATION (b): $r (cron heartbeat) contains MODE_OVERRIDE"
        VIOL=1
    fi
done

# --- Invariant (a): every chat prompt with OVERRIDE must ack or FAIL ---
for r in "$HIST"/*_chat.md.result.md; do
    [ -e "$r" ] || continue
    # Corresponding prompt file
    SRC="${r%.result.md}"
    [ -f "$SRC" ] || continue
    FIRST=$(awk 'NF {print; exit}' "$SRC")
    case "$FIRST" in
        OVERRIDE:*)
            # Override was requested — result must ack or FAIL
            if grep -q "^MODE_OVERRIDE:" "$r"; then
                : # acknowledged
            elif grep -qE "^FAIL:.*override_malformed" "$r"; then
                : # failed-as-designed
            else
                echo "VIOLATION (a): $SRC requested OVERRIDE but result has neither MODE_OVERRIDE nor FAIL: $r"
                VIOL=1
            fi
            ;;
    esac
done

if [ "$VIOL" -eq 0 ]; then
    echo "OK: user_override invariants hold across all history"
    exit 0
fi
exit 1
