# Humanizer

Style-preserving AI-text humanizer targeting **Alex Mamishev** as the style author.

See [SPEC.md](SPEC.md) for the full research spec.

Run by a rubber-duck pair (Peer-A implementer + Peer-B verifier) spawned by main
Neil in response to a single seed prompt.

## Progress

This file is auto-updated by main Neil after each rubber-duck cycle.

- **Current phase:** (not yet started)
- **Last update:** —
- **Peer-A:** —
- **Peer-B:** —
- **Last 3 metrics:** —
- **Open blockers:** —

## State

- `state/phase.json` — current phase + sub-task
- `state/metrics.jsonl` — append-only log of every metric run
- `logs/peer_a.log`, `logs/peer_b.log` — per-peer stdout

## Corpus

- `author_corpus/mamishev_clean.jsonl` — 1393 cleaned Mamishev paragraphs
- `author_corpus/alex_mamishev.txt` — compact style description

## Seed prompt (what to paste into main Neil's TUI to launch)

See `SEED_PROMPT.md`.
