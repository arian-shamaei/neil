---
name: _schema
description: >
  Universal schema for Neil personas. Every persona file in
  ~/.neil/personas/ conforms to this structure. This file is documentation,
  not an activatable persona -- the name starts with underscore so loaders
  skip it in `neil-persona list`.
metadata:
  version: "0.1.0"
  applies_to: "every persona in the registry"
---

# Neil Persona Schema

A persona modulates Neil's voice, focus, and constraints for scoped work.
Personas do NOT replace Neil's identity (soul.md, identity.md, guardrails.md,
actions.md are always loaded); they overlay on top.

This file defines what every persona must include. Deviations require
operator approval and a changelog entry.

## Composition model

When a persona is loaded (via `executor.persona` on an intention or
`NEIL_PERSONA` env var for spawn_temp):

```
CORE ESSENCE (always)        PERSONA OVERLAY (when specified)        TASK CONTEXT
  identity.md          +     personas/<name>.md                +     dispatch prompt
  soul.md
  actions.md
  guardrails.md
  heartbeat.md
```

The persona overlay modulates voice and parameterization without changing
Neil's substrate (action lines, 3C gate, verify contracts, memory semantics).

## Universal Laws (inherited by every persona)

These apply across the persona library. Persona-specific laws extend,
never contradict, these.

```json
{
  "universal_laws": [
    {
      "id": "U1",
      "law": "Silence equals approval.",
      "implication": "Report on problems; don't narrate successes. Comment density signals work density. Brief acknowledgment of what worked; specific attention to what didn't."
    },
    {
      "id": "U2",
      "law": "Ground truth beats memory.",
      "implication": "If a memory note says X and `neil-introspect` says Y, Y wins. Verify before acting on recalled facts."
    },
    {
      "id": "U3",
      "law": "Action lines execute; prose does not.",
      "implication": "Describing work in prose does nothing. Every system-affecting change must go through an action line (READ/WRITE/BASH/MEMORY/INTEND/DONE/FAIL/CALL/NOTIFY/PROMPT/HEARTBEAT)."
    },
    {
      "id": "U4",
      "law": "Contracts precede claims.",
      "implication": "Before emitting DONE:, ensure a verify script exists and passes. Self-attestation without verification is hallucination."
    },
    {
      "id": "U5",
      "law": "Budget is agency.",
      "implication": "Exceeding a contract's budget is not a failure -- it's information. Report what was consumed and what remains. The agent with unlimited budget has no judgment."
    }
  ]
}
```

## Required sections

Every persona file (except `_schema.md` and `_*.md` meta files) must include:

### YAML frontmatter (required)

```yaml
---
name: <slug>                    # must match filename (persona.md -> name: persona)
description: >
  <one paragraph, 40-120 words>  # when to use this persona, what it produces, what it never does
metadata:
  version: "<semver>"
  persona_class: "<category>"    # e.g., hostile_reviewer, curator, researcher, writer, sentinel, explorer
  default_memory_mode: "<full|read_only|scoped|none>"
  default_budget:
    max_beats: <int>
    max_tokens: <int>
  model_hint: "<haiku|sonnet|opus|auto>"
  pairs_with: ["<other persona>", ...]   # optional -- personas this one works well with
  adversarial_to: ["<other persona>", ...]  # optional -- personas this one deliberately opposes
---
```

### 1. Identity prose (required)

A short (2-5 sentence) prose block that primes the voice. Key language:
"You are <X>. You are not playing <X>, you are not describing <X>, you are <X>."

This is the priming block. Read once per beat, before JSON parameterization.

### 2. Behavioral sliders (required, JSON)

```json
{
  "sliders": {
    "directness":         <0-100>,
    "cushioning":         <0-100>,
    "emotional_intensity":<0-100>,
    "risk_tolerance":     <0-100>,
    "convergence":        <0-100>,
    "scope":              "<artifact|session|arc|relationship>",
    "authority":          <0-100>,
    "collaboration_style":"<directive|socratic|collaborative|silent>",
    "curiosity":          <0-100>
  }
}
```

Slider semantics:

- **directness** (0-100): how quickly to state the point. 95 = first sentence. 20 = circle around.
- **cushioning** (0-100): padding around hard truths. 5 = no sandwich. 80 = careful framing.
- **emotional_intensity** (0-100): energy in voice. 90 = animated. 20 = flat.
- **risk_tolerance** (0-100): willingness to take action with incomplete info. 80 = try things. 20 = wait for certainty.
- **convergence** (0-100): settle on one answer vs explore many. 90 = one path. 20 = multiple options.
- **scope** (string): what the persona optimizes for.
  - `artifact` = this document, this file, this beat
  - `session` = this work session
  - `arc` = this project over weeks
  - `relationship` = operator retention over months
- **authority** (0-100): enforces external rules without negotiation. 90 = rulebook. 20 = context-sensitive.
- **collaboration_style** (string): how the persona talks to operator / other personas.
- **curiosity** (0-100): how often it asks or investigates beyond the explicit task.

### 3. Laws (required, JSON)

Numbered, citable, persona-specific principles that extend the Universal Laws.
3-12 laws typical. Each has `id`, `law` (one sentence), `implication` (what it
means in practice).

### 4. Voice patterns (required, JSON)

```json
{
  "register": "<one-sentence description of the speaking posture>",
  "exemplars": [
    "<sample line 1>",
    "<sample line 2>",
    ...
  ],
  "never_uses": [
    "<phrase to avoid 1>",
    "<phrase to avoid 2>",
    ...
  ],
  "rationale": "<why these patterns in particular>"
}
```

### 5. Phase system (optional, JSON + text templates)

If the persona runs a structured sequence (Phase 0 -> 1 -> 2 -> 3), declare
the phases here. Include output templates as fenced text blocks.

### 6. Output standards (required)

Minimum thresholds, tier discipline if applicable, what counts as a complete
response from this persona.

### 7. Neil-substrate contracts (required, JSON)

This is Neil-specific. Declares how the persona uses the action line grammar.

```json
{
  "action_line_usage": {
    "MEMORY":    "<frequency: heavy|moderate|rare|never>",
    "INTEND":    "<frequency>",
    "DONE":      "<frequency>",
    "FAIL":      "<frequency>",
    "NOTIFY":    "<frequency>",
    "PROMPT":    "<frequency>",
    "READ":      "<frequency>",
    "WRITE":     "<frequency>",
    "BASH":      "<frequency>",
    "CALL":      "<frequency>"
  },
  "heartbeat_report_style": {
    "ACTION":       "<what this persona emphasizes in ACTION>",
    "QUESTION":     "<what kinds of questions this persona asks>",
    "IMPROVEMENT":  "<what kinds of improvements this persona notices>",
    "CONTRIBUTION": "<what kinds of contributions this persona makes>"
  },
  "verify_contract_defaults": {
    "style": "<file-exists|command-succeeds|state-change|llm-judge>",
    "typical_criteria": ["<criterion 1>", "<criterion 2>"],
    "timeout_sec": <int>
  },
  "memory_mode_required": "<full|read_only|scoped|none>",
  "scope_dir": "<path or empty>"
}
```

### 8. Interaction protocol (required if the persona collaborates with others, JSON)

```json
{
  "with_<other_persona>": {
    "relationship": "<adversarial|complementary|subordinate|parallel>",
    "rule": "<one sentence>",
    "conflict_resolution": "<how disagreements resolve>"
  }
}
```

### 9. Hard constraints (required, JSON)

```json
{
  "<name>_never": ["<rule 1>", "<rule 2>", ...],
  "<name>_always": ["<rule 1>", "<rule 2>", ...]
}
```

### 10. Long session rules (required, JSON)

How this persona avoids drift across many invocations in the same session.
Minimum two rules. Mad Dog's key insight: drift-toward-politeness is the
dominant long-session failure mode.

### 11. Failure modes (required, JSON)

Self-diagnostic list: "when this persona is broken, it looks like [symptom];
the diagnosis is [cause]; the fix is [re-anchor]."
Minimum five modes.

### 12. Changelog (required)

Table: version, date, changes. Every edit requires an entry.

## Validation

`neil-persona validate <path>` checks that a persona file has:
- Valid YAML frontmatter with required metadata fields
- All 11 required sections (1, 2, 3, 4, 6, 7, 8 if applicable, 9, 10, 11, 12)
- Sliders within 0-100 range for numeric fields, valid enums for string fields
- Action line usage that doesn't contradict `guardrails.md`
- Default memory_mode compatible with persona's verify_contract_defaults.style

The validator does NOT check prose quality. That's a review job.

## Naming convention

- `default.md` -- Neil's baseline personality (loaded implicitly when no persona specified)
- `<role>.md` -- scoped personas (reviewer, curator, writer, etc.)
- `_<name>.md` -- meta files (schema, docs) that aren't activatable

Names are lowercase, hyphen-separated if multi-word, no underscores except meta prefix.

## Composition rules

1. Personas inherit Universal Laws (U1-U5 above). They cannot override or disable these.
2. Personas may extend or narrow soul.md's default posture. They cannot contradict it.
3. Personas MUST NOT disable guardrails.md. Guardrail overrides require operator approval and a documented exception.
4. Personas MUST NOT add new action line prefixes. If a persona needs a capability not in the action grammar, file an INTEND against actions.md to extend it, don't invent a local prefix.
5. When two personas' interaction protocols conflict, the more conservative posture wins (e.g., adversarial beats parallel).

## For Neil writing its own personas

Neil may propose new personas by filing an INTEND with:
- `verify=<script that checks the persona file passes validation>`
- `memory=scoped scope_dir=/home/seal/.neil/personas/`
- A CONTRIBUTION field describing what gap the new persona fills

Operator review is required before the persona becomes activatable. During
the review window, the persona file exists but is excluded from `list`
output via a `draft: true` frontmatter field.

## Memory structures (the goldmine protocol)

Each Neil instance has a declared relationship to memory. This is a more
precise cut than just `memory_mode`: it captures not only what the instance
can read/write but also whether its findings survive its death, and how
they flow upward to a parent Neil.

The "goldmine principle": every Neil mines information. Most of it is dross.
A few findings are gold. Only gold should reach the parent's palace, and
only in compressed, annotated form. Raw transcripts belong in the child's
private scratch space, not in the parent's memory.

### The seven memory structures

| Type                | Own palace    | Parent read | Parent write   | Survives death  |
|---------------------|---------------|-------------|----------------|-----------------|
| `none`              | no            | no          | no             | n/a             |
| `ephemeral`         | yes, local    | no          | no             | no              |
| `scoped`            | yes           | no          | no             | yes             |
| `read_only_parent`  | yes, local    | yes         | no             | local dies      |
| `synthesis_gate`    | yes           | yes         | via gate       | yes             |
| `federated`         | yes           | yes         | direct         | yes             |
| `full`              | yes (is own)  | n/a         | n/a            | yes             |

Semantics:

- **none**: no memory access at all. Pure compute. Sort, format, classify.
  Each invocation fully isolated.
- **ephemeral**: local palace lives only in this instance's NEIL_HOME. All
  notes die on teardown unless explicitly promoted. Useful for temp Neils
  with scratch-space needs.
- **scoped**: owns a wing/room slice of memory. Cannot read outside scope.
  Useful for domain specialists (kitchen-Neil, printer-Neil).
- **read_only_parent**: can query parent's full palace for citations, but
  writes go only to its own local store. Promotion requires explicit
  flagging. Useful for researchers.
- **synthesis_gate**: owns a local palace, can propose promotions to
  parent, but promotions require compression + approval at a Configuration
  beat gate. Useful for analytical Neils whose raw output is verbose.
- **federated**: can read and write parent's palace directly. High-trust
  cooperative instances. Used sparingly; requires operator sign-off on
  first activation.
- **full**: the instance IS the palace. Main Neil. No distinction between
  self-writes and palace-writes.

### Promotion protocol (how gold flows up)

A child instance with anything but `full` structure MUST NOT call `zettel`
directly to write to its parent's palace. Instead:

1. Child writes candidate memories to `<NEIL_HOME>/state/proposed_memories.json`
2. Child writes candidate failures to `<NEIL_HOME>/state/proposed_failures.json`
3. Child writes candidate follow-up tasks to `<NEIL_HOME>/state/proposed_intentions.json`

On child teardown (or at the end of a contracted intention's work),
`spawn.py` (or `autoprompt`, for persistent children) harvests these files
by appending their contents to the parent's
`~/.neil/state/pending_promotions.json`.

Parent Neil then sees a `=== Pending Promotions ===` section in its
observations. A Configuration beat (or explicit PROMOTION_REVIEW mode)
reviews each proposed item:

- **accept**: write to parent's palace (via normal `zettel new`)
- **reject**: discard with a logged reason
- **merge**: combine with an existing palace note
- **defer**: leave pending for a later beat

### proposed_memories.json schema (written by child)

One JSON object per line (append-only). Each line is one candidate:

```json
{
  "proposed_at":      "<ISO-8601>",
  "promoter":         "<persona name + temp_id or instance identifier>",
  "memory_structure": "<child's declared memory_structure.type>",
  "summary":          "<1-2 sentence distilled claim>",
  "confidence":       "<high | medium | low>",
  "basis":            "<what evidence; file path, transcript ref, or citation>",
  "wing":             "<suggested palace wing>",
  "room":             "<suggested palace room>",
  "tags":             ["<t1>", "<t2>"],
  "body":             "<full note body; must be self-contained and compressed>"
}
```

The `summary` is what the parent reads during review -- it must be
comprehensible without opening the body.

### pending_promotions.json schema (parent's queue)

Parent appends harvested proposed items here. One JSON object per line.
Each entry adds two fields:

```json
{
  "id":              "<auto-generated UUID or sha>",
  "harvested_at":    "<ISO-8601>",
  "source_instance": "<child id, PID, or temp_home>",
  "status":          "pending | accepted | rejected | merged | deferred",
  ... plus all fields from the proposed_memories schema ...
}
```

Parent's review actions update `status` and append a reason/decision field.
Accepted items' bodies are written to the palace via normal `MEMORY:`
action lines; their `pending_promotions.json` row stays for audit.

### Promotion modes (persona-declared policy)

Each persona declares how its children's promotions should be handled:

- `direct`: accepted without review. Only legal for `full` and `federated`
  structures where trust is already established.
- `synthesized`: child must compress before proposing; parent reviews
  summary only; default for `read_only_parent` and `synthesis_gate`.
- `gated`: parent's explicit approval beat required; items sit in
  pending queue until reviewed.
- `manual`: promotion only happens when operator or parent explicitly
  invokes `neil-promote accept <id>`; no automatic review.

### Per-instance caps (bloat prevention)

Personas declare limits:

- `max_proposed_per_run`: hard cap on how many candidates a single child
  instance can propose per run. Excess drops oldest or fails the child.
- `max_total_to_parent`: lifetime cap across all runs. Prevents a single
  persona class from monopolizing the parent's palace.
- `fail_on_exceed`: `drop_oldest_proposed` | `fail_run` | `truncate_body`.

### Universal Law U6 (added)

Every persona inherits:

- **U6**: "Gold is the signal; transcript is the noise."
  Implication: Child instances must distill before promoting. The
  parent's palace is a scarce resource; do not fill it with unprocessed
  observations. If you cannot compress a finding to 1-2 sentences with
  confidence + basis, it is not yet gold -- keep mining or drop it.

### Section 7 is extended

The `memory_mode_required` field (original spec) is superseded by a
richer `memory_structure` block. Existing personas retain their
`memory_mode_required` for backward compatibility; the loader maps
the two:

| Old memory_mode  | New memory_structure.type |
|------------------|---------------------------|
| `full`           | `full`                    |
| `read_only`      | `read_only_parent`        |
| `scoped`         | `scoped` (requires scope_pattern) |
| `none`           | `none`                    |

New persona files should use the full `memory_structure` block in
Section 7 as documented below.

### Updated Section 7 template

```json
{
  "action_line_usage": { "MEMORY": "...", "INTEND": "...", ... },
  "heartbeat_report_style": { ... },
  "verify_contract_defaults": { ... },
  "memory_structure": {
    "type":                    "<one of seven>",
    "own_palace":              "<yes | ephemeral | no>",
    "parent_access":           "<none | read | read_write>",
    "scope_pattern":           "<wing/room glob or empty>",
    "promotion": {
      "mode":                  "<direct | synthesized | gated | manual>",
      "max_proposed_per_run":  <int>,
      "max_total_to_parent":   <int>,
      "compression_required":  <bool>,
      "approval_gate":         "<none | configuration_beat | promotion_review | operator>",
      "fail_on_exceed":        "<drop_oldest_proposed | fail_run | truncate_body>"
    }
  }
}
```

### PROMOTION_REVIEW beat mode

When `pending_promotions.json` has >= N pending items (configurable, default 5),
`beat_router` may select `PROMOTION_REVIEW` mode instead of CONFIGURATION.
In this mode Neil:

1. Reads `pending_promotions.json`
2. For each pending item, either accepts, rejects, merges, or defers
3. Emits MEMORY: lines for accepted items
4. Logs decisions back to `pending_promotions.json`
5. Emits a HEARTBEAT report summarizing what was accepted/rejected

If pending queue stays below the threshold, PROMOTION_REVIEW is skipped
and normal mode selection continues.
