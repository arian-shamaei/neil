#!/bin/bash
# memory_decay.sh -- Spaced repetition for autonomous agent memory
#
# Biological memory decays without reinforcement. This script applies
# the same principle to zettel notes:
#
# - Each note has a "strength" score (0.0 to 1.0)
# - Strength decays exponentially over time (half-life: 5 days)
# - Accessing a note reinforces it (strength boost)
# - Notes below threshold (0.3) are "decaying" and surface for review
# - Notes that get reviewed and reinforced survive; irrelevant ones fade
#
# This creates natural memory prioritization: frequently useful knowledge
# stays strong, stale knowledge surfaces for consolidation or pruning.
#
# Usage:
#   memory_decay.sh score          -- score all notes, show decaying ones
#   memory_decay.sh reinforce <id> -- log an access, boost strength
#   memory_decay.sh report         -- full report with all scores
#   memory_decay.sh decaying       -- list only decaying notes (for observe.sh)

set -euo pipefail

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
PALACE="${ZETTEL_HOME:-$NEIL_HOME/memory/palace}"
NOTES_DIR="$PALACE/notes"
ACCESS_LOG="$PALACE/access_log.json"
DECAY_HALF_LIFE_DAYS=5
DECAY_THRESHOLD=0.3
REINFORCE_BOOST=0.4  # Each access adds this much strength (capped at 1.0)

# Initialize access log if missing
if [ ! -f "$ACCESS_LOG" ]; then
    echo '{}' > "$ACCESS_LOG"
fi

now_epoch() {
    date +%s
}

# Parse ISO-ish timestamp from note frontmatter
parse_timestamp() {
    local ts="$1"
    # Handle formats: 2026-04-10T15:40:36 or 2026-04-10
    date -d "$(echo "$ts" | tr 'T' ' ')" +%s 2>/dev/null || echo "0"
}

# Exponential decay: strength = initial * 2^(-days_elapsed / half_life)
# With reinforcement: each access resets the decay clock partially
calc_strength() {
    local note_id="$1"
    local created_epoch="$2"
    local now="$3"

    # Get last access time and access count from log
    local last_access count
    last_access=$(python3 -c "
import json, sys
log = json.load(open('$ACCESS_LOG'))
entry = log.get('$note_id', {})
print(entry.get('last_access', 0))
" 2>/dev/null || echo "0")

    count=$(python3 -c "
import json, sys
log = json.load(open('$ACCESS_LOG'))
entry = log.get('$note_id', {})
print(entry.get('count', 0))
" 2>/dev/null || echo "0")

    # Decay from most recent event (creation or last access)
    local reference_epoch=$created_epoch
    if [ "$last_access" -gt "$reference_epoch" ] 2>/dev/null; then
        reference_epoch=$last_access
    fi

    # Calculate decay
    python3 -c "
import math
now = $now
ref = $reference_epoch
count = $count
half_life = $DECAY_HALF_LIFE_DAYS * 86400  # convert to seconds
elapsed = now - ref
if elapsed < 0:
    elapsed = 0

# Base decay from time since last access/creation
base_strength = 2 ** (-elapsed / half_life)

# Frequency bonus: each past access adds a small permanent floor
# (diminishing returns via log)
freq_floor = min(0.2, 0.05 * math.log(1 + count)) if count > 0 else 0

# Final strength: decay + frequency floor, capped at 1.0
strength = min(1.0, base_strength + freq_floor)
print(f'{strength:.3f}')
"
}

cmd_reinforce() {
    local note_id="$1"
    local now
    now=$(now_epoch)

    python3 -c "
import json
log = json.load(open('$ACCESS_LOG'))
entry = log.get('$note_id', {'count': 0, 'last_access': 0, 'history': []})
entry['count'] = entry['count'] + 1
entry['last_access'] = $now
# Keep last 10 access timestamps
entry['history'] = (entry.get('history', []) + [$now])[-10:]
log['$note_id'] = entry
json.dump(log, open('$ACCESS_LOG', 'w'), indent=2)
print(f'Reinforced {\"$note_id\"}: access #{entry[\"count\"]}')
"
}

cmd_score() {
    local now
    now=$(now_epoch)
    local decaying=0
    local total=0

    echo "=== Memory Strength Report ==="
    echo ""

    for note_file in "$NOTES_DIR"/*.md; do
        [ -f "$note_file" ] || continue
        local note_id
        note_id=$(basename "$note_file" .md)
        total=$((total + 1))

        # Extract created timestamp from frontmatter
        local created_ts
        created_ts=$(grep '^created:' "$note_file" | head -1 | sed 's/created: *//')
        local created_epoch
        created_epoch=$(parse_timestamp "$created_ts")

        # Extract wing/room for context
        local wing room
        wing=$(grep '^wing:' "$note_file" | head -1 | sed 's/wing: *//')
        room=$(grep '^room:' "$note_file" | head -1 | sed 's/room: *//')

        # First content line (skip frontmatter)
        local summary
        summary=$(awk '/^---$/{n++; next} n>=2{print; exit}' "$note_file" | head -c 60)

        local strength
        strength=$(calc_strength "$note_id" "$created_epoch" "$now")

        local status="alive"
        if python3 -c "exit(0 if $strength < $DECAY_THRESHOLD else 1)"; then
            status="DECAYING"
            decaying=$((decaying + 1))
        fi

        if [ "$1" = "all" ] || [ "$status" = "DECAYING" ]; then
            printf "  [%s] %.3f %s/%s -- %s\n" "$status" "$strength" "$wing" "$room" "$summary"
        fi
    done

    echo ""
    echo "Total: $total notes, $decaying decaying (below $DECAY_THRESHOLD threshold)"
}

cmd_decaying() {
    # Compact output for observe.sh integration
    local now
    now=$(now_epoch)
    local decaying=0
    local output=""

    for note_file in "$NOTES_DIR"/*.md; do
        [ -f "$note_file" ] || continue
        local note_id
        note_id=$(basename "$note_file" .md)

        local created_ts
        created_ts=$(grep '^created:' "$note_file" | head -1 | sed 's/created: *//')
        local created_epoch
        created_epoch=$(parse_timestamp "$created_ts")

        local strength
        strength=$(calc_strength "$note_id" "$created_epoch" "$now")

        if python3 -c "exit(0 if $strength < $DECAY_THRESHOLD else 1)"; then
            local wing room summary
            wing=$(grep '^wing:' "$note_file" | head -1 | sed 's/wing: *//')
            room=$(grep '^room:' "$note_file" | head -1 | sed 's/room: *//')
            summary=$(awk '/^---$/{n++; next} n>=2{print; exit}' "$note_file" | head -c 50)
            output+="  $note_id ($wing/$room) strength=$strength -- $summary"$'\n'
            decaying=$((decaying + 1))
        fi
    done

    if [ "$decaying" -gt 0 ]; then
        echo "decaying: $decaying notes below threshold"
        echo "$output"
    else
        echo "decaying: 0 (all memories strong)"
    fi
}

# Main dispatch
case "${1:-score}" in
    score)
        cmd_score "decaying"
        ;;
    report)
        cmd_score "all"
        ;;
    reinforce)
        if [ -z "${2:-}" ]; then
            echo "usage: memory_decay.sh reinforce <note_id>"
            exit 1
        fi
        cmd_reinforce "$2"
        ;;
    decaying)
        cmd_decaying
        ;;
    *)
        echo "usage: memory_decay.sh {score|report|reinforce|decaying}"
        exit 1
        ;;
esac
