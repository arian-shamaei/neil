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

## Show rich content to the user (inline in chat)



These render as rich widgets in the blueprint TUI. Use them to show
the human code, diagrams, tables, and charts inline in conversation.

Alternatively, use standard markdown code fences (```lang ... ```)
which are also rendered as code blocks.

## See something (visual capture via CALL)

Use the vision service to see your surroundings:
```
CALL: service=vision action=look
CALL: service=vision action=pane target=main:0.0
CALL: service=vision action=screenshot
CALL: service=vision action=inbox
CALL: service=vision action=list
```

Images and text captures come back through the ReAct loop.
Users can drop images in vision/inbox/ for you to see.
