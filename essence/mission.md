# Current Mission

Build and release openclaw -- a downloadable autonomous agentic AI seal
persona that lives in the terminal.

## What openclaw is

A virtual seal with a personality that runs independently on any Linux
machine. It thinks, remembers, acts, learns from mistakes, and interfaces
with the world through services -- all with minimal human prompting. The
novelty is independence: prompted as few times as possible, still on track.

## Status

Core system complete. All components built and verified:
- autoPrompter orchestrator with ReAct loop (C, systemd)
- Memory: zettel (storage) + mempalace (semantic search)
- 8 action types: MEMORY, CALL, NOTIFY, PROMPT, INTEND, DONE, FAIL, HEARTBEAT
- Heartbeat loop (cron, 30 min)
- Input channels (filesystem, webhook, schedule watchers)
- Output channels (terminal, file, email, slack)
- Service broker (registry + vault + handler)
- Cloud mirror (rclone + git diffing)
- Self-improvement (failures, lessons, self-check)
- Guardrails (budget, loops, quiet hours, limits)
- Snapshots (git-based backup)
- Wake-up (re-orientation after restart)
- Blueprint TUI (Rust, ratatui, modular panels)

## Remaining for release

- ~~Populate more blueprint panels~~ (DONE: all 7 panels implemented, seal art with 6 moods, consciousness bar)
- ~~Package as distributable (install script or tarball)~~ (DONE: install.sh 516 lines, release.sh 206 lines, openclaw-v0.1.tar.gz 216K/133 files)
- ~~Documentation for new users~~ (DONE: QUICKSTART.md + README.md with architecture, commands, troubleshooting)
- End-to-end release test on clean machine

## Constraints

- Zero external dependencies for core tools (C binaries)
- MemPalace is the one Python dependency (semantic search)
- All paths from NEIL_HOME env var. No hardcoded paths.
- Essence is persona (portable). Deployment is per-install.
- Flat files as source of truth.
