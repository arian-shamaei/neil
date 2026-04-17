# Beat Router

Decides what mode each heartbeat should be before Neil responds.
Enforces the 3C discipline: Configuration -> Characterization -> Creativity.

## Why this exists

Without routing, Neil defaults to "initiative work" every beat with no
pending intentions -- which produced 6 busy-work beats and 4 proposal
cycles in the hour we studied. The 3C method is already in soul.md but
nothing gated it. The router makes the gate real.

## Three modes

### CONFIGURATION
Study the system. Understand what IS. Ground truth only.
- Uses READ: and BASH: to inspect
- Produces MEMORY: notes documenting discoveries
- May produce INTEND: if something actionable emerges
- MUST NOT write code or modify files

### CHARACTERIZATION
Take something discovered. Verify it. Determine if it's a real problem.
- Uses BASH: to test, READ: to cross-reference
- Produces DONE: (works as expected), FAIL: (real problem), or INTEND: (specific work needed)
- Grounds Creativity beats that follow

### CREATIVITY
Execute a pending intention or fix an unresolved failure.
- Uses WRITE: and BASH: to ship code
- Verifies work (build passes, tests pass)
- Produces DONE: (shipped), FAIL: (blocked), or INTEND: (next step)
- Requires upstream evidence (intention or failure) -- not spontaneous

## Decision tree (top-down, first match wins)

```
1. pending INTENDs exist
   -> CREATIVITY, target = oldest pending intention

2. unresolved FAILs exist
   -> CREATIVITY, target = highest-severity failure

3. last_3_beats all CREATIVITY without prior CONFIGURATION
   -> FORCED_CONFIGURATION (anti-proposal-spam gate)
   -> target = rotated study area

4. last beat was CONFIGURATION and emitted findings but no INTEND
   -> CHARACTERIZATION
   -> target = the MEMORY note from last beat

5. default
   -> CONFIGURATION, target = next in study rotation
```

## Output format

`beat_router.sh` appends to observe.sh output:

```
=== Beat Directive ===
mode: <CONFIGURATION | CHARACTERIZATION | CREATIVITY>
reason: <one line explaining why this mode>
target: <specific file/intention/finding to focus on>
required: <what action lines must appear in response>
forbidden: <what Neil must NOT do this beat>
```

## Study rotation

`self/study_rotation.txt` tracks which areas have been studied recently.
Format: `area_name|last_studied_timestamp`.

Areas (rotated in order, oldest first):
- essence (soul, identity, mission, philosophy)
- autoprompter (src/autoprompt.c)
- agent (neil_agent.py, tool definitions)
- blueprint (TUI rendering, panels)
- memory (zettel, mempalace, palace structure)
- services (registry, vault, handlers)
- outputs (channels, recent logs)
- config (config.toml, cron, essence links)

## Success metrics (after 20 beats)

Target (measured by beat_audit.sh):
- Mode distribution: ~33% each (or skewed toward CHAR/CREAT when intentions exist)
- INTENDs created per Configuration beat: > 0.5
- INTEND completion rate: > 50% within 3 beats of creation
- Zero "ungrounded" Creativity beats (all have prior CHAR or pending intention)
- Proposal-to-ship ratio > 0.5

If metrics miss target after 20 beats, revert via config flag.

## Rollback

In `~/.neil/config.toml`:

```
[heartbeat]
mode_routing = false
```

When false, `beat_router.sh` outputs nothing, heartbeat.md falls back
to the pre-router tier logic (backup at `heartbeat.md.bak`).

## Enforcement levels

**v1 (this version):** Advisory. Router outputs directive, template
tells Neil to follow it. No hard enforcement.

**v2 (future):** `validate_beat.sh` runs after each beat, compares
declared mode to actual actions, logs a meta-FAIL if mismatch so Neil
sees it next beat.

## Philosophy

The 3C method (Configuration -> Characterization -> Creativity) is
already documented in `essence/soul.md` as Neil's core methodology.
This router is the mechanical enforcement of that methodology --
nothing new, just making implicit explicit.
