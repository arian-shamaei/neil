#!/bin/sh
# beat_taxonomy.sh -- Classify heartbeat work patterns and recommend next focus
#
# Categories:
#   fix     - Bug fixes, error resolution, root-cause analysis
#   create  - Building new capabilities, designing systems
#   learn   - Studying code, researching, deep-diving
#   recover - Wakeup, crash recovery, re-orientation
#   idle    - Standing by, no actionable work
#
# Outputs:
#   - Distribution of work types over last N beats
#   - Current work mode (what dominates recent activity)
#   - Recommendation for next beat based on pattern balance
#
# Usage: beat_taxonomy.sh [window_size]

LOG="$HOME/.neil/heartbeat_log.json"
WINDOW="${1:-10}"

if [ ! -f "$LOG" ] || [ ! -s "$LOG" ]; then
    echo "taxonomy: unknown (no data)"
    exit 0
fi

TOTAL=$(wc -l < "$LOG")
if [ "$TOTAL" -lt "$WINDOW" ]; then
    WINDOW="$TOTAL"
fi

# Classify each beat by keyword matching on summary
FIX=0
CREATE=0
LEARN=0
RECOVER=0
IDLE=0

# Use temp file to escape subshell variable scope
COUNTS=$(mktemp)
echo "0 0 0 0 0" > "$COUNTS"

tail -"$WINDOW" "$LOG" | while IFS= read -r line; do
    SUMMARY=$(echo "$line" | sed 's/.*"summary":"\([^"]*\)".*/\1/' | tr 'A-Z' 'a-z')
    STATUS=$(echo "$line" | sed 's/.*"status":"\([^"]*\)".*/\1/')

    # Skip unknown/empty beats
    if [ "$STATUS" = "unknown" ] || [ -z "$SUMMARY" ]; then
        read FIX CREATE LEARN RECOVER IDLE < "$COUNTS"
        IDLE=$((IDLE + 1))
        echo "$FIX $CREATE $LEARN $RECOVER $IDLE" > "$COUNTS"
        continue
    fi

    read FIX CREATE LEARN RECOVER IDLE < "$COUNTS"

    CLASSIFIED=0

    # 1. Recover: wakeup, crash recovery (check FIRST -- these often mention fixes)
    case "$SUMMARY" in
        *woke\ up*|*wakeup*|*recover*|*re-orient*)
            RECOVER=$((RECOVER + 1)); CLASSIFIED=1 ;;
    esac

    # 2. Idle: standing by, no work
    if [ "$CLASSIFIED" -eq 0 ]; then
        case "$SUMMARY" in
            *standing\ by*|*nominal*|*stray*|*no\ action*|*idle*|*pausing*)
                IDLE=$((IDLE + 1)); CLASSIFIED=1 ;;
        esac
    fi

    # 3. Learn: research, study, deep-dive (check before fix -- deep-dives find bugs)
    if [ "$CLASSIFIED" -eq 0 ]; then
        case "$SUMMARY" in
            *stud*|*research*|*deep-d*|*analyz*|*discover*|*understand*|*investigat*|*profil*)
                LEARN=$((LEARN + 1)); CLASSIFIED=1 ;;
        esac
    fi

    # 4. Fix: bug fixes, error resolution (narrow patterns to avoid false matches)
    if [ "$CLASSIFIED" -eq 0 ]; then
        case "$SUMMARY" in
            *fixed*|*bug*|*broke*|*repair*|*patch*|*failure\ rate*)
                FIX=$((FIX + 1)); CLASSIFIED=1 ;;
        esac
    fi

    # 5. Create: building, designing, implementing (default for acted beats)
    if [ "$CLASSIFIED" -eq 0 ]; then
        case "$SUMMARY" in
            *built*|*creat*|*added*|*design*|*implement*|*develop*|*wrote*|*integrat*|*prototype*)
                CREATE=$((CREATE + 1)); CLASSIFIED=1 ;;
        esac
    fi

    # Fallback: if acted but not classified, it's probably create
    if [ "$CLASSIFIED" -eq 0 ]; then
        if [ "$STATUS" = "acted" ]; then
            CREATE=$((CREATE + 1))
        else
            IDLE=$((IDLE + 1))
        fi
    fi

    echo "$FIX $CREATE $LEARN $RECOVER $IDLE" > "$COUNTS"
done

read FIX CREATE LEARN RECOVER IDLE < "$COUNTS"
rm -f "$COUNTS"

# Find dominant category
MAX=0
MODE="idle"
for CAT_NAME in fix create learn recover idle; do
    eval "VAL=\$$( echo "$CAT_NAME" | tr 'a-z' 'A-Z')"
    if [ "$VAL" -gt "$MAX" ]; then
        MAX="$VAL"
        MODE="$CAT_NAME"
    fi
done

# Calculate active beats (non-idle, non-recover)
ACTIVE=$((FIX + CREATE + LEARN))

# Generate recommendation based on balance
RECOMMENDATION=""
if [ "$WINDOW" -ge 5 ]; then
    # Too many recoveries -- systemic instability (highest priority)
    if [ "$RECOVER" -ge 3 ]; then
        RECOMMENDATION="Frequent recoveries -- address root cause of instability"
    # Too much fixing -- switch to creative work
    elif [ "$FIX" -gt 0 ] && [ "$ACTIVE" -gt 0 ]; then
        FIX_PCT=$((FIX * 100 / ACTIVE))
        if [ "$FIX_PCT" -ge 60 ]; then
            RECOMMENDATION="Heavy fix mode -- consider creative or learning work"
        fi
    fi

    # Only check build/study balance if no recommendation yet
    if [ -z "$RECOMMENDATION" ] && [ "$ACTIVE" -ge 3 ]; then
        if [ "$CREATE" -gt 0 ] && [ "$LEARN" -eq 0 ]; then
            RECOMMENDATION="All build, no study -- pause to understand before building more"
        elif [ "$LEARN" -gt 0 ] && [ "$CREATE" -eq 0 ]; then
            RECOMMENDATION="All study, no build -- apply what you've learned"
        else
            RECOMMENDATION="Balanced work pattern -- keep diversifying"
        fi
    fi
fi

# Output
echo "work pattern (last $WINDOW beats):"
echo "  fix=$FIX create=$CREATE learn=$LEARN recover=$RECOVER idle=$IDLE"
echo "  mode: $MODE"
if [ -n "$RECOMMENDATION" ]; then
    echo "  insight: $RECOMMENDATION"
fi
