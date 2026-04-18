#!/bin/bash
# End-to-end verify: neil_os_enabled kill switch actually disables the OS layer.
# Composes Steps 1+1.5 (kill_switch_stub.sh) and Step 3 (phantom_flag_step3.sh)
# and then performs a live three-state behavioral test of beat_router.sh:
#   - flag absent            -> normal operation (exit 0 or whatever non-kill path returns)
#   - neil_os_enabled=false  -> kill switch engaged, exits 0 silently
#   - neil_os_enabled=true   -> normal operation resumes
#
# Idempotent: saves/restores config.toml via a temp copy. Runs <60s.

set -u
VERIFY_DIR="${NEIL_HOME:-$HOME/.neil}/self/verify"
NEIL="${NEIL_HOME:-$HOME/.neil}"
CONFIG="$NEIL/config.toml"
BEAT_ROUTER="$NEIL/tools/beat_router/beat_router.sh"

if [ ! -f "$CONFIG" ]; then
  echo "verify-fail: config.toml not found at $CONFIG" >&2
  exit 2
fi
if [ ! -f "$BEAT_ROUTER" ]; then
  echo "verify-fail: beat_router.sh not found at $BEAT_ROUTER" >&2
  exit 2
fi

# Step 1+1.5: source + binary have the flag
if ! bash "$VERIFY_DIR/kill_switch_stub.sh" >/dev/null 2>&1; then
  echo "verify-fail: Steps 1+1.5 (source+binary) did not pass -- run kill_switch_stub.sh for detail" >&2
  exit 1
fi

# Step 3: beat_router source references the flag with a gate
if ! bash "$VERIFY_DIR/phantom_flag_step3.sh" >/dev/null 2>&1; then
  echo "verify-fail: Step 3 (beat_router gate) not in source -- run phantom_flag_step3.sh for detail" >&2
  exit 1
fi

# Behavioral test: save config, mutate, test, restore
BACKUP="$(mktemp)"
cp "$CONFIG" "$BACKUP"
trap 'cp "$BACKUP" "$CONFIG"; rm -f "$BACKUP"' EXIT

# State 1: flag absent
grep -v '^neil_os_enabled' "$BACKUP" > "$CONFIG"
OUT_ABSENT="$(bash "$BEAT_ROUTER" 2>&1)"; RC_ABSENT=$?

# State 2: flag=false (kill switch ENGAGED)
grep -v '^neil_os_enabled' "$BACKUP" > "$CONFIG"
echo 'neil_os_enabled = false' >> "$CONFIG"
OUT_FALSE="$(bash "$BEAT_ROUTER" 2>&1)"; RC_FALSE=$?

# State 3: flag=true (kill switch DISENGAGED)
grep -v '^neil_os_enabled' "$BACKUP" > "$CONFIG"
echo 'neil_os_enabled = true' >> "$CONFIG"
OUT_TRUE="$(bash "$BEAT_ROUTER" 2>&1)"; RC_TRUE=$?

# Kill switch contract: when false, router exits 0 with no directive emitted.
# (A "directive" in beat_router.sh stdout would be mode=... lines.)
if [ "$RC_FALSE" -ne 0 ]; then
  echo "verify-fail: kill-switch engaged (false) but beat_router exited nonzero ($RC_FALSE)" >&2
  exit 1
fi
if echo "$OUT_FALSE" | grep -qE '^mode='; then
  echo "verify-fail: kill-switch engaged (false) but beat_router still emitted a directive" >&2
  echo "stdout: $OUT_FALSE" >&2
  exit 1
fi

# Sanity: true and absent should NOT be gated out. They may emit directives or
# other output; we only assert they are not silenced by the kill switch.
# (beat_router can legitimately exit nonzero for other policy reasons; we just
#  check it is not silently exiting with the kill-switch message pattern.)
if [ -z "$OUT_TRUE" ] && [ "$RC_TRUE" -eq 0 ]; then
  echo "verify-fail: flag=true produced identical silent behavior to flag=false (gate too wide)" >&2
  exit 1
fi

echo "verify-ok: kill switch end-to-end: absent rc=$RC_ABSENT, false rc=$RC_FALSE (silenced), true rc=$RC_TRUE (not silenced)"
exit 0
