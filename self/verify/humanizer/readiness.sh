#!/bin/bash
# Readiness audit: is the humanizer project ready for Peer-A dispatch?
# Ready = all Phase 1 infrastructure verify scripts pass (detector bench,
# style extractor, semantic fidelity, edit scorer). Without Phase 1,
# Peer-A has no falsifiable metrics to optimize against.

set -u
VERIFY_DIR="$(dirname "$0")"
STATE_DIR="$NEIL_HOME/projects/humanizer/state"
mkdir -p "$STATE_DIR"

declare -A results
overall=0

for phase in detector_calibration style_extractor semantic_fidelity edit_scorer; do
    script="$VERIFY_DIR/phase1_${phase}.sh"
    if [ ! -x "$script" ]; then
        results[$phase]="missing"
        overall=1
        continue
    fi
    if "$script" >/dev/null 2>&1; then
        results[$phase]="pass"
    else
        results[$phase]="fail"
        overall=1
    fi
done

# Emit readiness.json (machine-readable state)
{
    echo "{"
    echo "  \"timestamp\": \"$(date -Iseconds)\","
    echo "  \"ready\": $([ $overall -eq 0 ] && echo true || echo false),"
    echo "  \"phase1\": {"
    echo "    \"detector_calibration\": \"${results[detector_calibration]}\","
    echo "    \"style_extractor\": \"${results[style_extractor]}\","
    echo "    \"semantic_fidelity\": \"${results[semantic_fidelity]}\","
    echo "    \"edit_scorer\": \"${results[edit_scorer]}\""
    echo "  },"
    echo "  \"dispatch_gate\": \"Phase 1 infra must pass before Peer-A begins Phase 2 primitives\""
    echo "}"
} > "$STATE_DIR/readiness.json"

# Human-readable stderr for operator
echo "Readiness audit results:" >&2
for phase in detector_calibration style_extractor semantic_fidelity edit_scorer; do
    echo "  phase1_${phase}: ${results[$phase]}" >&2
done
echo "Overall: $([ $overall -eq 0 ] && echo READY || echo NOT_READY)" >&2

exit $overall