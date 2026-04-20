# Author Corpus — Alex Mamishev

Staged for the Humanizer project. The only data transferred from the broader
`humanize/` project; everything else is built fresh here.

## Files

- `mamishev_clean.jsonl` (3.7 MB, 1393 paragraphs) — cleaned corpus of Mamishev's
  academic / technical writing. Each line is a JSON record with `{text, source, ...}`.
- `alex_mamishev.txt` (6 KB) — compact style description used by the LLM rewriter (Primitive 2.5).
- `README.md` — style-folder notes from the originating project.

## Provenance

Assembled by the openclaw/humanize project (separate tree). The corpus is
derivative of Mamishev's published academic works and online writing; used here
only to build the style-match evaluator and fine-tune the holistic rewriter
(2.5). Not redistributed.

## Usage

- Phase 1.2 style extractor treats this as the target author fingerprint.
- Phase 2.5 LLM rewriter fine-tunes on the clean jsonl.
- No other use.
