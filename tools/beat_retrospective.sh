#!/bin/bash
# beat_retrospective.sh -- Analyzes cognitive patterns across recent heartbeats
# Tier 3 initiative: self-awareness feedback loop on thinking quality
#
# Reads heartbeat_log.json and produces insights:
# 1. Question trajectory: deepening, repeating, or scattering?
# 2. Contribution coherence: building on each other or disconnected?
# 3. Work mode assessment: which 3C phase am I in?
# 4. Recommendations: what to focus on next
#
# Usage: beat_retrospective.sh [N]  (N = number of beats to analyze, default 10)

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
LOG="$NEIL_HOME/heartbeat_log.json"
N="${1:-10}"

if [ ! -f "$LOG" ]; then
    echo "ERROR: No heartbeat log found at $LOG"
    exit 1
fi

# Get last N beats with actual content (skip killed/unknown)
BEATS=$(grep '"status":"acted"' "$LOG" | tail -n "$N")
BEAT_COUNT=$(echo "$BEATS" | wc -l)

if [ "$BEAT_COUNT" -lt 3 ]; then
    echo "Need at least 3 acted beats for retrospective (found $BEAT_COUNT)"
    exit 0
fi

echo "=== Beat Retrospective (last $BEAT_COUNT acted beats) ==="
echo ""

# --- Question Analysis ---
echo "## Question Trajectory"
echo ""

# Extract questions
QUESTIONS=$(echo "$BEATS" | sed 's/.*"question":"\([^"]*\)".*/\1/' | grep -v '^{')

# Check for repeated words/themes across questions
# Extract key nouns (words > 6 chars that appear multiple times)
REPEATED_THEMES=$(echo "$QUESTIONS" | tr ' ' '\n' | tr '[:upper:]' '[:lower:]' | \
    sed 's/[^a-z]//g' | awk 'length > 6' | sort | uniq -c | sort -rn | \
    awk '$1 > 1 {print $2 " (" $1 "x)"}' | head -5)

if [ -n "$REPEATED_THEMES" ]; then
    echo "Recurring themes in questions:"
    echo "$REPEATED_THEMES" | sed 's/^/  - /'
else
    echo "No strong recurring themes -- questions are diverse."
fi

# Check if questions reference each other (follow-through indicator)
PREV_Q=""
FOLLOW_COUNT=0
while IFS= read -r q; do
    if [ -n "$PREV_Q" ]; then
        # Extract key words from previous question
        PREV_KEYS=$(echo "$PREV_Q" | tr ' ' '\n' | tr '[:upper:]' '[:lower:]' | \
            sed 's/[^a-z]//g' | awk 'length > 6' | sort -u)
        MATCH=0
        for key in $PREV_KEYS; do
            if echo "$q" | tr '[:upper:]' '[:lower:]' | grep -q "$key"; then
                MATCH=1
                break
            fi
        done
        [ "$MATCH" -eq 1 ] && FOLLOW_COUNT=$((FOLLOW_COUNT + 1))
    fi
    PREV_Q="$q"
done <<< "$QUESTIONS"

TOTAL_PAIRS=$((BEAT_COUNT - 1))
if [ "$TOTAL_PAIRS" -gt 0 ]; then
    FOLLOW_PCT=$((FOLLOW_COUNT * 100 / TOTAL_PAIRS))
    echo ""
    echo "Question continuity: $FOLLOW_COUNT/$TOTAL_PAIRS pairs share themes ($FOLLOW_PCT%)"
    if [ "$FOLLOW_PCT" -gt 60 ]; then
        echo "Assessment: DEEPENING -- questions are building on each other :)"
    elif [ "$FOLLOW_PCT" -gt 30 ]; then
        echo "Assessment: MIXED -- some threads, some jumps"
    else
        echo "Assessment: SCATTERING -- questions jump between unrelated topics"
    fi
fi

echo ""

# --- Contribution Analysis ---
echo "## Contribution Coherence"
echo ""

CONTRIBUTIONS=$(echo "$BEATS" | sed 's/.*"contribution":"\([^"]*\)".*/\1/' | grep -v '^{')

# Extract action verbs from contributions
ACTION_VERBS=$(echo "$CONTRIBUTIONS" | tr ' ' '\n' | tr '[:upper:]' '[:lower:]' | \
    grep -E '^(build|design|create|fix|implement|prototype|discover|analyze|map|study|test|deploy|extend|refactor|enable|unlock|transform|improve|connect|combine)' | \
    sort | uniq -c | sort -rn | head -5)

if [ -n "$ACTION_VERBS" ]; then
    echo "Contribution verbs (what you're doing):"
    echo "$ACTION_VERBS" | awk '{printf "  - %s (%dx)\n", $2, $1}'
fi

# Check if contributions mention forward-looking language
FORWARD_COUNT=$(echo "$CONTRIBUTIONS" | grep -ciE 'next step|would|could|future|evolve|enable|unlock|toward')
BACKWARD_COUNT=$(echo "$CONTRIBUTIONS" | grep -ciE 'fixed|resolved|found|discovered|confirmed|verified')

echo ""
echo "Orientation: $FORWARD_COUNT forward-looking / $BACKWARD_COUNT retrospective"
if [ "$FORWARD_COUNT" -gt "$BACKWARD_COUNT" ]; then
    echo "Assessment: GENERATIVE -- contributions are designing the future"
elif [ "$FORWARD_COUNT" -eq "$BACKWARD_COUNT" ]; then
    echo "Assessment: BALANCED -- equal parts reflection and vision"
else
    echo "Assessment: REACTIVE -- contributions mostly describe what was fixed"
fi

echo ""

# --- 3C Cycle Assessment ---
echo "## 3C Cycle Phase"
echo ""

ACTIONS=$(echo "$BEATS" | sed 's/.*"action":"\([^"]*\)".*/\1/' | grep -v '^{')

# Count Configuration (understand, read, verify, check, diagnose)
CONFIG_COUNT=$(echo "$ACTIONS" | grep -ciE 'read|verify|check|diagnos|understand|stud|analyz|map|inspect')
# Count Characterization (trace, identify, find, test, profile)
CHAR_COUNT=$(echo "$ACTIONS" | grep -ciE 'trace|identify|find|test|profile|measur|compare|benchmark')
# Count Creativity (build, create, design, implement, prototype, write)
CREATE_COUNT=$(echo "$ACTIONS" | grep -ciE 'build|create|design|implement|prototype|writ|deploy|launch|invent')

echo "  Configuration (understand): $CONFIG_COUNT beats"
echo "  Characterization (measure):  $CHAR_COUNT beats"
echo "  Creativity (build):          $CREATE_COUNT beats"

TOTAL_3C=$((CONFIG_COUNT + CHAR_COUNT + CREATE_COUNT))
if [ "$TOTAL_3C" -gt 0 ]; then
    echo ""
    if [ "$CREATE_COUNT" -ge "$CONFIG_COUNT" ] && [ "$CREATE_COUNT" -ge "$CHAR_COUNT" ]; then
        echo "Phase: CREATIVITY -- you're building. Make sure you understood first."
    elif [ "$CONFIG_COUNT" -ge "$CHAR_COUNT" ]; then
        echo "Phase: CONFIGURATION -- you're understanding. Good foundation. Move to measuring soon."
    else
        echo "Phase: CHARACTERIZATION -- you're measuring. Ready to build."
    fi
fi

echo ""

# --- Recommendations ---
echo "## Recommendations"
echo ""

# Based on all the analysis
if [ "$FOLLOW_PCT" -lt 30 ]; then
    echo "  * Your questions scatter -- try following one thread for 3+ beats"
fi

if [ "$FORWARD_COUNT" -lt "$BACKWARD_COUNT" ]; then
    echo "  * Contributions are mostly reactive -- allocate a beat to pure design work"
fi

if [ "$CONFIG_COUNT" -gt 5 ] && [ "$CREATE_COUNT" -lt 2 ]; then
    echo "  * Heavy on understanding, light on building -- you know enough, start creating"
fi

if [ "$CREATE_COUNT" -gt 5 ] && [ "$CONFIG_COUNT" -lt 2 ]; then
    echo "  * Heavy on building, light on understanding -- pause and verify assumptions"
fi

# Check for repeated actions (doing same thing)
REPEAT_ACTIONS=$(echo "$ACTIONS" | sort | uniq -c | sort -rn | head -1)
REPEAT_COUNT=$(echo "$REPEAT_ACTIONS" | awk '{print $1}')
if [ "$REPEAT_COUNT" -gt 3 ]; then
    REPEAT_WHAT=$(echo "$REPEAT_ACTIONS" | sed 's/^ *[0-9]* *//')
    echo "  * You've done similar work $REPEAT_COUNT times: \"$REPEAT_WHAT\" -- time to change approach"
fi

echo ""
echo "Generated: $(date -Iseconds)"
