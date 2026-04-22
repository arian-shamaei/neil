#!/usr/bin/env python3
"""Aggregate per-section render times from neil_bench.jsonl."""
import json, sys
from collections import defaultdict
from pathlib import Path

path = Path(sys.argv[1] if len(sys.argv) > 1 else '/tmp/neil_bench.jsonl')
if not path.exists():
    print(f'ERROR: no bench file at {path}', file=sys.stderr); sys.exit(1)

by_section = defaultdict(list)
total = 0
with path.open() as f:
    for line in f:
        try:
            obj = json.loads(line)
            by_section[obj['s']].append(obj['us'])
            total += 1
        except (json.JSONDecodeError, KeyError):
            continue

def pct(xs, p):
    xs_s = sorted(xs)
    i = max(0, min(int(len(xs_s) * p / 100), len(xs_s) - 1))
    return xs_s[i]

print(f'\nBench report from {path}  ({total} events, {len(by_section)} sections)\n')
print(f'{"Section":<28} {"count":>6} {"p50_ms":>8} {"p95_ms":>8} {"p99_ms":>8} {"max_ms":>8} {"mean_ms":>8}')
print('-' * 80)

rows = []
for section, xs in by_section.items():
    rows.append((
        section, len(xs),
        pct(xs,50)/1000, pct(xs,95)/1000, pct(xs,99)/1000,
        max(xs)/1000, sum(xs)/len(xs)/1000,
    ))
# Sort by p95 descending — worst-smoothness panels on top
for r in sorted(rows, key=lambda x: -x[3]):
    print(f'{r[0]:<28} {r[1]:>6} {r[2]:>8.2f} {r[3]:>8.2f} {r[4]:>8.2f} {r[5]:>8.2f} {r[6]:>8.2f}')

# Quick smoothness verdict per panel (33ms = 30fps target; >33ms p99 = stutter)
print()
print('Smoothness verdict (target: p99 < 33ms for 30fps):')
for r in sorted(rows, key=lambda x: -x[3]):
    if not r[0].startswith('render.'): continue
    verdict = 'OK' if r[4] < 33 else ('HITCH' if r[4] < 100 else 'BAD')
    print(f'  {r[0]:<28} p99={r[4]:>6.2f}ms  {verdict}')
