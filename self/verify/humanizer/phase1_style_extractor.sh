#!/bin/sh
# Phase 1.2 verify — style extractor
# Passes when: stylometry + embedding fingerprint discriminate Mamishev from
# 100 decoy authors with cosine gap ≥ 0.15.
python3 - <<'PY'
import json, pathlib, sys
p = pathlib.Path.home() / ".neil/projects/humanizer/state/metrics.jsonl"
if not p.exists(): print("pending"); sys.exit(2)
runs = [json.loads(l) for l in p.read_text().splitlines() if l.strip()]
p12 = [r for r in runs if r.get("phase") == "1.2"]
if not p12: print("pending"); sys.exit(2)
latest = p12[-1]
gap = latest.get("author_decoy_gap")
if gap is None: print("FAIL: no author_decoy_gap metric"); sys.exit(1)
if gap < 0.15: print(f"FAIL: cosine gap {gap:.3f} < 0.15"); sys.exit(1)
print(f"PASS: author-decoy cosine gap = {gap:.3f}")
PY
