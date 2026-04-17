# Grammar expressiveness stress test

Purpose: answer the open question from 2026-04-16T15:02 beat:
"Is flat flags=csv key=value expressive enough, or do we need
structured values (TOML/JSON) for lists and nested objects?"

Method: enumerate realistic future prompt scenarios drawn from
actual openclaw subsystems (webhook, schedule, vision, retrospective,
multi-recipient notifies, parallel beats). For each, decide whether
flat key=value suffices. If >=30% need structure, evolve the grammar
NOW before integration locks the flat form in.

Rule: a scenario "needs structure" only if the flat form is genuinely
lossy or ambiguous -- not merely less pretty.

## Scenarios

### S1. Multiple reply-to paths (webhook fan-out)

**Need:** webhook watcher triggered by two subscribers; result must be
written to two separate files.

**Flat:** `reply-to=/path/a,/path/b`
**Flat verdict:** works IF we declare "reply-to is CSV on comma." No
escape needed because POSIX paths can't contain commas without being
pathological.
**Structured gain:** zero.
**Needs structure:** NO.

### S2. Structured context injection (operator correction with scope)

**Need:** `CONTEXT: {type=correction, scope=last-beat, severity=high}`
-- operator wants to inject a typed correction, not just free text.

**Flat:** `flags=context context-type=correction context-scope=last-beat context-severity=high`
**Flat verdict:** works via key-namespacing (`context-*`). Adds 3 keys
to the namespace but the grammar doesn't care.
**Structured gain:** slight aesthetic win, zero semantic win. The body
of the prompt is the natural place for the correction content; the
header only needs scalar metadata.
**Needs structure:** NO.

### S3. Budget caps (token, wall-clock, API dollars)

**Need:** a parallel-beat framework wants to pin a beat to
"max 20k tokens OR 90 seconds OR $0.05, whichever first."

**Flat:** `max-tokens=20000 timeout=90s max-cost=0.05`
**Flat verdict:** trivial, three scalars.
**Structured gain:** none.
**Needs structure:** NO.

### S4. Tool whitelist (safety: only allow specific tools)

**Need:** sandbox mode -- "only CALL: mempalace and zettel, deny all
others."

**Flat:** `allow-tools=mempalace,zettel` (CSV of tool names)
**Flat verdict:** works. Tool names don't contain commas.
**Structured gain:** none, unless we later want per-tool options like
`{mempalace: {max-results=10}, zettel: {read-only=true}}`. But those
are per-deployment config, not per-prompt, so they belong in services/
registry, not the prompt header.
**Needs structure:** NO.

### S5. Dependency chain (run after prompt X and prompt Y complete)

**Need:** a retrospective beat should wait until both
`beat-abc` and `beat-def` finish.

**Flat:** `after=beat-abc,beat-def`
**Flat verdict:** works as CSV. Prompt IDs are short ASCII tokens.
**Structured gain:** if we ever need logical conditions (`after=A AND
(B OR C)`), flat fails. But that's a scheduler problem, not a grammar
problem -- express it as a proper dependency spec file, not a header.
**Needs structure:** NO (for the grammar; the scheduler can evolve
separately).

### S6. Named output schema (structured result expected)

**Need:** a test-mode prompt wants back `{status: pass|fail, errors:
[...], duration-ms: N}`.

**Flat:** `expect-schema=test-result-v1` -- just name the schema.
**Flat verdict:** works, because the schema definition lives elsewhere.
**Structured gain:** none. Inlining schemas in prompt headers would be
deranged.
**Needs structure:** NO.

### S7. Multi-flag with mutually exclusive pairs

**Need:** operator forgets and writes `flags=test,dry-run,context`
(three classification flags, all top-priority).

**Flat:** the grammar already specifies priority order (context > test
> dry-run > urgent). Parser picks the highest-priority one and warns.
**Structured gain:** none. A nested `{classification: "test"}` form
would prevent the mistake, but so does just documenting the flag set.
**Needs structure:** NO.

### S8. Parameterized retrospective (analyze last N beats, exclude idle)

**Need:** `retro --last=20 --exclude-types=idle,recover`

**Flat:** `retro-last=20 retro-exclude=idle,recover`
**Flat verdict:** works. Two keys, one CSV value.
**Structured gain:** if retro options keep growing, the `retro-*`
namespace feels cluttered. But cluttered != broken. Real problem is
that header options should be sparse; complex tool configs belong in
the prompt body or a referenced file.
**Needs structure:** NO.

### S9. Nested trace context (distributed-tracing style)

**Need:** `parent=beat-X trace-id=abc-123 span-id=def-456 baggage=user-role=operator,region=us-west`

**Flat:** works. All scalars and one CSV.
**Flat verdict:** fine. W3C trace-context is literally designed to be
flat k=v.
**Structured gain:** zero.
**Needs structure:** NO.

### S10. Header-in-body (operator drops prompt with multi-line intent)

**Need:** operator writes a 5-paragraph prompt and adds structured
metadata like "reply in markdown, cite sources, max 200 words."

**Flat:** `format=markdown cite=true max-words=200`
**Flat verdict:** works. Three scalars.
**Structured gain:** none.
**Needs structure:** NO.

## Tally

10 scenarios examined. 0 require structured values.

All realistic future uses fit:
- CSV for lists (paths, tool names, dependency IDs)
- Key-namespacing for grouped options (`context-*`, `retro-*`)
- Body-of-prompt for anything that's actually content, not metadata

## The one thing flat CANNOT express

Genuinely nested data where BOTH keys and values are dynamic, e.g.:
`{handlers: {error: {retry: 3, backoff: "exp"}, success: {notify: [a, b]}}}`

But this isn't prompt metadata -- it's configuration. It belongs in a
config file referenced by the prompt (`config=/path/to/handlers.toml`),
not inlined into a prompt header.

## Decision

Keep the grammar flat: `#!openclaw: flags=csv key=value key=value ...`

Rationale:
1. Every realistic scenario fits flat form without loss.
2. Parser stays under 80 lines of C (invariant preserved).
3. Grep-ability: `grep "flags=test" queue/` works; JSON doesn't grep.
4. Operator ergonomics: flat headers are one-line and typeable; nested
   forms invite escaping nightmares.
5. Escape valve: when genuinely nested metadata is needed, reference
   an external config file (`config=<path>`). This is the standard
   Unix pattern (cf. systemd unit files, git config).

## Implication for integration

Integration can proceed on the v0 spec without grammar changes. The
prototype at src/parse_prompt_header.c is ready to wire in.

One small addition to the spec: document the "reference external
config" escape valve so future-me doesn't rediscover it under pressure.

## What would invalidate this decision later

If two or more of the following happen, revisit:
- Three or more callers need the same `<namespace>-*` key cluster.
- Operators complain about header readability.
- A use case emerges where header values genuinely carry nested data
  (not just namespaced scalars).

Until then, flat is correct.

---

Written 2026-04-16. Beat 15:07. Answers the 15:02 question decisively
so integration can proceed without grammar-churn risk.
