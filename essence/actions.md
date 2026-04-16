# Action Formats

Structured output lines parsed by autoPrompter.

## READ: -- read a file
````
READ: /path/to/file
````
Returns file content in the next ReAct turn. Max 50KB.

## WRITE: -- write/create a file
````
WRITE: path=/path/to/file
\```
file content here
\```
````
Overwrites or creates the file with the content in the code block.

## BASH: -- run a shell command
````
BASH: ls -la ~/.neil/memory/palace/notes/
BASH: grep -r "pattern" ~/.neil/essence/
BASH: python3 script.py
````
Runs via sh, captures stdout+stderr, returns in next ReAct turn. 60s timeout.

## MEMORY: -- store knowledge
````
MEMORY: wing=<domain> room=<topic> tags=<t1,t2> | <what to remember>
````

## CALL: -- API call (ReAct loop, max turns from config)
````
CALL: service=<name> action=<action> [param=value ...]
````

## NOTIFY: -- send outbound message (fire-and-forget)
````
NOTIFY: channel=<terminal|file|email|slack> [param=value ...] | <message>
````

## PROMPT: -- self-prompt (max 1 per cycle)
````
PROMPT: <next task or question>
````

## INTEND: -- defer a task for later
````
INTEND: priority=<low|medium|high> [after=<30m|2h|1d>] [tag=<label>] | <what to do>
````

## DONE: -- complete an intention
````
DONE: <keyword from intention description>
````

## FAIL: -- log a failure for review
````
FAIL: source=<component> severity=<low|medium|high|critical> [context=<where>] | <what went wrong>
````

## HEARTBEAT: -- log beat status
````
HEARTBEAT: status=<ok|acted|error>
ACTION: <what you did>
QUESTION: <a question you have>
IMPROVEMENT: <small improvement>
CONTRIBUTION: <larger creative thought>
````

## Rules
- READ before WRITE. Know what you are changing.
- BASH for inspection and testing. WRITE for file changes.
- One MEMORY per fact. Always assign wing and room.
- Only CALL registered services. NOTIFY for fire-and-forget.
- PROMPT only for genuine follow-up. INTEND for deferred work.
- Always FAIL when something goes wrong.
- Check memory before API calls. Check lessons.md before debugging.

## Workflow example

````
READ: /home/seal/.neil/self_check.sh
````
(autoPrompter returns file content)
````
The check on line 15 is wrong. Fixing it.
WRITE: path=/home/seal/.neil/self_check.sh
\```sh
#!/bin/bash
# ... corrected content ...
\```
BASH: bash /home/seal/.neil/self_check.sh
````
(autoPrompter returns script output)
````
All checks pass.
MEMORY: wing=openclaw room=status | Fixed self_check.sh
HEARTBEAT: status=acted
ACTION: Fixed self_check.sh
...
````

## See something (visual capture via CALL)

````
CALL: service=vision action=look
CALL: service=vision action=inbox
````
