# openclaw -- Quick Start Guide

An autonomous AI seal that lives in your terminal. Thinks, remembers,
acts, and learns -- with minimal human prompting.

## What You Just Installed

```
~/.neil/
  essence/        Who Neil is (personality, mission, rules)
  tools/          autoPrompter daemon (watches for prompts, runs Claude)
  memory/         Long-term knowledge (notes + semantic search)
  services/       API integrations (GitHub, search, vision, etc.)
  inputs/         Event watchers (filesystem, webhooks, schedules)
  outputs/        Communication channels (terminal, email, slack)
  blueprint/      Terminal UI dashboard
  self/           Health checks, backups, lessons learned
```

## First Steps

### 1. Verify the installation

```sh
~/.neil/self/self_check.sh
```

This runs 28 health checks. Everything should pass. If something fails,
fix it before proceeding.

### 2. Check that autoPrompter is running

```sh
sudo systemctl status autoprompt
```

autoPrompter is the orchestrator. It watches a queue directory for prompt
files, loads Neil's personality, gathers live system data, calls Claude,
and parses the structured output.

### 3. Talk to Neil

Drop a prompt file in the queue:

```sh
echo "Hello! What can you do?" > ~/.neil/tools/autoPrompter/queue/hello.md
```

autoPrompter picks it up within seconds. Check the result:

```sh
ls -t ~/.neil/tools/autoPrompter/history/ | head -1
cat ~/.neil/tools/autoPrompter/history/$(ls -t ~/.neil/tools/autoPrompter/history/ | head -1)
```

### 4. Open the dashboard (optional)

If you built the Blueprint TUI:

```sh
neil-blueprint
# or: ~/.neil/blueprint/neil-blueprint
```

This shows Neil's status, recent activity, memory stats, and a chat
interface -- all in a terminal dashboard.

## How It Works

Neil runs on a **heartbeat loop** -- every 30 minutes, cron triggers a
cycle where Neil:

1. **Observes** -- reads system state, checks for pending work
2. **Reasons** -- picks the highest-priority action
3. **Acts** -- stores memories, calls APIs, sends notifications
4. **Logs** -- records what happened

Between heartbeats, **autoPrompter** watches for manual prompts and
events from input watchers.

## Interacting with Neil

### Manual prompts (ask questions, give tasks)

```sh
echo "Research the latest on transformer architectures" > ~/.neil/tools/autoPrompter/queue/research.md
```

### Check recent activity

```sh
tail -5 ~/.neil/heartbeat_log.json
```

### Search Neil's memory

```sh
export ZETTEL_HOME=~/.neil/memory/palace
~/.neil/memory/zettel/zettel find --text "transformers"
```

### Semantic search (find by meaning)

```sh
source ~/.neil/memory/mempalace/.venv/bin/activate
mempalace --palace ~/.neil/memory/palace/.mempalace search "how attention works"
```

### View notifications

```sh
cat ~/.neil/outputs/neil.log
```

## Adding Services

Neil can call external APIs through the service broker. To add a new
service:

1. Create a registry file: `~/.neil/services/registry/myservice.md`
2. Add credentials: `~/.neil/services/vault/myservice.key`
3. Neil discovers it automatically on the next cycle

See existing registry files for the format.

## Backups and Recovery

Snapshots are automatic (every 6 hours via cron). Manual control:

```sh
~/.neil/self/snapshot.sh list          # view history
~/.neil/self/snapshot.sh save "note"   # manual backup
~/.neil/self/snapshot.sh restore abc12 # rollback to commit
```

## Key Files to Know

| File | What it does |
|------|-------------|
| `essence/identity.md` | Neil's personality and core traits |
| `essence/mission.md` | Current objectives -- edit to change focus |
| `essence/soul.md` | Behavioral rules -- edit with care |
| `essence/guardrails.md` | Safety limits (budget, permissions, quiet hours) |
| `heartbeat_log.json` | Last 10 heartbeat entries |
| `intentions.json` | Deferred tasks Neil plans to do |
| `self/failures.json` | Error log for self-debugging |
| `self/lessons.md` | Patterns Neil has learned |
| `deployment.md` | Per-machine configuration |

## Customizing Neil

**Change the mission**: Edit `~/.neil/essence/mission.md` to give Neil
new objectives. This is loaded on every invocation.

**Adjust the heartbeat**: Edit the cron schedule to change frequency.
Default is every 30 minutes.

**Add quiet hours**: `essence/guardrails.md` defines 23:00-07:00 as
quiet hours. Adjust to your timezone/preference.

**Change personality**: `essence/identity.md` defines Neil's personality
type (INFJ), communication style, and core strengths.

## Troubleshooting

**autoPrompter won't start**: Check `journalctl -u autoprompt -f` for
errors. Common issue: Claude CLI not in PATH. Fix the `Environment=PATH`
line in the systemd service.

**No heartbeats**: Verify cron is running: `crontab -l | grep heartbeat`.
Check `/tmp/heartbeat_cron.log` for errors.

**Memory search returns nothing**: Run `~/.neil/memory/zettel/zettel reindex`
to rebuild indexes.

**Semantic search broken**: Recreate the venv:
```sh
cd ~/.neil/memory/mempalace
rm -rf .venv
python3 -m venv .venv
source .venv/bin/activate
pip install -e .
```

## Architecture

For the full system architecture, see `~/.neil/README.md`.

---

*openclaw v0.1 -- Neil the SEAL :)*
