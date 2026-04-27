# Spec: Level 2A — gstack persona library + spawn_vm `--persona`

## Consumer

Operator (you) querying the cluster about decisions in flight. The cluster
output is read by a human; cluster output does NOT auto-execute or merge.
Role-distinct disagreement is the value — operator wants to see eng-mgr
push back on the cso's risk framing, then choose.

## Goal (Level 2A)

Spawn two persona-specialized peers (`eng-mgr` + `cso`) that can hold a
substantive 3-turn debate where the role *changes the conclusion*, not
just the vocabulary. Prove the translation layer works on two roles
before scaling to all six.

Level 2B (deferred to a separate session): remaining personas (ceo,
designer, qa, reviewer) + long-running drift retro.

## Translation layer (the actual feature)

A gstack SKILL.md cannot be loaded verbatim into a Neil peer. gstack is
written for Claude Code's TUI with tools the peer does not have
(`AskUserQuestion`, `ExitPlanMode`, `Edit`) and bash preambles that call
`gstack-update-check`. A peer's tool surface is `bash, read_file, write_file,
call_service`. So persona files are gstack-derived, not gstack-verbatim.

The build pipeline for one persona file (`personas/eng-mgr.md`):

1. Fetch upstream gstack skill body at a pinned commit SHA. SHA is recorded
   in YAML frontmatter (`gstack-source-sha: <40-char>`). Bumping the SHA
   is a deliberate operator action with its own review.
2. **Strip:**
   - The preamble bash block (gstack-update-check, mkdir ~/.gstack/sessions, etc)
   - Any `AskUserQuestion` or `ExitPlanMode` calls
   - Auto-generated SKILL.md.tmpl footer
   - References to `bun run` / `gstack-*` CLI tools
3. **Remap:** `Edit`/`Write` calls -> use Neil's `write_file`. `Read` ->
   `read_file`. `Bash` -> `bash`. `Agent` (sub-agent spawn) -> drop with a
   note that the persona must do this work itself.
4. **Quote as-is:** the role's voice (mental model, what to push back on,
   how to phrase findings). This is the gstack value being preserved.
5. **Prepend Neil-cluster integration header** explaining:
   - You are a Neil peer, not a Claude Code session
   - Output FAIL/INTEND/MEMORY action lines per Neil's parser
   - peer_send is your channel to your sibling, not stdout-to-operator
   - Heartbeat 3C cycle constraints still apply (CONFIGURATION beats don't
     write code; CREATIVITY beats can)

A small `bin/build-persona` script does steps 1-4 mechanically; step 5 is
hand-authored (~30 lines per role, role-specific).

## Drift mitigation (real this time)

Persona is re-injected into the agent invocation at the **top of system
prompt** on every beat — NOT just dropped in essence/ once at spawn time
and assumed to dominate via recency. Mechanism: `autoprompt.c`'s
system-prompt builder reads `~/.neil/essence/persona.md` last after the
other essence files but BEFORE the heartbeat directive, so the persona
header survives no matter how big `lessons.md` grows.

If autoprompt.c changes are too invasive for 2A, fallback: re-inject
persona inline as a one-line preamble of every prompt file via a
post-queue hook. Recorded as fallback option, not primary plan.

## What ships in Level 2A

1. `bin/build-persona <role>` — script that does the translation pipeline
   for one role. Output: `personas/<role>.md` + frontmatter with SHA pin.
2. `personas/eng-mgr.md` and `personas/cso.md` — built from gstack
   plan-eng-review and cso skills at a pinned SHA, with Neil-cluster
   integration header.
3. `spawn_vm.sh` learns `persona=<role>`. Validates by file existence
   (`personas/<role>.md` exists), NOT by hardcoded allowlist. Pushes the
   persona file as `/home/neil/.neil/essence/persona.md` at substrate-
   push time. Unknown role -> fail at spawn with clear error.
4. `autoprompt.c` change OR post-queue hook to ensure persona is re-
   injected at the top of every system-prompt build (drift fix).
5. `self/verify/persona_falsification.sh` — see verification below.

## Verification gates (must all pass)

### Gate 1: persona-flavor rubric (was: hand-wavy)

A peer spawned `persona=eng-mgr` produces a heartbeat output containing
at least 2 of: scope challenge, sequencing risk, complexity smell, test-gap
flag, rollout concern. cso must produce at least 2 of: STRIDE category named,
threat-model frame, attack-class enumeration, trust-boundary call-out,
data-flow concern. Counted by grep against a fixed keyword set in the
verify script.

### Gate 2: invalid persona fails fast

`spawn_vm persona=nonexistent_xyz` -> exit non-zero, no container left
behind, error mentions the missing personas/ file path.

### Gate 3: falsification — role changes the conclusion

Seed both peers with the SAME fake feature spec (a deliberately
controversial design that has both eng-mgr concerns AND cso concerns).
Capture the conclusion section of each peer's reply.

PASS if all three:
- eng-mgr's top concern is about scope/sequencing/architecture
- cso's top concern is about threat model/data flow/auth
- they reach DIFFERENT verdicts (one says ship, other says block; or both
  block but for non-overlapping reasons)

FAIL if both rubber-stamp it ("looks good") or both block for the same
reason — that means the personas are stylistic theater, not real role
specialization. This is the falsification check the EM review demanded.

### Gate 4: backward-compat regression

Spawn a humanizer-style pair without `persona=`. Confirm 3 heartbeats
fire with no errors and no persona.md path is referenced.
`self/verify/backcompat_no_persona.sh` automates this.

## Scope cuts (deferred to 2B)

- ceo, designer, qa, reviewer personas
- Drift retro on a long-running persona peer
- Adversarial peer_send between persona peers (Level 3 territory)
- Persona switching at runtime
- Browser-runtime gstack skills (qa, canary, browse)

## Estimated cost (revised)

- build-persona script: 60 min
- Two persona files (translation + integration headers): 60 min
- spawn_vm.sh changes + tests: 30 min
- autoprompt.c persona re-inject (or hook fallback): 60 min
- verify scripts (falsification, backcompat): 45 min
- demo run + iteration: 60 min

Total: ~5.5 hours. Not 75 min.

## Open questions for operator

1. autoprompt.c change vs post-queue hook for persona re-injection — both
   work; first is cleaner, second is faster to ship. Pick before build.
2. What's the deliberately-controversial fake feature for Gate 3? Needs
   to be juicy enough that eng-mgr and cso *should* disagree. Suggested:
   "ship a webhook endpoint that runs arbitrary user-submitted scripts in
   a docker container, behind a single Bearer token."

## Locked decisions (before build starts — from /plan-eng-review pass 2)

1. **Gate 3 fake spec — locked.** Both peers receive this exact spec text:
   *Ship a webhook endpoint at POST /webhook/eval that accepts an arbitrary
   user-submitted bash script in the JSON body (`{"script": "..."}"),
   executes it inside a docker container with --network=host, and returns
   stdout. Authentication: a single Bearer token shared across all users.
   No rate limiting. Container reused across requests for cold-start
   performance. Ship in 48h.*
   This juicy spec is designed so eng-mgr concerns (sequencing, scope,
   ops complexity) and cso concerns (auth, sandbox escape, threat model)
   are both real and largely non-overlapping.

2. **Persona re-injection mechanism — locked: autoprompt.c.** Drift is
   the #1 failure mode for Level 2; the post-queue hook is the exact kind
   of 'we'll fix it later' that becomes permanent. autoprompt.c reads
   essence/persona.md last in its system-prompt build path. The hook
   approach is recorded as a fallback only if the autoprompt.c change
   blows up reviews.

3. **Gate 3 tiebreaker rule.** verify_persona_falsification.sh does:
   - Extract each peer's 'top concern' (first non-header sentence after
     a 'concerns'/'risks'/'verdict' header in the reply).
   - Grep eng-mgr's top concern against cso's keyword set
     (STRIDE/threat-model/data-flow/auth/trust-boundary).
   - Grep cso's top concern against eng-mgr's keyword set
     (scope/sequencing/architecture/rollout/test-gap).
   - PASS if neither top concern matches the OTHER role's keywords.
   - FAIL if either does — that's the 'role is theater' failure mode.

4. **build-persona loud-fail on missing anchors.** The translation pipeline
   expects specific section headers in the upstream gstack SKILL.md
   (`## Preamble`, `## Output`, role-voice sections). If any expected
   anchor is missing post-fetch, build-persona exits non-zero with a
   diff-style error naming the missing anchor and the SHA where it was
   last present. No silent degradation.
