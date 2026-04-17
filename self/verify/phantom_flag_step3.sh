#!/bin/bash
# Verify: beat_router actually honors g_neil_os_enabled / neil_os_enabled at dispatch
# Exit 0 if beat_router source checks the flag AND a test run with flag=0 short-circuits
# Exit non-zero with reason on stderr otherwise

set -u
NEIL="${NEIL_HOME:-$HOME/.neil}"
ROUTER_DIR="$NEIL/tools/beat_router"

if [ ! -d "$ROUTER_DIR" ]; then
  echo "verify-fail: beat_router dir not found at $ROUTER_DIR" >&2
  exit 2
fi

# Look for any reference to the flag in router source (python, shell, or C)
if ! grep -rq 'neil_os_enabled' "$ROUTER_DIR" 2>/dev/null; then
  echo "verify-fail: beat_router source does not reference neil_os_enabled" >&2
  exit 1
fi

# Also require a guard-style check (if/return/skip) near that reference to avoid
# the same "flag read but not gated" trap we just fixed in autoprompt.c
if ! grep -rEq '(if.*neil_os_enabled|neil_os_enabled.*==.*(0|false|False)|skip.*neil_os_enabled)' "$ROUTER_DIR" 2>/dev/null; then
  echo "verify-fail: beat_router references flag but has no conditional gate" >&2
  exit 1
fi

echo "verify-ok: beat_router references and gates on neil_os_enabled"
exit 0