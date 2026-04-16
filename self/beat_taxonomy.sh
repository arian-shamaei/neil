#!/bin/sh
# beat_taxonomy.sh -- Classify heartbeat work patterns with temporal awareness
#
# Categories:
#   fix     - Bug fixes, error resolution, root-cause analysis
#   create  - Building new capabilities, designing systems
#   learn   - Studying code, researching, deep-diving
#   recover - Wakeup, crash recovery, re-orientation
#   idle    - Standing by, no actionable work
#
# Temporal awareness:
#   Splits window into "recent" (last 3) and "history" (older).
#   When the dominant mode shifts between halves, reports a trend.
#   Recommendations use the RECENT mode, not the aggregate, to avoid
#   stale advice after a problem class has been resolved.
#
# Usage: beat_taxonomy.sh [window_size]

LOG="$HOME/.neil/heartbeat_log.json"
WINDOW="${1:-10}"
RECENT_SIZE=3  # How many beats count as "recent"

if [ ! -f "$LOG" ] || [ ! -s "$LOG" ]; then
    echo "taxonomy: unknown (no data)"
    exit 0
fi

TOTAL=$(wc -l < "$LOG")
if [ "$TOTAL" -lt "$WINDOW" ]; then
    WINDOW="$TOTAL"
fi

# classify_beat: takes a summary and status, prints a category
classify_beat() {
    _SUMMARY=$(echo "$1" | tr 'A-Z' 'a-z')
    _STATUS="$2"

    if [ "$_STATUS" = "unknown" ] || [ -z "$_SUMMARY" ]; then
        echo "idle"; return
    fi

    # 1. Recover (check first -- but only if summary STARTS with recovery language)
    #    "woke up..." / "5th wakeup in same batch" = recovery beat
    #    "implemented wakeup dedup guard" / "documented wakeup storm" = NOT recovery
    case "$_SUMMARY" in
        woke\ up*|*wakeup\ in\ *|re-orient*) echo "recover"; return ;;
    esac
    # 2. Idle
    case "$_SUMMARY" in
        *standing\ by*|*nominal*|*stray*|*no\ action*|*idle*|*pausing*) echo "idle"; return ;;
    esac
    # 3. Learn (before fix -- deep-dives find bugs)
    case "$_SUMMARY" in
        *stud*|*research*|*deep-d*|*analyz*|*discover*|*understand*|*investigat*|*profil*|*documented*) echo "learn"; return ;;
    esac
    # 4. Fix
    case "$_SUMMARY" in
        *fixed*|*bug*|*broke*|*repair*|*patch*|*failure\ rate*) echo "fix"; return ;;
    esac
    # 5. Create
    case "$_SUMMARY" in
        *built*|*creat*|*added*|*design*|*implement*|*develop*|*wrote*|*integrat*|*prototype*) echo "create"; return ;;
    esac
    # Fallback
    if [ "$_STATUS" = "acted" ]; then
        echo "create"
    else
        echo "idle"
    fi
}

# find_mode: given "fix create learn recover idle" counts, print dominant mode
find_mode() {
    _MAX=0; _MODE="idle"
    _I=0
    for _NAME in fix create learn recover idle; do
        _I=$((_I + 1))
        _VAL=$(echo "$1" | cut -d' ' -f"$_I")
        _VAL=${_VAL:-0}
        if [ "$_VAL" -gt "$_MAX" ] 2>/dev/null; then
            _MAX="$_VAL"; _MODE="$_NAME"
        fi
    done
    echo "$_MODE"
}

# Classify all beats, storing categories in temp file (one per line)
CATS=$(mktemp)
tail -"$WINDOW" "$LOG" | while IFS= read -r line; do
    SUMMARY=$(echo "$line" | sed 's/.*"summary":"\([^"]*\)".*/\1/')
    STATUS=$(echo "$line" | sed 's/.*"status":"\([^"]*\)".*/\1/')
    classify_beat "$SUMMARY" "$STATUS"
done > "$CATS"

# safe_count: grep -c that returns 0 on no match without double-output
safe_count() {
    _result=$(grep -c "$1" 2>/dev/null) || true
    echo "${_result:-0}"
}

# Count totals
FIX=$(grep -c '^fix$' "$CATS" || true)
CREATE=$(grep -c '^create$' "$CATS" || true)
LEARN=$(grep -c '^learn$' "$CATS" || true)
RECOVER=$(grep -c '^recover$' "$CATS" || true)
IDLE=$(grep -c '^idle$' "$CATS" || true)

# Ensure numeric (fallback to 0)
FIX=${FIX:-0}; CREATE=${CREATE:-0}; LEARN=${LEARN:-0}
RECOVER=${RECOVER:-0}; IDLE=${IDLE:-0}

# Count recent (last RECENT_SIZE)
TOTAL_CATS=$(wc -l < "$CATS")
RECENT_TMP=$(mktemp)
tail -"$RECENT_SIZE" "$CATS" > "$RECENT_TMP"
R_FIX=$(grep -c '^fix$' "$RECENT_TMP" || true)
R_CREATE=$(grep -c '^create$' "$RECENT_TMP" || true)
R_LEARN=$(grep -c '^learn$' "$RECENT_TMP" || true)
R_RECOVER=$(grep -c '^recover$' "$RECENT_TMP" || true)
R_IDLE=$(grep -c '^idle$' "$RECENT_TMP" || true)
R_FIX=${R_FIX:-0}; R_CREATE=${R_CREATE:-0}; R_LEARN=${R_LEARN:-0}
R_RECOVER=${R_RECOVER:-0}; R_IDLE=${R_IDLE:-0}
rm -f "$RECENT_TMP"

# Count history (older beats)
if [ "$TOTAL_CATS" -gt "$RECENT_SIZE" ]; then
    H_SIZE=$((TOTAL_CATS - RECENT_SIZE))
    HIST_TMP=$(mktemp)
    head -"$H_SIZE" "$CATS" > "$HIST_TMP"
    H_FIX=$(grep -c '^fix$' "$HIST_TMP" || true)
    H_CREATE=$(grep -c '^create$' "$HIST_TMP" || true)
    H_LEARN=$(grep -c '^learn$' "$HIST_TMP" || true)
    H_RECOVER=$(grep -c '^recover$' "$HIST_TMP" || true)
    H_IDLE=$(grep -c '^idle$' "$HIST_TMP" || true)
    H_FIX=${H_FIX:-0}; H_CREATE=${H_CREATE:-0}; H_LEARN=${H_LEARN:-0}
    H_RECOVER=${H_RECOVER:-0}; H_IDLE=${H_IDLE:-0}
    rm -f "$HIST_TMP"
    H_MODE=$(find_mode "$H_FIX $H_CREATE $H_LEARN $H_RECOVER $H_IDLE")
else
    H_MODE=""
fi

rm -f "$CATS"

# Determine modes
ALL_MODE=$(find_mode "$FIX $CREATE $LEARN $RECOVER $IDLE")
R_MODE=$(find_mode "$R_FIX $R_CREATE $R_LEARN $R_RECOVER $R_IDLE")

# Detect trend shift
TREND=""
if [ -n "$H_MODE" ] && [ "$H_MODE" != "$R_MODE" ]; then
    TREND="$H_MODE -> $R_MODE"
fi

# Use RECENT mode for recommendations when there's a trend shift
# (the old pattern is no longer active)
if [ -n "$TREND" ]; then
    EFF_MODE="$R_MODE"
    EFF_FIX="$R_FIX"; EFF_CREATE="$R_CREATE"; EFF_LEARN="$R_LEARN"
    EFF_RECOVER="$R_RECOVER"
else
    EFF_MODE="$ALL_MODE"
    EFF_FIX="$FIX"; EFF_CREATE="$CREATE"; EFF_LEARN="$LEARN"
    EFF_RECOVER="$RECOVER"
fi
EFF_ACTIVE=$((EFF_FIX + EFF_CREATE + EFF_LEARN))

# Generate recommendation based on EFFECTIVE (trend-aware) state
RECOMMENDATION=""
if [ "$WINDOW" -ge 5 ]; then
    if [ "$EFF_RECOVER" -ge 3 ] && [ -z "$TREND" ]; then
        RECOMMENDATION="Frequent recoveries -- address root cause of instability"
    elif [ "$EFF_FIX" -gt 0 ] && [ "$EFF_ACTIVE" -gt 0 ]; then
        FIX_PCT=$((EFF_FIX * 100 / EFF_ACTIVE))
        if [ "$FIX_PCT" -ge 60 ]; then
            RECOMMENDATION="Heavy fix mode -- consider creative or learning work"
        fi
    fi

    if [ -z "$RECOMMENDATION" ] && [ "$EFF_ACTIVE" -ge 2 ]; then
        if [ "$EFF_CREATE" -gt 0 ] && [ "$EFF_LEARN" -eq 0 ]; then
            RECOMMENDATION="All build, no study -- pause to understand before building more"
        elif [ "$EFF_LEARN" -gt 0 ] && [ "$EFF_CREATE" -eq 0 ]; then
            RECOMMENDATION="All study, no build -- apply what you've learned"
        else
            RECOMMENDATION="Balanced work pattern -- keep diversifying"
        fi
    fi

    # Trend-specific recommendations override stale advice
    if [ -n "$TREND" ] && [ -z "$RECOMMENDATION" ]; then
        RECOMMENDATION="Shifted from $H_MODE to $R_MODE -- new pattern emerging"
    fi
fi

# Output
echo "work pattern (last $WINDOW beats):"
echo "  fix=$FIX create=$CREATE learn=$LEARN recover=$RECOVER idle=$IDLE"
if [ -n "$TREND" ]; then
    echo "  mode: $R_MODE (was: $H_MODE)"
    echo "  trend: $TREND"
else
    echo "  mode: $ALL_MODE"
fi
if [ -n "$RECOMMENDATION" ]; then
    echo "  insight: $RECOMMENDATION"
fi
