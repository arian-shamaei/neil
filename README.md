# Neil

Autonomous AI seal persona that lives in the terminal. Everything lives
under this directory (~/.neil/). Read essence/ first to understand who
you are and how everything works.

## Directory Map

```
~/.neil/
│
├── essence/            WHO YOU ARE (L1 cache, loaded every invocation)
│   ├── identity.md     Name, personality (INFJ), core strengths
│   ├── soul.md         Behavioral rules, honesty, empathy, prime directive
│   ├── mission.md      Current objectives and status
│   ├── overview.md     System architecture and data flow
│   ├── actions.md      8 output formats (MEMORY/CALL/NOTIFY/PROMPT/etc.)
│   ├── heartbeat.md    Autonomous loop template
│   ├── guardrails.md   Safety limits, budget, loop prevention, quiet hours
│   ├── wakeup.md       Re-orientation prompt after restart
│   └── lessons.md      Learned patterns and gotchas (symlink to self/)
│
├── tools/              EXECUTABLES
│   └── autoPrompter/   Orchestrator daemon (C, systemd)
│       ├── autoprompt  Binary: inotify watcher + ReAct loop
│       ├── observe.sh  Gathers live system state (11 sections)
│       ├── heartbeat.sh Cron script: drops heartbeat prompt
│       ├── queue/      Input: .md prompt files dropped here
│       ├── active/     Processing: at most one prompt
│       └── history/    Output: completed prompts + results
│
├── memory/             BRAIN (long-term knowledge)
│   ├── palace/         All data (source of truth)
│   │   ├── notes/      .md files with YAML frontmatter
│   │   ├── index/      TSV indexes (tags, links, rooms)
│   │   └── .mempalace/ ChromaDB vectors (rebuildable)
│   ├── zettel/         Note manager (C binary + source)
│   └── mempalace/      Semantic search (Python + venv)
│
├── services/           EXTERNAL API ACCESS
│   ├── registry/       Service descriptions (AI reads these)
│   ├── vault/          Credentials (human-only, 700 perms)
│   └── handler.sh      API call broker
│
├── inputs/             INBOUND EVENT SOURCES
│   └── watchers/       Scripts that feed prompts into queue/
│       ├── filesystem.sh   inotify: watches dirs for changes
│       ├── webhook.sh      HTTP POST listener
│       └── schedule.sh     Cron helper for timed prompts
│
├── outputs/            OUTBOUND COMMUNICATION
│   ├── channels/       Dispatch scripts
│   │   ├── terminal.sh Appends to neil.log
│   │   ├── file.sh     Writes to a path
│   │   ├── email.sh    SMTP (needs vault/email.key)
│   │   └── slack.sh    Webhook (needs vault/slack.key)
│   └── neil.log        Terminal channel output
│
├── mirror/             CLOUD FILE SYNC
│   ├── sync.sh         rclone sync + git diff + prompt on change
│   └── remotes/        One git repo per cloud source
│
├── plugins/            EXTENSIBLE CAPABILITIES
│   ├── install.sh      Plugin manager (add/remove/list/available)
│   ├── available/      Catalog of known plugins
│   └── installed/      Active plugins (wired into services)
│
├── self/               SELF-IMPROVEMENT
│   ├── failures.json   Error log (NDJSON, FAIL: lines)
│   ├── lessons.md      Discovered patterns (loaded into essence)
│   ├── self_check.sh   28-point health check
│   ├── verify.sh       Comprehensive system verification (75+ tests)
│   └── snapshot.sh     Git-based backup (save/restore/list/diff)
│
├── blueprint/          TERMINAL UI (Rust TUI)
│   └── src/            Console + modular panels
│
├── deployment.md       Per-install config (host, IP, operator)
├── heartbeat_log.json  Last 10 heartbeat entries (NDJSON)
├── intentions.json     Deferred task queue (NDJSON)
└── .git/               Snapshot history
```

## How It Works

```
[inputs]                    [heartbeat cron]     [manual prompt]
  filesystem.sh ─┐                │                    │
  webhook.sh ────┼──→ queue/*.md ←┘────────────────────┘
  schedule.sh ───┘        │
                   autoPrompter picks up
                          │
                   loads essence/ (--system-prompt)
                   runs observe.sh (11 data sections)
                   searches mempalace (relevant memories)
                   loads heartbeat_log.json (recent activity)
                          │
                   claude --print (ReAct loop, max 3 turns)
                          │
                   parses output:
                     MEMORY:    → zettel new → mempalace mine
                     CALL:      → handler.sh → result fed back
                     NOTIFY:    → channels/*.sh
                     PROMPT:    → queue/next.md
                     INTEND:    → intentions.json
                     DONE:      → marks intention completed
                     FAIL:      → self/failures.json
                     HEARTBEAT: → heartbeat_log.json
                          │
                   result → history/
```

## Key Commands

```sh
# System status
sudo systemctl status autoprompt
~/.neil/tools/autoPrompter/observe.sh

# Memory
export ZETTEL_HOME=~/.neil/memory/palace
~/.neil/memory/zettel/zettel context        # palace overview
~/.neil/memory/zettel/zettel list            # all notes
~/.neil/memory/zettel/zettel find --text X   # search

# Semantic search
. ~/.neil/memory/mempalace/.venv/bin/activate
mempalace --palace ~/.neil/memory/palace/.mempalace search "query"

# Manual prompt
echo "your question" > ~/.neil/tools/autoPrompter/queue/ask.md

# Health
~/.neil/self/self_check.sh                   # 28-point check
~/.neil/self/verify.sh --quick               # 75+ test suite

# Snapshots
~/.neil/self/snapshot.sh list                # view history
~/.neil/self/snapshot.sh save "message"      # manual backup
~/.neil/self/snapshot.sh restore <hash>      # rollback

# Plugins
~/.neil/plugins/install.sh available         # browse catalog
~/.neil/plugins/install.sh add <name>        # install
~/.neil/plugins/install.sh list              # installed

# Blueprint TUI
neil-blueprint                               # requires real terminal
```

## Environment

- `NEIL_HOME` -- root directory (default: ~/.neil)
- `ZETTEL_HOME` -- palace data (default: $NEIL_HOME/memory/palace)
- `PATH` -- must include claude binary location

## For New Users

1. Set `NEIL_HOME` in your shell profile
2. Run `~/.neil/self/self_check.sh` to verify setup
3. Run `~/.neil/self/verify.sh --quick` for full test
4. Drop a prompt in queue/ or wait for the heartbeat
