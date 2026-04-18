#!/bin/bash
set -e
NH="${NEIL_HOME:-$HOME/.neil}"
REG="$NH/services/registry/spawn_vm.md"
[ -f "$REG" ] || { echo "missing $REG" >&2; exit 1; }
for act in create destroy list status; do
    grep -q "  - $act" "$REG" || { echo "registry missing action: $act" >&2; exit 1; }
done
DISP="$NH/services/handlers/spawn_vm.sh"
[ -f "$DISP" ] || { echo "missing $DISP" >&2; exit 1; }
chmod +x "$DISP" 2>/dev/null || true
for act in create destroy list status; do
    grep -qE "^[[:space:]]*$act\)" "$DISP" || { echo "dispatcher missing case: $act" >&2; exit 1; }
done
CLUSTER="$NH/cluster"
[ -d "$CLUSTER" ] || { echo "missing $CLUSTER" >&2; exit 1; }
[ -f "$CLUSTER/schema.json" ] || { echo "missing schema.json" >&2; exit 1; }
[ -f "$CLUSTER/instances.json" ] || { echo "missing instances.json" >&2; exit 1; }
python3 -c "import json; json.load(open('$CLUSTER/schema.json'))" >/dev/null 2>&1 || { echo "schema.json invalid" >&2; exit 1; }
python3 -c "import json; json.load(open('$CLUSTER/instances.json'))" >/dev/null 2>&1 || { echo "instances.json invalid" >&2; exit 1; }
OUT=$(bash "$DISP" create name=verify-probe dry_run=1 2>&1)
echo "$OUT" | python3 -c "
import json, sys
rec = json.loads(sys.stdin.read())
assert rec.get('id','').startswith('dry-')
assert rec.get('name') == 'verify-probe'
assert rec.get('status') == 'dry-provisioned'
" >/dev/null 2>&1 || { echo "dry-run probe failed: $OUT" >&2; exit 1; }
bash "$DISP" destroy name=verify-probe >/dev/null 2>&1 || true
echo "spawn_vm skeleton verified"
exit 0