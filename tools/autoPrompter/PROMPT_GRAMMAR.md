# openclaw prompt header grammar (v0 draft)

Status: DRAFT spec, not yet wired into autoprompt.c. Reviewed for feasibility.
Written 2026-04-16 to crystallize last beat's insight: dedup granularity is the
wrong lever; prompt classification is. A prompt should self-describe its intent
BEFORE any downstream routing (dedup, dispatch, logging, reply).

## Why a grammar, not more prefixes?

Ad-hoc prefixes (`TEST:`, `FORCE:`, `URGENT:`) stack poorly. A prompt that is
both urgent AND quiet AND force-rerun needs three prefixes on one line, or the
parser has to learn combinatorial rules. A single structured header line is
parseable in 20 lines of C and extensible without grammar churn.

## Format

First non-blank line of the prompt file, if it begins with `#!openclaw:`,
is the header. Everything after that line is the prompt body.

    #!openclaw: flags=<csv> [key=value ...]
    <body...>

If the header is absent, defaults apply (equivalent to `flags=` with no flags
set). Headers are case-insensitive for flag/key names, case-sensitive for
values.

## Flags (boolean, presence = true)

| Flag         | Meaning                                                              |
|--------------|----------------------------------------------------------------------|
| test         | Verify mode: run tool calls, return pass/fail, skip heartbeat log    |
| force        | Bypass dedup ring buffer -- always process even if hash matches      |
| urgent       | Jump queue -- process before other pending prompts                   |
| quiet        | Suppress NOTIFY outputs (no terminal/email/slack side effects)       |
| dry-run      | Parse action lines but do not execute them                           |
| context      | Inject body into next beat's memory search, do not trigger own beat  |
| observe-only | Skip running observe.sh; use already-injected context (faster)       |
| no-memory    | Skip mempalace search for this prompt                                |

## Key=value options

| Key        | Values                                | Meaning                              |
|------------|---------------------------------------|--------------------------------------|
| priority   | low / medium / high / critical        | Scheduling hint for queue sort       |
| timeout    | <int>s or <int>m                      | Hard cap on Claude invocation        |
| tag        | <label>                               | Correlate with intentions/retro      |
| reply-to   | <path>                                | Write result to this file            |
| parent     | <prompt-id>                           | Causal link for trace reconstruction |

## Routing

After parsing, the dispatcher routes:

    flags contains 'context'     -> append to next beat's context, return
    flags contains 'test'        -> verify handler (minimal, pass/fail)
    flags contains 'dry-run'     -> plan handler (parse + report, no exec)
    flags contains 'urgent'      -> queue-front insert
    else                         -> normal full-beat handler

Multiple classification flags are mutually exclusive in priority order above
(context beats test beats dry-run). Orthogonal flags (force, quiet, no-memory,
observe-only) always apply regardless of classification.

## Examples

    #!openclaw: flags=test
    verify mcp bash tool works

    #!openclaw: flags=test,quiet tag=mcp-verify
    parallel tool-call smoke test

    #!openclaw: flags=urgent priority=critical
    disk 95% full -- act now

    #!openclaw: flags=context
    operator just said they prefer terser summaries

    #!openclaw: flags=dry-run
    propose a plan to refactor the zettel index format

    #!openclaw: flags=force
    re-run the same verification prompt to check stability

## Collapsing three proposed systems into one

This grammar unifies three separate proposals from recent beats:

1. **Tool adapter** (beats 13:12, 13:15): three backends coexist. Not needed --
   the live backend is a deployment property, not a per-prompt property.
   Observability of which backend handled a prompt can live in the reply-to
   metadata, not a runtime abstraction.

2. **Layered dedup** (beat 13:12, superseded by beat 14:02): exact-hash dedup
   was correct for "accidental double-drop"; everything above that is a
   classification problem. `force` flag solves legitimate re-run. `test`
   flag solves "please don't dedup this verification run."

3. **Test harness mode** (beat 13:15): becomes `flags=test` -- no separate
   binary, no separate config, just a classification the existing dispatcher
   consults.

## Invariants to preserve when implementing

- Backward compatible: prompts without header behave identically to today.
- Parser fits in <80 lines of C, reuses existing file-read in load_prompt_file().
- Unknown flags/keys are logged and ignored, never fatal.
- Header malformation (e.g., `#!openclaw:` with garbled body) falls back to
  default routing with a warning log, never crashes the daemon.
- The blueprint TUI shows header-parsed classification in the queue panel.

## Next steps (intend after review)

1. Add `prompt_header_t` struct and `parse_header()` to autoprompt.c.
2. Thread `header` into process_prompt() dispatch switch.
3. Implement `test` and `context` handlers first (highest operator value).
4. Add header-awareness to dedup.log (don't skip `force` prompts).
5. Surface parsed header in blueprint queue panel.
6. Document in essence/actions.md once stable.

## Addendum: Flat-vs-structured decision (2026-04-16)

Validated after stress-testing 10 realistic future scenarios
(GRAMMAR_STRESS_TEST.md). Decision: **keep the grammar flat.**

Zero of the 10 scenarios need structured values. All fit via:
- CSV for lists (paths, tool names, dependency IDs)
- Key-namespacing (`context-*`, `retro-*`) for grouped options
- Body-of-prompt for content vs metadata

**Escape valve for genuinely nested config:** reference an external
config file via `config=<path>`. This is the standard Unix pattern
(systemd unit files, git config). The header stays flat; complex
shape lives next to it, not inside it.

**Revisit trigger:** revisit only if two or more happen:
- Three+ callers need the same namespaced key cluster.
- Operators report header readability pain.
- A use case genuinely needs nested values in the header.

Until then, flat is correct. Integration unblocked.
