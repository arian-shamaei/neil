#!/bin/sh
# verify: outputs audit log -- email.sh and slack.sh both append to dispatch.log
# exit 0 = all criteria met, non-zero = reason on stderr
set -u

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
SLACK="$NEIL_HOME/outputs/channels/slack.sh"
EMAIL="$NEIL_HOME/outputs/channels/email.sh"
LOG="$NEIL_HOME/outputs/dispatch.log"

# 1. Both channel scripts must exist and parse cleanly
for f in "$SLACK" "$EMAIL"; do
    if [ ! -f "$f" ]; then
        echo "missing: $f" >&2
        exit 1
    fi
    if ! bash -n "$f" 2>/dev/null; then
        echo "syntax error: $f" >&2
        exit 2
    fi
done

# 2. Both scripts must reference dispatch.log
grep -q 'dispatch\.log' "$SLACK" || { echo "slack.sh missing dispatch.log append" >&2; exit 3; }
grep -q 'dispatch\.log' "$EMAIL" || { echo "email.sh missing dispatch.log append" >&2; exit 4; }

# 3. dispatch.log must exist (created by first append or by hand)
if [ ! -f "$LOG" ]; then
    echo "dispatch.log not found at $LOG" >&2
    exit 5
fi

# 4. Format check: each line should have ISO timestamp + tab-separated fields
# Tolerate empty file (nothing dispatched yet) but reject malformed entries.
if [ -s "$LOG" ]; then
    bad=$(grep -cv '^[0-9]\{4\}-[0-9]\{2\}-[0-9]\{2\}T[0-9]\{2\}:[0-9]\{2\}:[0-9]\{2\}' "$LOG" || true)
    if [ "$bad" -gt 0 ]; then
        echo "dispatch.log has $bad malformed lines" >&2
        exit 6
    fi
fi

echo "outputs audit log verified: slack.sh + email.sh both write to $LOG"
exit 0
