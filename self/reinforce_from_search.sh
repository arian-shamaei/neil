#!/bin/bash
# reinforce_from_search.sh -- Parse mempalace search output and reinforce found notes
#
# Reads stdin (mempalace search results) and extracts note IDs from
# "Source: <id>.md" lines, then reinforces each one via memory_decay.sh.
#
# Usage:
#   mempalace search "query" | reinforce_from_search.sh
#   # or pass results as argument:
#   reinforce_from_search.sh "$(cat search_output.txt)"
#
# Also processes heartbeat history output for note IDs referenced in
# RELEVANT MEMORIES sections of previous prompts.

set -euo pipefail

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
DECAY_SCRIPT="$NEIL_HOME/self/memory_decay.sh"

# Get input from stdin or argument
if [ -n "${1:-}" ]; then
    INPUT="$1"
else
    INPUT=$(cat)
fi

# Extract note IDs from "Source: <id>.md" lines
echo "$INPUT" | grep -oP 'Source:\s+\K[0-9T_a-f]+(?=\.md)' | sort -u | while read -r note_id; do
    "$DECAY_SCRIPT" reinforce "$note_id" 2>/dev/null || true
done
