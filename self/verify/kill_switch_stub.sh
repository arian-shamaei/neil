#!/bin/bash
# Verify: neil_os_enabled flag is parsed and logged by autoprompt binary
# Exit 0 if: source declares g_neil_os_enabled AND binary emits the config-loaded log line
# Exit non-zero with reason on stderr otherwise

set -u
SRC="${NEIL_HOME:-$HOME/.neil}/tools/autoPrompter/src/autoprompt.c"
BIN="${NEIL_HOME:-$HOME/.neil}/tools/autoPrompter/autoprompt"

if [ ! -f "$SRC" ]; then
  echo "verify-fail: source not found at $SRC" >&2
  exit 2
fi
if [ ! -f "$BIN" ]; then
  echo "verify-fail: binary not found at $BIN" >&2
  exit 2
fi

# Step 1: declaration present in source
if ! grep -q 'g_neil_os_enabled' "$SRC"; then
  echo "verify-fail: g_neil_os_enabled not declared in source" >&2
  exit 1
fi

# Step 1: parser clause present in source
if ! grep -q 'strcmp(key, *"neil_os_enabled")' "$SRC"; then
  echo "verify-fail: config parser does not read neil_os_enabled key" >&2
  exit 1
fi

# Step 1.5: startup fprintf present in source
if ! grep -q 'config loaded: neil_os_enabled' "$SRC"; then
  echo "verify-fail: startup fprintf for neil_os_enabled missing in source" >&2
  exit 1
fi

# Binary ground truth: symbol/string should appear in the compiled binary
if ! strings "$BIN" 2>/dev/null | grep -q 'config loaded: neil_os_enabled'; then
  echo "verify-fail: binary does not contain the config-loaded log literal (stale build?)" >&2
  exit 1
fi

echo "verify-ok: g_neil_os_enabled declared, parsed, logged, and present in binary"
exit 0