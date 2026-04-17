#!/bin/sh
# Archetype: command-succeeds verify script.
#
# Copy this and change the CMD + success pattern for your intention.
# Useful for: build passes, tests pass, script runs cleanly.

set -e

CMD="${CMD:-true}"  # replace with the real command, e.g.:
# CMD="cd ~/.neil/tools/mempalace && cargo test --quiet"
# CMD="cd ~/.neil/tools/autoPrompter && make"
# CMD="bash ~/.neil/self/self_check.sh"

OUT=$(mktemp)
trap "rm -f $OUT" EXIT

if ! sh -c "$CMD" > "$OUT" 2>&1; then
    echo "command failed (exit $?):" >&2
    tail -20 "$OUT" >&2
    exit 1
fi

# Optional: check for a success marker in output
# grep -q "test result: ok" "$OUT" || { echo "tests did not run"; exit 2; }

echo "verify_ok"
exit 0
