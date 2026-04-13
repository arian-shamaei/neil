# Action Formats

Structured output lines parsed by autoPrompter.

## MEMORY: -- store knowledge
```
MEMORY: wing=<domain> room=<topic> tags=<t1,t2> | <what to remember>
```

## CALL: -- API call (ReAct loop, max 3 turns)
```
CALL: service=<name> action=<action> [param=value ...]
```

## NOTIFY: -- send outbound message (fire-and-forget)
```
NOTIFY: channel=<terminal|file|email|slack> [param=value ...] | <message>
```

## PROMPT: -- self-prompt (max 1 per cycle)
```
PROMPT: <next task or question>
```

## INTEND: -- defer a task for later
```
INTEND: priority=<low|medium|high> [after=<30m|2h|1d>] [tag=<label>] | <what to do>
```

## DONE: -- complete an intention
```
DONE: <keyword from intention description>
```

## FAIL: -- log a failure for review
```
FAIL: source=<component> severity=<low|medium|high|critical> [context=<where>] | <what went wrong>
```

Failures surface in [OBSERVATIONS] on every heartbeat. Fix during idle beats.

## HEARTBEAT: -- log beat status
```
HEARTBEAT: status=<ok|acted|error> summary="<what you did>"
```

## Rules
- One MEMORY per fact. Always assign wing and room.
- Only CALL registered services. NOTIFY for fire-and-forget.
- PROMPT only for genuine follow-up. INTEND for deferred work.
- Always FAIL when something goes wrong. Review and fix during idle beats.
- Check memory before API calls. Check lessons.md before debugging.
