#!/bin/bash
# Verify: email.sh appends an ISO-timestamped line to dispatch.log when invoked.
# Delegates to outputs_audit_log.sh which already checks the shared substrate,
# then adds an email-specific syntax check.
set -e

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
EMAIL_SH="$NEIL_HOME/outputs/channels/email.sh"
LOG="$NEIL_HOME/outputs/dispatch.log"

# 1. email.sh must exist and parse
[ -f "$EMAIL_SH" ] || { echo "email.sh missing: $EMAIL_SH" >&2; exit 1; }
bash -n "$EMAIL_SH" || { echo "email.sh syntax error" >&2; exit 2; }

# 2. email.sh must reference dispatch.log (tee-append landed)
grep -q 'dispatch.log' "$EMAIL_SH" || { echo "email.sh missing dispatch.log append" >&2; exit 3; }

# 3. Delegate shared-substrate checks to the outputs audit verify
SHARED="$NEIL_HOME/self/verify/outputs_audit_log.sh"
if [ -x "$SHARED" ]; then
    "$SHARED" || { echo "shared outputs_audit_log.sh failed" >&2; exit 4; }
fi

echo "dispatch_audit_email.sh: OK"
exit 0