#!/usr/bin/env python3
"""
beat_audit.py -- Personal reviewer for Neil.

Queries disk state (heartbeat log, intentions, result files) and reports
whether the beat router is actually producing the behavior it promised.

Success criteria (from beat_router/README.md):
 1. Mode distribution roughly balanced (or skewed toward CHAR/CREAT
    when intentions exist)
 2. INTENDs created per Configuration beat > 0.5
 3. INTEND completion rate > 50%
 4. Zero ungrounded Creativity beats (every Creativity has prior
    Characterization OR pending intention)
 5. Proposal-to-ship ratio > 0.5

Usage:
    beat_audit.py                  # last 20 beats
    beat_audit.py --beats 40       # last 40 beats
    beat_audit.py --verbose        # per-beat detail
"""

import argparse
import json
import os
import re
import sys
from pathlib import Path

NEIL_HOME = Path(os.environ.get("NEIL_HOME", os.path.expanduser("~/.neil")))

CONFIG_KW = ["read", "traced", "mapped", "studied", "profiled",
             "deep-read", "examined", "inspect", "surveyed", "honored"]
CHAR_KW = ["verified", "confirmed", "tested", "validated", "diagnosed"]
CREAT_KW = ["fixed", "built", "deployed", "wrote", "created", "added",
            "patched", "integrated", "shipped", "drafted", "ran"]

PROPOSAL_PATTERNS = [
    r"\bi'?d (?:build|prototype|add|create|ship|draft)",
    r"\b(?:would|should) (?:build|create|add|ship|prototype)",
    r"\bnext (?:step|move|evolution) (?:would|is|should) be",
    r"\bif i had (?:\d+ )?more beats",
    r"\bcould formalize",
    r"\bthe fix is",
]


def classify_action(action: str) -> str:
    a = (action or "").lower()
    for kw in CONFIG_KW:
        if kw in a:
            return "CONFIGURATION"
    for kw in CHAR_KW:
        if kw in a:
            return "CHARACTERIZATION"
    for kw in CREAT_KW:
        if kw in a:
            return "CREATIVITY"
    return "UNKNOWN"


def load_heartbeats(limit: int) -> list[dict]:
    path = NEIL_HOME / "heartbeat_log.json"
    if not path.exists():
        return []
    beats = []
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            beats.append(json.loads(line))
        except json.JSONDecodeError:
            pass
    return beats[-limit:]


def load_intentions() -> list[dict]:
    path = NEIL_HOME / "intentions.json"
    if not path.exists():
        return []
    out = []
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            out.append(json.loads(line))
        except json.JSONDecodeError:
            pass
    return out


def find_result_file(prompt: str) -> Path | None:
    hist = NEIL_HOME / "tools/autoPrompter/history"
    if not hist.exists():
        return None
    for p in hist.iterdir():
        name = p.name
        if name.endswith(".result.md") and prompt in name:
            return p
    return None


def proposals_in_contribution(contrib: str) -> int:
    c = (contrib or "").lower()
    return sum(1 for pat in PROPOSAL_PATTERNS if re.search(pat, c))


def has_intend_in_output(result_path: Path) -> bool:
    if not result_path or not result_path.exists():
        return False
    text = result_path.read_text()
    # Look in Output section
    if "## Output" not in text:
        return False
    output = text.split("## Output", 1)[1]
    for line in output.splitlines():
        stripped = line.strip()
        if stripped.startswith("INTEND:"):
            return True
    return False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--beats", type=int, default=20)
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()

    beats = load_heartbeats(args.beats)
    intentions = load_intentions()

    if not beats:
        print("No beats to audit.")
        return 0

    # ── Classify each beat ──
    classified = []
    for b in beats:
        mode = classify_action(b.get("action", ""))
        classified.append({**b, "mode": mode})

    # ── Mode distribution ──
    mode_counts = {"CONFIGURATION": 0, "CHARACTERIZATION": 0,
                   "CREATIVITY": 0, "UNKNOWN": 0}
    for c in classified:
        mode_counts[c["mode"]] += 1
    total = len(classified)

    # ── Intention activity (within beat window timeframe) ──
    window_start = classified[0]["timestamp"] if classified else ""
    window_end = classified[-1]["timestamp"] if classified else ""

    # Count intentions created/completed in window
    intent_created = 0
    intent_completed = 0
    intent_pending = 0
    for i in intentions:
        created = (i.get("created") or "").replace("T", "T")
        # created uses - separators in date portion; same format as timestamps
        if created and window_start <= created <= window_end + "Z":
            intent_created += 1
            if i.get("status") == "completed":
                intent_completed += 1
        if i.get("status") == "pending":
            intent_pending += 1

    # Simpler approach: count all pending vs completed total
    total_pending = sum(1 for i in intentions if i.get("status") == "pending")
    total_completed = sum(1 for i in intentions if i.get("status") == "completed")

    # ── Grounding check: Creativity beats with/without prior Characterization ──
    grounded_creat = 0
    ungrounded_creat = 0
    for i, b in enumerate(classified):
        if b["mode"] != "CREATIVITY":
            continue
        # Look back up to 3 beats for a CHARACTERIZATION or CONFIGURATION
        prior_window = classified[max(0, i - 3):i]
        has_prior = any(p["mode"] in ("CHARACTERIZATION", "CONFIGURATION")
                        for p in prior_window)
        # Also grounded if there was a pending intention at the time
        # (simplified: assume grounded if has_prior)
        if has_prior:
            grounded_creat += 1
        else:
            ungrounded_creat += 1

    # ── Proposal-to-ship ratio ──
    total_proposals = 0
    shipped_proposals = 0
    for b in classified:
        p = proposals_in_contribution(b.get("contribution", ""))
        total_proposals += p
        if p > 0:
            # Did this beat (or the next) actually result in an INTEND or shipped code?
            rf = find_result_file(b.get("prompt", ""))
            if has_intend_in_output(rf):
                shipped_proposals += p  # credit each proposal if any INTEND was emitted

    proposal_ratio = (shipped_proposals / total_proposals) if total_proposals > 0 else None

    # ── Configuration INTEND rate ──
    config_beats = [c for c in classified if c["mode"] == "CONFIGURATION"]
    config_intends = 0
    for b in config_beats:
        rf = find_result_file(b.get("prompt", ""))
        if has_intend_in_output(rf):
            config_intends += 1
    config_intend_rate = (config_intends / len(config_beats)) if config_beats else None

    # ── Completion rate ──
    completion_rate = (intent_completed / intent_created) if intent_created > 0 else None

    # ── Emit report ──
    print("=" * 70)
    print(f"  BEAT AUDIT: last {total} beats")
    print(f"  window: {window_start} -> {window_end}")
    print("=" * 70)
    print()
    print("Mode distribution:")
    for m in ("CONFIGURATION", "CHARACTERIZATION", "CREATIVITY", "UNKNOWN"):
        n = mode_counts[m]
        pct = 100 * n / total if total else 0
        bar = "#" * int(pct / 5)
        print(f"  {m:<18} {n:>3} ({pct:5.1f}%)  {bar}")
    print()

    print("Intention activity (in window):")
    print(f"  Created:   {intent_created}")
    print(f"  Completed: {intent_completed}")
    if completion_rate is not None:
        print(f"  Rate:      {100 * completion_rate:.0f}%")
    print(f"  Total pending now: {total_pending}")
    print()

    print("Grounding check (Creativity beats):")
    total_creat = grounded_creat + ungrounded_creat
    if total_creat > 0:
        print(f"  Grounded:   {grounded_creat}/{total_creat} "
              f"({100*grounded_creat/total_creat:.0f}%)")
        print(f"  Ungrounded: {ungrounded_creat}/{total_creat} "
              f"({100*ungrounded_creat/total_creat:.0f}%)")
    else:
        print("  (no Creativity beats in window)")
    print()

    print("Configuration productivity:")
    if config_beats:
        print(f"  Config beats that emitted INTEND: {config_intends}/{len(config_beats)} "
              f"({100*config_intend_rate:.0f}%)")
    else:
        print("  (no Configuration beats in window)")
    print()

    print("Proposal-to-ship ratio:")
    print(f"  Proposals in CONTRIBUTION fields: {total_proposals}")
    print(f"  Resulted in INTEND or code:       {shipped_proposals}")
    if proposal_ratio is not None:
        print(f"  Ratio:                            {proposal_ratio:.2f}")
    else:
        print("  (no proposals found)")
    print()

    # ── Verdict ──
    checks = []
    checks.append(("Mode distribution balanced (each > 10%)",
                   all(mode_counts[m] / total > 0.10
                       for m in ("CONFIGURATION", "CHARACTERIZATION", "CREATIVITY")
                       if mode_counts[m] > 0) if total else False))
    checks.append(("INTEND completion rate > 50%",
                   (completion_rate is not None and completion_rate > 0.5)))
    checks.append(("Zero ungrounded Creativity beats",
                   ungrounded_creat == 0))
    checks.append(("Configuration INTEND rate > 50%",
                   (config_intend_rate is not None and config_intend_rate > 0.5)))
    checks.append(("Proposal-to-ship ratio > 0.5",
                   (proposal_ratio is not None and proposal_ratio > 0.5)))

    passed = sum(1 for _, ok in checks if ok)
    total_checks = len(checks)

    print("=" * 70)
    print(f"  VERDICT: {passed}/{total_checks} checks passed")
    print("=" * 70)
    for desc, ok in checks:
        marker = "PASS" if ok else "FAIL"
        print(f"  [{marker}] {desc}")
    print()

    if passed == total_checks:
        print("  All criteria met. Beat router is improving behavior.")
    elif passed >= 3:
        print("  Partial win. Router is helping but needs tuning.")
    else:
        print("  Router did not produce significant improvement.")
        print("  Consider: mode_routing = false in config.toml to revert.")

    if args.verbose:
        print()
        print("-" * 70)
        print("  PER-BEAT DETAIL")
        print("-" * 70)
        for c in classified:
            ts = c.get("timestamp", "")
            mode = c["mode"]
            act = (c.get("action") or "")[:60]
            print(f"  {ts}  {mode:<16}  {act}")

    return 0 if passed == total_checks else 1


if __name__ == "__main__":
    sys.exit(main())
