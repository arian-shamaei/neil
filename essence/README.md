# Essence

L1 cache for Neil. Any AI reads this directory and instantly understands
everything: who it is, what it's doing, how the system works, and what
actions are available.

Loaded as `--system-prompt` on every Claude invocation by autoPrompter.
All .md files in this directory are concatenated and injected.

## Files

| File | Purpose |
|------|---------|
| identity.md | Name, host, operator, basic behavior rules |
| soul.md | Non-negotiable behavioral core: honesty, competence, boundaries |
| mission.md | Current objectives and constraints |
| overview.md | System architecture, components, data flow, key paths |
| actions.md | Output formats: MEMORY, CALL, PROMPT |
| heartbeat.md | Autonomous loop template: observe, reason, act, log |

## Rules

- Keep files concise. Every token here is loaded on every invocation.
- Update mission.md when objectives change.
- soul.md is the behavioral contract. Change with care and notify operator.
- Don't put ephemeral state here. Use heartbeat_log.json or memory for that.
