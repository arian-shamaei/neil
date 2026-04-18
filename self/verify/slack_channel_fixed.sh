#!/bin/sh
# Verify contract for slack_channel_fixed INTEND.
# Passes when slack.sh parses cleanly and contains no \$ escapes.
set -eu
F="$HOME/.neil/outputs/channels/slack.sh"

if [ ! -f "$F" ]; then
  echo "slack.sh missing" >&2; exit 1
fi

if ! bash -n "$F" 2>/dev/null; then
  echo "slack.sh still fails bash -n" >&2; exit 2
fi

if grep -qE '\\\$' "$F"; then
  echo "slack.sh still contains backslash-escaped dollars" >&2; exit 3
fi

# Must reference the token and channel variables unescaped
grep -q '"\$VAULT"' "$F" || { echo "missing unescaped \$VAULT reference" >&2; exit 4; }
grep -q '\$(cat' "$F" || { echo "missing unescaped \$() substitution" >&2; exit 5; }

exit 0