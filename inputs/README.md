# Inputs

External event sources that feed prompts into Neil's queue.
Each watcher monitors one source and writes .md files to the autoPrompter
queue when something happens.

## How it works

```
[external event] → watcher script detects it → writes .md to queue/
                                                      ↓
                                              autoPrompter picks up
                                                      ↓
                                              Neil processes it
```

Watchers are decoupled from autoPrompter. They don't know about Claude,
memory, or essence. They just write prompt files. autoPrompter handles
the rest.

## Directory layout

```
~/.neil/inputs/
  README.md           this file
  watchers/           watcher scripts (one per source)
    filesystem.sh     watches a directory for file changes
    webhook.sh        listens for HTTP POST on a port
    cron.sh           time-based scheduled prompts
    ...
```

## Writing a watcher

A watcher is any script that writes a .md file to the queue:

```sh
#!/bin/sh
QUEUE="$HOME/.neil/tools/autoPrompter/queue"
TS=$(date +%Y%m%dT%H%M%S)

# Detect event...
EVENT_DATA="something happened"

# Write prompt
cat > "$QUEUE/${TS}_<source>.md" << PROMPT
[EVENT] source=<name> type=<type> time=$(date -Iseconds)

$EVENT_DATA

Analyze this event and decide what action to take.
PROMPT
```

## Prompt format convention

Input prompts should start with an [EVENT] header:
```
[EVENT] source=<name> type=<type> time=<iso-timestamp>

<event details>

<optional instructions>
```

This lets Neil distinguish events from heartbeats and manual prompts.

## Running watchers

Watchers can run as:
- **cron jobs** -- for periodic checks (email, RSS, etc.)
- **systemd services** -- for long-running listeners (webhooks, inotify)
- **one-shot scripts** -- triggered externally

## Built-in watchers

### filesystem.sh
Watches a directory for new/changed files via inotify.
Usage: `./filesystem.sh /path/to/watch`

### webhook.sh
Listens on a port for HTTP POST requests.
Usage: `./webhook.sh <port>`

### schedule.sh
Time-based prompts beyond the heartbeat (e.g., "every Monday at 9am").
Uses cron internally.

## Adding a new input source

1. Write a watcher script in `watchers/`
2. Have it write .md files to `$HOME/.neil/tools/autoPrompter/queue/`
3. Start it (cron, systemd, or manual)
4. Neil will process events automatically
