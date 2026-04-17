#!/bin/bash
# Verify: every config.toml key is read somewhere in source (no phantom flags)
# Exit 0 if every non-comment key appears at least once in autoPrompter or os/ sources
# Exit non-zero listing the phantom keys otherwise

set -u
NEIL="${NEIL_HOME:-$HOME/.neil}"
CFG="$NEIL/config.toml"

if [ ! -f "$CFG" ]; then
  echo "verify-fail: config.toml not found at $CFG" >&2
  exit 2
fi

# Extract bare keys from TOML (ignore section headers, comments, blanks)
KEYS=$(awk -F'=' '
  /^[[:space:]]*#/ {next}
  /^[[:space:]]*\[/ {next}
  /^[[:space:]]*$/ {next}
  NF>=2 {
    k=$1
    gsub(/^[[:space:]]+|[[:space:]]+$/, "", k)
    if (k != "") print k
  }
' "$CFG" | sort -u)

PHANTOMS=""
for k in $KEYS; do
  # Search tool source trees for either the bare key or strcmp(key, "name")
  if ! grep -rqE "(\"$k\"|'$k')" \
        "$NEIL/tools/autoPrompter/src" \
        "$NEIL/tools/beat_router" \
        "$NEIL/os" 2>/dev/null; then
    PHANTOMS="$PHANTOMS $k"
  fi
done

if [ -n "$PHANTOMS" ]; then
  echo "verify-fail: phantom config keys (declared in config.toml but never read by any source):$PHANTOMS" >&2
  exit 1
fi

echo "verify-ok: all config.toml keys are referenced in source"
exit 0