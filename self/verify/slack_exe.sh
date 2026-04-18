#!/bin/bash
# verify: slack output channel is executable and syntactically valid
# exit 0 = all assertions pass; non-zero = specific failure on stderr
set -u
F="${NEIL_HOME:-$HOME/.neil}/outputs/channels/slack.sh"

if [ ! -f "$F" ]; then
  echo "slack.sh does not exist at $F" >&2
  exit 1
fi

if [ ! -x "$F" ]; then
  MODE=$(stat -c '%a' "$F" 2>/dev/null || echo "???")
  echo "slack.sh is not executable (mode=$MODE); autoprompt.c:782 X_OK guard will silently drop dispatches" >&2
  exit 2
fi

if ! bash -n "$F" 2>/dev/null; then
  echo "slack.sh has shell syntax errors" >&2
  bash -n "$F"
  exit 3
fi

# Smoke: script must accept --help or handle empty invocation without blowing up on parse
# (don't actually send; just ensure entrypoint parses)
head -1 "$F" | grep -q '^#!' || { echo "slack.sh missing shebang" >&2; exit 4; }

exit 0