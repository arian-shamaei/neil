#!/bin/sh
# Phase 1.1 verify — detector bench calibration
# Passes when: ≥4 detectors wired, each with AUC ≥ 0.85 on a 500-sample
# held-out set of (labeled) AI vs human text.
#
# Reads: projects/humanizer/state/metrics.jsonl (last entry must have phase=1.1)
# Exit: 0 = pass, 1 = fail, 2 = pending (not yet attempted)

STATE=$HOME/.neil/projects/humanizer/state/metrics.jsonl
[ -f "$STATE" ] || { echo "pending: no metrics yet"; exit 2; }

python3 - <<'PY'
import json, sys, pathlib
p = pathlib.Path.home() / ".neil/projects/humanizer/state/metrics.jsonl"
runs = [json.loads(l) for l in p.read_text().splitlines() if l.strip()]
p11 = [r for r in runs if r.get("phase") == "1.1"]
if not p11:
    print("pending: no 1.1 runs logged"); sys.exit(2)
latest = p11[-1]
detectors = latest.get("detectors", {})
if len(detectors) < 4:
    print(f"FAIL: only {len(detectors)} detectors wired (need ≥4)"); sys.exit(1)
bad = {k: v for k, v in detectors.items() if v.get("auc", 0) < 0.85}
if bad:
    print(f"FAIL: detectors below AUC=0.85 threshold: {bad}"); sys.exit(1)
print(f"PASS: {len(detectors)} detectors, all AUC ≥ 0.85")
sys.exit(0)
PY
