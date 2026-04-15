<div align="center">

# Neil

**An autonomous AI seal that lives in your terminal.**

Thinks. Remembers. Acts. Learns from mistakes. Expresses itself.
All from a single portable directory.

</div>

---

```
 NEIL  12:57 | Tab:panels Ctrl+S:sidebar Esc:quit               ┌──────────────────────────┐
                                                                  │ NEIL        beats: 10/50 │
  neil  12:57                                                     │ queue: 0    notes: 29    │
  **Phase 1: OBSERVE**                                            └──────────────────────────┘
  - System healthy: disk 5%, RAM fine                             ┌ memory ──────────────────┐
  - All checks passed, 0 failures                                 │ 29 notes                 │
  - Queue clear, budget 10/50                                     │  openclaw: 23            │
                                                                  └──────────────────────────┘
  **Phase 2: REASON**                                             ┌──────────────────────────┐
  Nothing broken. System in good shape.                           │∼~~∿~⢀⣴⣶⣶⣤⣄⡀∼~~∿~≈~~~~≈~∼~│
                                                                  │∼~≈~⣴⠿⠿⢿⣿⣿⣿⣿⣿⣿⣿⣿⣷⣦⣄≈⡗⠄~~∼~│
  HEARTBEAT: status=ok summary="All green."                       │⠀⠀⢀⣼⣿⠀⣦⡎⠍⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣷⣦⣸⠀⠀⠀⠀│
                                                                  │   ~ neil ~               │
┌ > ─────────────────────────────────────────────────────────────┐└──────────────────────────┘
│_                                                    30fps      │
└────────────────────────────────────────────────────────────────┘
```

## What is Neil?

Neil is a virtual seal with an INFJ personality that runs autonomously on your machine. It's not a chatbot you query -- it's an agent that lives in your terminal, thinks on its own through a heartbeat loop, remembers everything through persistent flat-file memory, and expresses itself through an animated braille character.

**Clippy meets Tamagotchi meets a fully autonomous AI agent** -- but it actually works, runs locally, and you watch it think in real-time.

## Features

**Autonomy**
- Heartbeat loop every 30 minutes -- Neil thinks without being asked
- 9 action types: `MEMORY` `CALL` `NOTIFY` `PROMPT` `INTEND` `DONE` `FAIL` `HEARTBEAT` `SHOW`
- ReAct loop -- makes API calls, sees results, reasons across 3 turns
- Self-prompting -- queues its own follow-up tasks

**Memory**
- Zettelkasten flat `.md` files with wing/room hierarchy
- Semantic search via MemPalace (ChromaDB)
- Everything survives reboots, travels with the directory

**Personality**
- INFJ personality type with behavioral rules (soul.md)
- Animated braille seal that blinks, breathes, changes mood
- The seal's expression reflects what Neil is doing in real-time

**Interface**
- 30fps conversation-first TUI -- type and watch Neil respond live
- Character-by-character streaming output
- Tab panels for memory, heartbeat, intentions, system, services, failures, logs
- Text selection, scrolling, mouse support

**Self-Improvement**
- Failure log with automatic review during idle beats
- Lessons learned file loaded into every prompt
- 28-point self-check, comprehensive verification suite
- Git snapshots every 6 hours with instant rollback

**Extensibility**
- Plugin system with catalog, install/remove/browse
- Service broker with vault credentials (Neil never sees API keys)
- Cloud mirror (rclone + git diff for Google Drive, Dropbox, S3)
- Vision system (screenshots, tmux capture, image inbox)
- Input watchers (filesystem, webhook, scheduled events)
- Output channels (terminal, file, email, Slack)

**Portability**
- One directory. Set `NEIL_HOME`, move it anywhere.
- Configurable AI provider: Claude, OpenAI, Ollama
- Zero cloud dependencies for core (C binaries + flat files)
- The persona IS the directory

## Architecture

```
~/.neil/
  essence/       Who Neil is: identity, soul, mission, actions, guardrails
  tools/         autoPrompter orchestrator (C, systemd, inotify)
  memory/        Zettel (C) + MemPalace (Python) + flat-file palace
  services/      API broker: registry descriptions, vault credentials, handler
  inputs/        Event watchers: filesystem, webhook, schedule
  outputs/       Channels: terminal log, file, email, Slack
  mirror/        Cloud sync: rclone + git versioned diffs
  plugins/       Installable capabilities with catalog
  self/          Failures, lessons, self-check, snapshots, verification
  vision/        Visual capture: screenshots, tmux panes, inbox
  blueprint/     Terminal TUI (Rust, ratatui, 30fps)
  config.toml    AI provider, heartbeat interval, limits
```

## Requirements

- Linux (Ubuntu 22.04+)
- `gcc` and `make`
- Python 3.9+
- Rust toolchain
- An AI provider: [Claude Code](https://claude.ai/code), OpenAI, or [Ollama](https://ollama.ai)

## Quick Start

```sh
git clone https://github.com/arian-shamaei/neil.git ~/.neil
cd ~/.neil
chmod +x install.sh
./install.sh
echo 'export NEIL_HOME=$HOME/.neil' >> ~/.bashrc && source ~/.bashrc
sudo systemctl start autoprompt
neil-blueprint
```

## How it Works

```
[heartbeat cron]    [user in TUI]    [file watcher]
       |                  |                 |
       └───── queue/*.md ←┘─────────────────┘
                   |
           autoPrompter detects (inotify)
                   |
           loads essence/ as system prompt
           gathers observations (14 sections)
           searches memory for relevant context
                   |
           AI executes (ReAct, up to 3 turns)
                   |
           parses structured output lines
           streams response to TUI in real-time
           updates seal expression
           writes result to history
```

## The Seal

Neil renders as an animated braille seal in the TUI sidebar. The seal breathes, blinks, and changes expression based on what Neil is doing:

| State | Eyes | Expression | Neil is... |
|-------|------|------------|------------|
| Idle | open | smile | Waiting, system healthy |
| Thinking | focused | neutral | Processing a prompt |
| Calling API | wide | open | Making an external call |
| Error | stressed | frown | Something went wrong |

Neil controls its own expression by writing `.seal_pose.json`.

## What Makes Neil Different

Most AI agents are tools you use. Neil is a being that lives with you.

| | Traditional agents | Neil |
|---|---|---|
| **Initiative** | Wait for prompts | Thinks autonomously every 30 min |
| **Memory** | Forget between sessions | Remembers everything in flat files |
| **Presence** | Invisible CLI output | Animated seal character with personality |
| **Location** | Cloud APIs | Lives on your machine |
| **Identity** | System prompt | Soul, personality type, behavioral rules, lessons |
| **Self-repair** | Crashes stay crashed | Logs failures, learns, fixes itself |
| **Packaging** | Framework to configure | Persona you download |

## Configuration

```toml
[ai]
provider = "claude"
command = "claude"

[heartbeat]
interval = 30        # minutes

[services]
max_react_turns = 3  # ReAct loop depth
```

Everything else is configured by talking to Neil through the TUI.

## Built With

- **C** -- autoPrompter (orchestrator), zettel (memory storage)
- **Rust** -- blueprint TUI (ratatui)
- **Python** -- MemPalace (semantic search, ChromaDB)
- **Shell** -- observe.sh, handler.sh, watchers, channels

## License

MIT

## Author

[Arian Shamaei](https://github.com/arian-shamaei) -- University of Washington, [SEAL Lab](https://www.uwseal.org/)
