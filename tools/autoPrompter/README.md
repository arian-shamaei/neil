# autoPrompter

Event-driven prompt queue for Claude Code. C daemon using Linux inotify.

## How it runs

- **systemd**: `autoprompt.service` (enabled, auto-restart on crash)
- **Working dir**: `~/.neil/tools/autoPrompter/`
- **Heartbeat**: cron fires `heartbeat.sh` every 30 minutes

```sh
sudo systemctl status autoprompt   # check
sudo systemctl restart autoprompt  # restart
journalctl -u autoprompt -f        # live logs
```

## Directory layout

```
autoPrompter/
  autoprompt         compiled binary
  src/autoprompt.c   source
  Makefile
  heartbeat.sh       cron script: drops heartbeat prompt into queue/
  observe.sh         gathers live system state for [OBSERVATIONS]
  queue/             drop .md prompt files here (input)
  active/            prompt currently executing (at most one)
  history/           completed prompts + result files (output)
```

## Prompt processing pipeline

```
1. inotify detects .md in queue/
2. Move to active/ (atomic rename)
3. Load essence/ → --system-prompt
4. Run observe.sh → [OBSERVATIONS]
5. Load heartbeat_log.json → [RECENT ACTIVITY]
6. Search mempalace → [RELEVANT MEMORIES]
7. Invoke claude --print with all context
8. Parse output (ReAct loop, max 3 turns):
   - MEMORY: → zettel new + mempalace mine
   - CALL: → handler.sh → results fed back to Claude
   - PROMPT: → queue/next.md (max 1 per cycle)
   - HEARTBEAT: → heartbeat_log.json
9. Write result to history/
10. Move prompt to history/
```

## ReAct loop

If Claude outputs CALL: lines, autoPrompter:
1. Executes the calls via `~/.neil/services/handler.sh`
2. Re-invokes Claude with [PREVIOUS RESPONSE] + [CALL RESULTS]
3. Repeats up to 3 turns or until no more CALL: lines

## Crash recovery

- If daemon dies mid-prompt: file stays in active/
- On restart: active/ files moved back to queue/ and re-processed
- systemd Restart=always with 5s delay

## Building

```sh
cd ~/.neil/tools/autoPrompter
make              # build
make clean        # remove binary
```

## Result file format

```markdown
# Result: filename.md
- **executed:** timestamp
- **exit_code:** 0
- **status:** success
- **turns:** 2

## Prompt
(original prompt)

## Output
(Claude's response, all turns concatenated)

## Service Calls
(CALL results, if any)
```
