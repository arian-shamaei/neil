#!/bin/bash
# Verify observe.sh sed patterns handle space-after-colon in failures.json
# Fast version: checks the source patch + direct sed behavior, no full observe.sh run.
# Exits 0 if patch is in place AND direct sed extracts a real-format failure record.

set -u

OBSERVE=~/.neil/tools/autoPrompter/observe.sh
FAILURES=~/.neil/self/failures.json

# 1. Confirm three patched patterns exist (space-optional ` *"` form)
for key in severity source error; do
    if ! grep -q "\"${key}\": \\*\"" "$OBSERVE"; then
        echo "FAIL: pattern for \"${key}\" not patched in $OBSERVE" >&2
        exit 1
    fi
done

# 2. If failures.json is empty or missing, patch is trivially correct
if [ ! -s "$FAILURES" ]; then
    exit 0
fi

# 3. Use the patched sed patterns directly against a real failure line
LINE=$(grep '"resolution": *"pending"' "$FAILURES" 2>/dev/null | head -1)
if [ -z "$LINE" ]; then
    # No pending failures; patch is trivially correct
    exit 0
fi

SEV=$(echo "$LINE" | sed 's/.*"severity": *"\([^"]*\)".*/\1/')
SRC=$(echo "$LINE" | sed 's/.*"source": *"\([^"]*\)".*/\1/')
ERR=$(echo "$LINE" | sed 's/.*"error": *"\([^"]*\)".*/\1/')

# If any extract equals the input line (substitution failed), patch is wrong
for field in SEV SRC ERR; do
    val=$(eval "echo \$$field")
    if [ "$val" = "$LINE" ]; then
        echo "FAIL: sed for $field did not match; still raw line" >&2
        exit 2
    fi
    if [ -z "$val" ]; then
        echo "FAIL: sed for $field produced empty" >&2
        exit 3
    fi
done

# 4. Severity must be one of the canonical values
case "$SEV" in
    low|medium|high|critical) ;;
    *) echo "FAIL: extracted severity '$SEV' not canonical" >&2; exit 4 ;;
esac

exit 0
