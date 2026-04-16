#!/bin/sh
# question_tracker.sh -- Track heartbeat questions and surface unanswered ones
#
# Every heartbeat asks a QUESTION. This script closes the loop:
#   1. Extracts all questions from heartbeat_log.json
#   2. Checks if subsequent beats addressed them (by keyword overlap)
#   3. Surfaces unanswered questions as initiative candidates
#
# Output modes:
#   (default)  - Compact summary for observe.sh integration
#   --detail   - Full list of questions with status
#   --open     - Only unanswered questions (for initiative selection)
#
# A question is considered "addressed" if any subsequent beat's action
# or summary contains 2+ significant keywords from the question.
#
# Usage: question_tracker.sh [--detail|--open]

LOG="$HOME/.neil/heartbeat_log.json"
MODE="${1:-summary}"

if [ ! -f "$LOG" ] || [ ! -s "$LOG" ]; then
    echo "questions: unknown (no data)"
    exit 0
fi

TOTAL=$(wc -l < "$LOG")
if [ "$TOTAL" -lt 2 ]; then
    echo "questions: insufficient data (need 2+ beats)"
    exit 0
fi

# Stop words to filter out when extracting keywords
STOPWORDS="the a an is are was were be been being have has had do does did will would shall should may might can could of in to for on with at by from as into through during before after above below between out off over under"

# extract_keywords: pull significant words (4+ chars, not stopwords) from text
extract_keywords() {
    echo "$1" | tr 'A-Z' 'a-z' | tr -cs 'a-z0-9' ' ' | tr ' ' '\n' | while read -r word; do
        # Skip short words
        if [ ${#word} -lt 4 ]; then
            continue
        fi
        # Skip stop words
        _skip=0
        for sw in $STOPWORDS; do
            if [ "$word" = "$sw" ]; then
                _skip=1
                break
            fi
        done
        if [ "$_skip" -eq 0 ]; then
            echo "$word"
        fi
    done | sort -u
}

# Build arrays via temp files
Q_FILE=$(mktemp)    # questions with timestamps
A_FILE=$(mktemp)    # all subsequent actions/summaries concatenated per-beat
RESULT=$(mktemp)    # final output

# Extract all beats with questions
LINE_NUM=0
tail -"$TOTAL" "$LOG" | while IFS= read -r line; do
    LINE_NUM=$((LINE_NUM + 1))
    QUESTION=$(echo "$line" | sed -n 's/.*"question":"\([^"]*\)".*/\1/p')
    TIMESTAMP=$(echo "$line" | sed -n 's/.*"timestamp":"\([^"]*\)".*/\1/p')
    if [ -n "$QUESTION" ]; then
        echo "${LINE_NUM}|${TIMESTAMP}|${QUESTION}" >> "$Q_FILE"
    fi
done

# Extract all actions/summaries for matching
LINE_NUM=0
tail -"$TOTAL" "$LOG" | while IFS= read -r line; do
    LINE_NUM=$((LINE_NUM + 1))
    ACTION=$(echo "$line" | sed -n 's/.*"action":"\([^"]*\)".*/\1/p')
    SUMMARY=$(echo "$line" | sed -n 's/.*"summary":"\([^"]*\)".*/\1/p')
    echo "${LINE_NUM}|${ACTION} ${SUMMARY}" >> "$A_FILE"
done

# Check each question against subsequent beats
TOTAL_Q=0
ANSWERED=0
OPEN=0

while IFS='|' read -r QLINE QTIME QTEXT; do
    TOTAL_Q=$((TOTAL_Q + 1))

    # Extract keywords from question
    KEYWORDS=$(extract_keywords "$QTEXT")
    KEYWORD_COUNT=$(echo "$KEYWORDS" | wc -w)

    if [ "$KEYWORD_COUNT" -lt 2 ]; then
        # Too few keywords to match reliably -- mark as open
        OPEN=$((OPEN + 1))
        echo "open|${QTIME}|${QTEXT}" >> "$RESULT"
        continue
    fi

    # Check subsequent beats for keyword overlap
    MATCHED=0
    MATCH_BEAT=""
    while IFS='|' read -r ALINE ATEXT; do
        # Only check beats AFTER this question
        if [ "$ALINE" -le "$QLINE" ]; then
            continue
        fi

        ATEXT_LOWER=$(echo "$ATEXT" | tr 'A-Z' 'a-z')
        HIT_COUNT=0

        for kw in $KEYWORDS; do
            case "$ATEXT_LOWER" in
                *"$kw"*) HIT_COUNT=$((HIT_COUNT + 1)) ;;
            esac
        done

        # Threshold: 2+ keyword hits = addressed
        if [ "$HIT_COUNT" -ge 2 ]; then
            MATCHED=1
            MATCH_BEAT=$(echo "$ATEXT" | head -c 60)
            break
        fi
    done < "$A_FILE"

    if [ "$MATCHED" -eq 1 ]; then
        ANSWERED=$((ANSWERED + 1))
        echo "answered|${QTIME}|${QTEXT}" >> "$RESULT"
    else
        OPEN=$((OPEN + 1))
        echo "open|${QTIME}|${QTEXT}" >> "$RESULT"
    fi
done < "$Q_FILE"

# Output based on mode
case "$MODE" in
    --detail)
        echo "=== Question Tracker ($TOTAL_Q questions) ==="
        echo "  answered: $ANSWERED  open: $OPEN"
        echo ""
        if [ -f "$RESULT" ]; then
            while IFS='|' read -r STATUS QTIME QTEXT; do
                if [ "$STATUS" = "answered" ]; then
                    MARK="[x]"
                else
                    MARK="[ ]"
                fi
                echo "  $MARK ($QTIME) $QTEXT"
            done < "$RESULT"
        fi
        ;;
    --open)
        if [ -f "$RESULT" ]; then
            grep '^open|' "$RESULT" | while IFS='|' read -r STATUS QTIME QTEXT; do
                echo "$QTEXT"
            done
        fi
        ;;
    *)
        # Compact summary for observe.sh
        if [ "$TOTAL_Q" -eq 0 ]; then
            echo "questions: none tracked"
        else
            RATE=$((ANSWERED * 100 / TOTAL_Q))
            echo "questions: ${ANSWERED}/${TOTAL_Q} answered (${RATE}% follow-through)"
            if [ "$OPEN" -gt 0 ]; then
                # Show most recent unanswered question as initiative hint
                LATEST=$(grep '^open|' "$RESULT" | tail -1 | cut -d'|' -f3)
                if [ -n "$LATEST" ]; then
                    # Truncate to 100 chars
                    SHORT=$(echo "$LATEST" | head -c 100)
                    echo "  latest open: $SHORT"
                fi
            fi
        fi
        ;;
esac

# Cleanup
rm -f "$Q_FILE" "$A_FILE" "$RESULT"
