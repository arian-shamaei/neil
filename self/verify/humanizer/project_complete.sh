#!/bin/sh
# project_complete.sh — true only when ALL phases pass their gates and
# the final testbench hits the 4 success criteria.
VERIFY_DIR=$HOME/.neil/self/verify/humanizer
FAIL=0
for s in phase1_detector_calibration phase1_style_extractor phase1_semantic_fidelity phase1_edit_scorer \
         phase2_primitives phase3_orchestration phase4_testbench; do
    SH="$VERIFY_DIR/${s}.sh"
    if [ ! -x "$SH" ]; then
        echo "pending: $s verify script not yet written"; FAIL=1; continue
    fi
    OUT=$(bash "$SH" 2>&1)
    RC=$?
    case $RC in
        0) echo "  [PASS] $s — $OUT" ;;
        1) echo "  [FAIL] $s — $OUT"; FAIL=1 ;;
        2) echo "  [PENDING] $s — $OUT"; FAIL=1 ;;
        *) echo "  [ERR]  $s — rc=$RC — $OUT"; FAIL=1 ;;
    esac
done
exit $FAIL
