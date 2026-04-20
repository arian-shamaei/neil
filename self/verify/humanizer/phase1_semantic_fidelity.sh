#!/bin/sh
# Phase 1.3 verify — semantic fidelity scorer
# Passes when: BERTScore + NLI pipeline gives Pearson r ≥ 0.8 vs human-rated
# paraphrase quality (WMT subset).
python3 - <<'PY'
import json, pathlib, sys
p = pathlib.Path.home() / ".neil/projects/humanizer/state/metrics.jsonl"
if not p.exists(): print("pending"); sys.exit(2)
runs = [json.loads(l) for l in p.read_text().splitlines() if l.strip()]
p13 = [r for r in runs if r.get("phase") == "1.3"]
if not p13: print("pending"); sys.exit(2)
latest = p13[-1]
r = latest.get("pearson_r_vs_human")
if r is None: print("FAIL: no pearson_r_vs_human metric"); sys.exit(1)
if r < 0.8: print(f"FAIL: Pearson r = {r:.3f} < 0.80"); sys.exit(1)
print(f"PASS: semantic fidelity scorer Pearson r = {r:.3f}")
PY
