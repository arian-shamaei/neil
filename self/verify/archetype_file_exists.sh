#!/bin/sh
# Archetype: file-exists verify script.
#
# Copy this and edit the OUT path + structure checks for your intention.
# Exit 0 on success, non-zero with reason on stderr.

set -e

OUT="${OUT:-/tmp/REPLACE_ME.md}"

if [ ! -f "$OUT" ]; then
    echo "file not found: $OUT" >&2
    exit 1
fi

# Example structure checks (adapt for the specific intention):
# - non-empty
[ -s "$OUT" ] || { echo "file is empty: $OUT" >&2; exit 2; }

# - contains required marker
# grep -q '^# Summary' "$OUT" || { echo "missing header" >&2; exit 3; }

# - within size bounds
# SZ=$(wc -c < "$OUT")
# [ "$SZ" -gt 100 ] || { echo "too small: $SZ bytes" >&2; exit 4; }
# [ "$SZ" -lt 100000 ] || { echo "too large: $SZ bytes" >&2; exit 5; }

echo "verify_ok"
exit 0
