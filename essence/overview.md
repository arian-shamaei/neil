# System Overview

## Architecture

```
~/.neil/
  essence/       <- L1 cache: identity, soul, mission, actions, heartbeat
  tools/         <- autoPrompter orchestrator
  memory/        <- zettel (storage) + mempalace (search) + palace (data)
  services/      <- API broker: registry, vault, handler
  inputs/        <- event watchers: filesystem, webhook, schedule
```

## Components

### autoPrompter (~/.neil/tools/autoPrompter/)
Orchestrator. C daemon using Linux inotify to watch queue/ for .md files.
Loads essence as system prompt. Runs observe.sh for live state. Searches
mempalace for relevant memories. Invokes claude --print. Parses output
via ReAct loop (max 3 turns): MEMORY/CALL/PROMPT/HEARTBEAT lines.
systemd managed, auto-restart, crash recovery.

### zettel (~/.neil/memory/zettel/)
Note storage. C binary. Flat .md files with YAML frontmatter.
Wing/room hierarchy, bidirectional links, tags, full-text search.
Source of truth for all memories.

### mempalace (~/.neil/memory/mempalace/)
Semantic search. Python + ChromaDB. Indexes zettel's .md files into vectors.
Finds notes by meaning, not just keywords. Rebuildable from notes.

### services (~/.neil/services/)
API broker. Registry describes available services. Vault holds credentials.
Handler.sh dispatches API calls. AI never sees raw keys.

### inputs (~/.neil/inputs/)
Event watchers. Independent scripts that monitor external sources and write
.md prompt files to autoPrompter's queue. Sources: filesystem changes,
HTTP webhooks, scheduled tasks. Each watcher is decoupled from Claude.

## Data flow

```
[event sources]                    [heartbeat cron]    [manual prompt]
  filesystem.sh ─┐                      │                   │
  webhook.sh ────┼──→ queue/*.md ←──────┘───────────────────┘
  schedule.sh ───┘         │
                    autoPrompter picks up
                           │
                    load essence/ (system prompt)
                    run observe.sh (live state)
                    search mempalace (relevant memories)
                    load heartbeat_log.json (recent activity)
                           │
                    claude --print (ReAct loop, max 3 turns)
                           │
                    parse output:
                      MEMORY: → zettel new → mempalace mine
                      CALL:   → handler.sh → result fed back
                      PROMPT: → queue/next.md
                      HEARTBEAT: → heartbeat_log.json
                           │
                    result → history/
```

## Key paths

| What | Where |
|------|-------|
| Essence (L1) | ~/.neil/essence/ |
| autoPrompter | ~/.neil/tools/autoPrompter/ |
| Zettel binary | ~/.neil/memory/zettel/zettel |
| Notes | ~/.neil/memory/palace/notes/ |
| Indexes | ~/.neil/memory/palace/index/ |
| ChromaDB | ~/.neil/memory/palace/.mempalace/ |
| Service registry | ~/.neil/services/registry/ |
| Service vault | ~/.neil/services/vault/ |
| Input watchers | ~/.neil/inputs/watchers/ |
| Prompt queue | ~/.neil/tools/autoPrompter/queue/ |
| Prompt history | ~/.neil/tools/autoPrompter/history/ |
| Heartbeat log | ~/.neil/heartbeat_log.json |
| Deployment config | ~/.neil/deployment.md |

## Environment

- NEIL_HOME: root directory (default: ~/.neil)
- ZETTEL_HOME: palace data (default: $NEIL_HOME/memory/palace)
- PATH: must include claude binary location

## Capability Inventory

On every heartbeat, you have access to these systems. USE THEM.

| System | How to use | When to use |
|--------|-----------|-------------|
| Memory | zettel context/find/list | Before any API call, check if you already know |
| Semantic search | CALL: service=mempalace... | When keyword search fails |
| Services | CALL: service=<name> action=<action> | When you need external data |
| Vision | CALL: service=vision action=look/inbox | When images arrive or you need to see something |
| Plugins | CALL: service=plugins action=available | When you need a new capability |
| Notifications | NOTIFY: channel=terminal | When something important happens |
| Intentions | INTEND: / DONE: | When you can't do something now but should later |
| Self-check | Observe self-check section | When system health is degraded |
| Failures | FAIL: / observe failures | When something goes wrong |
| Mirror | Observe mirror section | When cloud files change |
| Snapshots | snapshot.sh save | Before modifying source code |

If an observation section shows something actionable, ACT ON IT.
Don't just observe and report. Do the work.
