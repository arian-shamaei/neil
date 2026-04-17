#!/bin/sh
# Archetype: LLM-as-judge verify script.
#
# Use when the criteria are genuinely subjective:
# - "is this draft in Arian's voice?"
# - "does this summary capture the paper's key claims?"
# - "is this code comment clear?"
#
# Spawns a small temp Neil whose only job is to judge pass/fail.
# More expensive than objective checks; use sparingly.

set -e

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
TARGET="${TARGET:-/tmp/draft.txt}"
CRITERIA="${CRITERIA:-is this well-written}"

if [ ! -f "$TARGET" ]; then
    echo "target not found: $TARGET" >&2
    exit 1
fi

CONTENT=$(head -c 4000 "$TARGET")

# Invoke the agent directly with judge prompt
JUDGE_PROMPT="You are a judge. Return exactly one word: PASS or FAIL.

Criteria: $CRITERIA

Content to judge:
---
$CONTENT
---

Your answer (PASS or FAIL only):"

AGENT="$NEIL_HOME/tools/autoPrompter/agent/.venv/bin/python"
SCRIPT="$NEIL_HOME/tools/autoPrompter/agent/neil_agent.py"

VERDICT=$("$AGENT" "$SCRIPT" --system-prompt "You are a judge." -p "$JUDGE_PROMPT" 2>/dev/null | \
          tail -5 | grep -oE "PASS|FAIL" | head -1)

case "$VERDICT" in
    PASS) echo "judge_pass"; exit 0 ;;
    FAIL) echo "judge_fail" >&2; exit 1 ;;
    *)    echo "judge_no_verdict" >&2; exit 2 ;;
esac
