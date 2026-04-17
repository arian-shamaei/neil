#!/bin/sh
# Archetype: state-change verify script.
#
# Use when the intention's success is a change in live system state:
# - a new zettel note exists in the right wing/room
# - a config file has the expected key=value
# - a service is now running with the expected settings
#
# Copy and adapt. Multiple independent checks possible.

set -e

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"

# EXAMPLE 1: a new memory note landed in the right wing/room
# NOTE_PATTERN="*tilde*"
# MATCHING=$(ls -t "$NEIL_HOME/memory/palace/notes/"*.md 2>/dev/null | \
#            xargs grep -l "tilde" 2>/dev/null | head -1)
# [ -n "$MATCHING" ] || { echo "no note matching tilde found" >&2; exit 1; }
# grep -q "wing: openclaw" "$MATCHING" || { echo "wrong wing" >&2; exit 2; }
# grep -q "room: autoprompt" "$MATCHING" || { echo "wrong room" >&2; exit 3; }

# EXAMPLE 2: config.toml has expected value
# grep -q "^max_react_turns = 6" "$NEIL_HOME/config.toml" || \
#     { echo "config not updated" >&2; exit 1; }

# EXAMPLE 3: binary was rebuilt recently (within last 5 min)
# BIN="$NEIL_HOME/tools/autoPrompter/autoprompt"
# [ -f "$BIN" ] || { echo "binary missing" >&2; exit 1; }
# AGE=$(( $(date +%s) - $(stat -c %Y "$BIN") ))
# [ "$AGE" -lt 300 ] || { echo "binary not recently rebuilt (${AGE}s old)" >&2; exit 2; }

echo "verify_ok"
exit 0
