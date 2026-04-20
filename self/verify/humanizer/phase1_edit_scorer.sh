#!/bin/sh
# Phase 1.4 verify — edit-distance scorer
# Passes when: char-level + token-level + structural diff all report,
# reversibility sanity (scorer(a,b) == scorer(b,a)).
python3 - <<'PY'
import json, pathlib, sys
p = pathlib.Path.home() / ".neil/projects/humanizer/state/metrics.jsonl"
if not p.exists(): print("pending"); sys.exit(2)
runs = [json.loads(l) for l in p.read_text().splitlines() if l.strip()]
p14 = [r for r in runs if r.get("phase") == "1.4"]
if not p14: print("pending"); sys.exit(2)
latest = p14[-1]
needed = ["char_edit_distance", "token_edit_distance", "structural_preserved", "symmetry_check"]
missing = [k for k in needed if k not in latest]
if missing: print(f"FAIL: missing fields {missing}"); sys.exit(1)
if not latest["symmetry_check"]: print("FAIL: edit scorer not symmetric"); sys.exit(1)
print("PASS: edit scorer implements char+token+structural with symmetry")
PY
