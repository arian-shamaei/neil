---
service: gstack
phase: 1
category: integration
---

# gstack — call gstack skills as Neil services

Bridge to Garry Tan's gstack collection. Neil beats (or peers) can fire
`CALL: service=gstack action=<skill> params="context=<text>"` to invoke a
gstack skill prompt against a Neil-managed workload — e.g. ask /retro to
reflect on the last hour of cluster_activity.jsonl, or /plan-eng-review
to evaluate a proposed change.

Skill prompts live at `~/.neil/skills/gstack/<skill>/SKILL.md` and were
fetched from https://github.com/garrytan/gstack. The handler reads the
matching SKILL.md, passes it as the system prompt to a one-shot
neil_agent.py invocation, and feeds the caller's `context` as the user
prompt. The agent's output is returned verbatim.

This is **Level 1 integration** — pure prompt reuse, no gstack runtime,
no Bun, no Claude Code slash-command harness. Works for prompt-only
skills (retro, plan-ceo-review, plan-eng-review, design-review,
investigate, learn, cso). Does NOT work for skills that need TypeScript
runtimes or browser automation (browse, qa, canary).

## Call surface

```
CALL: service=gstack action=<skill> context=<verbatim text the skill should chew on>
```

| Param | Required | Purpose |
|---|---|---|
| action  | yes (as NEIL_ACTION) | gstack skill name; must be a directory under `~/.neil/skills/gstack/` |
| context | yes | Free-form text passed to the skill as user prompt. The skill's SKILL.md explains what kind of input it expects. |

## Effects

1. Reads `~/.neil/skills/gstack/<action>/SKILL.md` as system prompt
2. Runs `neil_agent.py --system-prompt <skill-body> -p <context>` (NEIL_MAX_TURNS=15)
3. Echoes the agent's output as the CALL result
4. Logs to cluster_activity.jsonl as event=gstack_invoke

## Preconditions

- `~/.neil/skills/gstack/<action>/SKILL.md` exists
- Claude credentials are fresh (any caller can verify via neil-creds-sync)

## Currently installed skills

```
$(ls ~/.neil/skills/gstack/ 2>/dev/null)
```

## Adding more skills

```
mkdir -p ~/.neil/skills/gstack/<name>
curl -sL https://raw.githubusercontent.com/garrytan/gstack/main/<name>/SKILL.md     -o ~/.neil/skills/gstack/<name>/SKILL.md
```

## Example — ask /retro to reflect on recent cluster activity

```
CALL: service=gstack action=retro context="Reflect on the last 24 hours of cluster_activity.jsonl. What patterns? What's working? What needs attention?"
```
