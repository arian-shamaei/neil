#!/bin/bash
# Verify that spawn_vm.sh contains the substrate-verify gate after kickoff_peer.
# Exits 0 if gate is present and syntactically valid; non-zero otherwise.
#
# Gate requirements (per 2026-04-21T14-15-57 beat):
#   1. spawn_vm.sh must exist and pass bash -n
#   2. Must contain an lxc exec ... test -d ... essence check
#   3. Must contain an lxc exec ... test -f ... identity.md check
#   4. Must set registry status to "substrate-missing" on failure
#   5. Gate must fire BEFORE the "ready" status is emitted

set -u

SPAWN_VM="${NEIL_HOME:-$HOME/.neil}/tools/spawn_vm/spawn_vm.sh"

if [ ! -f "$SPAWN_VM" ]; then
    echo "spawn_vm.sh not found at $SPAWN_VM" >&2
    exit 1
fi

# 1. Syntax check
if ! bash -n "$SPAWN_VM" 2>/dev/null; then
    echo "spawn_vm.sh fails bash -n syntax check" >&2
    exit 2
fi

# 2. essence directory check present
if ! grep -qE 'lxc[[:space:]]+exec.*test[[:space:]]+-d.*essence' "$SPAWN_VM"; then
    echo "gate missing: no 'lxc exec ... test -d ... essence' pattern" >&2
    exit 3
fi

# 3. identity.md file check present
if ! grep -qE 'test[[:space:]]+-f.*identity\.md' "$SPAWN_VM"; then
    echo "gate missing: no 'test -f ... identity.md' pattern" >&2
    exit 4
fi

# 4. substrate-missing status transition present
if ! grep -q 'substrate-missing' "$SPAWN_VM"; then
    echo "gate missing: no 'substrate-missing' status string" >&2
    exit 5
fi

# 5. Ordering: substrate-missing must appear BEFORE the final ready emission
# Find line numbers
missing_line=$(grep -n 'substrate-missing' "$SPAWN_VM" | head -1 | cut -d: -f1)
ready_line=$(grep -n 'registry_set.*ready\|status.*ready' "$SPAWN_VM" | tail -1 | cut -d: -f1)

if [ -z "$missing_line" ]; then
    echo "could not locate substrate-missing line" >&2
    exit 6
fi

if [ -n "$ready_line" ] && [ "$missing_line" -gt "$ready_line" ]; then
    echo "gate ordering wrong: substrate-missing appears AFTER ready emission" >&2
    exit 7
fi

echo "spawn_vm substrate-verify gate present and valid"
exit 0