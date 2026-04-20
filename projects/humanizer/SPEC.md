# Humanizer — Research Spec

**Goal.** Given `input_text` (AI-generated) and `author_corpus` (target human's writing),
produce `output_text` that:
1. Passes ≥4 SOTA AI detectors as "human" at ≥80% confidence
2. Matches `author_corpus` style within cosine distance ≤ ε
3. Minimizes edit distance vs input (the "change as little as possible" constraint)
4. Preserves semantic meaning (BERTScore ≥ 0.90 vs input)
5. Works scale-invariant: 100-tok snippets up to 100K-tok documents

Target author: **Alex Mamishev** (corpus at `author_corpus/mamishev_clean.jsonl`, 1393
cleaned paragraphs).

---

## Neil-pair roles (rubber-duck)

- **Peer-A — Implementer.** Writes code, produces candidate transformations,
  fine-tunes or calls models, emits proposed outputs.
- **Peer-B — Verifier.** Runs the detector bench, computes metrics, does
  attribution on failures, pushes back with concrete "sentence N still
  flags detector M" feedback.

Termination of each sub-task: Peer-B emits `DONE: <subtask> verify=pass`.
Overall termination: all Phase 4 tests pass.

Cadence rule: Peer-A produces → Peer-B verifies → one iteration is ONE cycle.
Each sub-task budgets 20 cycles max before escalating to main Neil.

---

## Phase 1 — Ground Truth & Metrics (foundation)

Nothing built later is falsifiable without this. All of Phase 1 is instrumentation,
not transformation.

| # | Deliverable | Metric (must meet to pass) | Verify script |
|---|---|---|---|
| 1.1 | Detector bench: GPTZero + Originality.ai + Binoculars + RADAR + Ghostbuster | AUC ≥ 0.85 per detector on 500 labeled samples (held-out) | `self/verify/humanizer/phase1_detector_calibration.sh` |
| 1.2 | Style extractor: stylometry (burstiness, perplexity, TTR, sentence-length stdev, punctuation patterns) + author-embedding fingerprint | Cosine gap ≥ 0.15 between Mamishev vs 100 decoy authors | `self/verify/humanizer/phase1_style_extractor.sh` |
| 1.3 | Semantic fidelity scorer: BERTScore + NLI entailment pipeline | Pearson r ≥ 0.8 vs human-rated paraphrase quality (WMT subset) | `self/verify/humanizer/phase1_semantic_fidelity.sh` |
| 1.4 | Edit-distance scorer: char + token + structural (sentence boundaries preserved?) | All three diff types reported, reversibility check | `self/verify/humanizer/phase1_edit_scorer.sh` |

---

## Phase 2 — Transformation Primitives

Each primitive must independently move ≥1 metric without blowing the others.
Minimal-edit primitives land first; heavier ones only if lighter ones fail.

| # | Primitive | Hypothesis | Target metric |
|---|---|---|---|
| 2.1 | Burstiness reintroduction (sentence-length variance) | AI output is flat; humans oscillate | Detector ↓, edit ~5% |
| 2.2 | Idiolect transfer (insert author's signature n-grams where meaning allows) | Vocabulary is a strong style signal | Style-match ↑, edit ~3–8% |
| 2.3 | Hedging & discourse markers | Humans equivocate; AI doesn't | Detector ↓, edit ~2% |
| 2.4 | Punctuation/capitalization micro-perturbation | Em-dash preference, comma splices | Detector ↓, edit <1% |
| 2.5 | LLM rewriter (Llama 3.1 8B fine-tuned on Mamishev corpus) | Holistic rewrite when primitives insufficient | All metrics, edit 20–40% |

Verify per primitive: runs on 100 held-out AI samples; reports
metric deltas vs baseline in `state/metrics.jsonl`.

---

## Phase 3 — Orchestration

The minimal-edit policy in action: apply cheapest primitive first, re-detect,
stop when pass threshold reached, and **revert any edit that didn't move the needle.**

| # | Component | Purpose |
|---|---|---|
| 3.1 | Chunker | Splits input into N-token windows w/ overlap; scale-invariant |
| 3.2 | Attribution | Per-sentence detector-score breakdown — which sentences fire hardest? |
| 3.3 | Greedy rewrite loop | Cheapest→heaviest primitive order; stop at threshold; revert no-ops |
| 3.4 | Cross-chunk style coherence | Stitched output must not alternate voices |

Verify: end-to-end on 100 samples × 3 length buckets (100/1K/10K tok);
pass rate ≥ 80% at mean edit distance ≤ 15%.

---

## Phase 4 — Evaluation Testbench

| # | Test | Measures |
|---|---|---|
| 4.1 | Automated bench | 4 metrics × 500 samples × 3 length buckets; per-sample receipts |
| 4.2 | Ablation matrix | Each primitive alone vs combined vs full pipeline |
| 4.3 | Blind style review | Human panel: is this Mamishev or decoy? Chance = failure |
| 4.4 | Robustness | 30-day re-check against newer detector; survives? |

---

## Success criteria (the whole system)

The humanizer is "done" when **all four** hold simultaneously on a
held-out test set of 500 Mamishev-domain AI-generated samples:

1. **Detection pass rate ≥ 85%** — ≥85% of outputs pass ≥4/5 detectors at ≥80% "human" confidence
2. **Style match ≥ 0.75** — cosine similarity to Mamishev author fingerprint ≥ 0.75
3. **Semantic fidelity ≥ 0.90** — BERTScore ≥ 0.90 vs input
4. **Mean edit distance ≤ 15%** — minimal-edit principle respected

---

## Anti-goals (explicit)

- NOT a generator — does not produce new content, only transforms existing.
- NOT style theft at scale — corpus is a known public author (Mamishev) with academic works; target use is protecting his own output from false positives.
- NOT a jailbreak — does not attempt to bypass safety policies of underlying models.

---

## Budget & stop conditions

- **Per-subtask budget:** 20 rubber-duck cycles max before escalation.
- **Daily token budget:** $20 USD across both peers.
- **Any failure of semantic fidelity ≥ 0.10 drop** → revert, log to failures.json, do not proceed.
- **If Peer-B flags the same detector failure twice in a row** → escalate, don't loop.
