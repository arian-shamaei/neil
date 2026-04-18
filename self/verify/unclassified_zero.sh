#!/bin/bash
# Verifies that zero unclassified notes exist in the palace.
# A note is "unclassified" if it lacks a `wing:` frontmatter key.
# Exits 0 if count == 0, non-zero otherwise (with count on stderr).

set -euo pipefail

NOTES_DIR="${ZETTEL_HOME:-$HOME/.neil/memory/palace}/notes"

if [ ! -d "$NOTES_DIR" ]; then
  echo "notes dir missing: $NOTES_DIR" >&2
  exit 2
fi

# Count notes missing a wing: frontmatter line
count=$(grep -rLE '^wing:' "$NOTES_DIR" 2>/dev/null | wc -l)

if [ "$count" -eq 0 ]; then
  echo "unclassified=0 OK"
  exit 0
fi

echo "unclassified=$count notes missing wing: frontmatter" >&2
grep -rLE '^wing:' "$NOTES_DIR" 2>/dev/null | head -20 >&2
exit 1