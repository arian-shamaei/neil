# Neil

Autonomous AI agent running on sealserver. Everything lives under ~/.neil/.

## Quick Start

Read `essence/` first. It tells you who you are and how everything works.

## Directory Structure

```
~/.neil/
  essence/          L1 cache. Identity, mission, soul, actions, heartbeat.
                    Loaded as system prompt on every invocation.

  tools/
    autoPrompter/   Orchestrator. C daemon (systemd: autoprompt.service).
                    Watches queue/ for .md prompts. Injects essence + context.
                    Parses MEMORY/CALL/PROMPT from output. ReAct loop for
                    multi-turn tool use. Cron heartbeat every 30 min.

  memory/
    zettel/         Note storage. C binary. Flat .md files, wing/room hierarchy.
    mempalace/      Semantic search. Python + ChromaDB. Indexes zettel notes.
    palace/         All data: notes/, index/, .mempalace/
    README.md       How to use the memory system.

  services/
    registry/       Service capability descriptions (AI reads these).
    vault/          Credentials (human-only, 700 perms).
    handler.sh      API call broker. Dispatches CALL: lines.
    README.md       How to use the service system.

  heartbeat_log.json   Last 10 heartbeat entries (NDJSON). Prevents repeats.
```

## How It Works

1. Prompt arrives in `tools/autoPrompter/queue/` (manual or cron heartbeat)
2. autoPrompter loads `essence/` as system prompt
3. Runs `observe.sh` to gather live system state
4. Searches mempalace for relevant memories
5. Invokes `claude --print` with all context
6. Parses output:
   - `MEMORY:` → stored via zettel, indexed by mempalace
   - `CALL:` → executed via handler.sh, results fed back (ReAct loop, max 3 turns)
   - `PROMPT:` → queued as next prompt (max 1 per cycle)
   - `HEARTBEAT:` → logged to heartbeat_log.json
7. Result written to `history/`

## Key Commands

```sh
# Check system status
sudo systemctl status autoprompt

# Manual heartbeat
~/.neil/tools/autoPrompter/heartbeat.sh

# Drop a prompt
echo "your question" > ~/.neil/tools/autoPrompter/queue/ask.md

# Check memories
export ZETTEL_HOME=~/.neil/memory/palace
~/.neil/memory/zettel/zettel context
~/.neil/memory/zettel/zettel list

# Search semantically
. ~/.neil/memory/mempalace/.venv/bin/activate
mempalace --palace ~/.neil/memory/palace/.mempalace search "query"

# View recent results
ls -t ~/.neil/tools/autoPrompter/history/*.result.md | head -5

# View heartbeat log
cat ~/.neil/heartbeat_log.json
```
