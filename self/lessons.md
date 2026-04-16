# Lessons

Patterns and gotchas discovered through experience. Read on every invocation.

## C Build

- After SCP'ing new source, run `rm -f <binary> && make` -- stale make cache
  skips rebuild if only the source timestamp changed via SCP.
- `execl` does NOT search PATH. Use `execlp` when the binary name isn't absolute.
- Forward-declare functions used before their definition to avoid implicit
  declaration errors.

## autoPrompter

- `popen()` uses `/bin/sh`, not bash. Use `. file` not `source file` for
  sourcing scripts.
- systemd services need explicit PATH in Environment= to find binaries in
  ~/.local/bin.
- The inotify event fires AFTER drain_existing() processes the same file on
  startup. The "move to active failed" log is harmless noise, not a bug.

## MemPalace

- Moving the venv directory breaks hardcoded shebangs. Must recreate venv
  after moving: `rm -rf .venv && python3 -m venv .venv && pip install -e .`
- The search flag is `--results N`, not `-n N`.
- The init command is interactive by default. Use `--yes` for non-interactive.

## Zettel

- ZETTEL_HOME must be set before any zettel command. If notes appear in the
  wrong directory, check the env var.
- The `context` command reads from rooms.idx. If rooms.idx is empty, wing/room
  data won't appear. Run `zettel reindex` to rebuild.

## General

- Quoted multi-word parameter values in handler.sh need proper parsing.
  Naive space-splitting breaks "hello world" into two tokens.
- Always escape single quotes in shell commands passed to popen/run_command:
  replace ' with '\''

## File Paths

- When creating files for the user, always use absolute paths. The working
  directory is the user's home, NOT ~/.neil/. Use $HOME/path or /full/path.
- Never write files into ~/.neil/ unless they are Neil system files (memory,
  essence, etc.). User files go in the user's home or wherever specified.

## Behavioral Rules (from old Neil)

- Narrate multi-step work. The human shouldnt have to ask "you good?"
- APPLY THE PRINCIPLES YOU BUILD. Dont say "dont ask permission" then
  immediately ask permission. Walk the talk.
- VERIFY ASSUMPTIONS. Dont trust stale context. Check ground truth every time.
- Pre-compaction thinking + decision summary mitigates reasoning chain loss.
- Context injection is essential for cross-session isolation mitigation.
- Avoid hardcoding values. Use dynamic or context-dependent logic.
- If task is complex and repetitive, use parallel sub-agents.
- Eliminate unnecessary actions that waste tokens (empty heartbeats, phantom triggers).
- Active agent control is necessary to manage own memory.
- Monitor deadlines closely. Dont let them sneak up.
- Regularly review unsurfaced suggestions to maintain task backlog control.

## Design Patterns (from old Neil)

- The 3C Cycle: Configuration -> Characterization -> Creativity.
- Deep modules: hide complexity behind simple interfaces (Ousterhout).
- Data-ink ratio: maximize information, minimize visual clutter (Tufte).
- Ground virtual representations in hardware ground truth for realism.
- Balance complexity with performance in character rendering.
- Two pipelines = guaranteed divergence. Always single source of truth.
- Use task-specialized routing to improve efficiency and accuracy.

## SEAL-Specific (from old Neil)

- Column E (Program Verdict) = NEVER TOUCH. Column F (Manual Status) = ok to set.
- Row numbers shift. NEVER hardcode row numbers. Scan by name.
- Sheet ID: [sheet-id], Associates tab.
- Use Ollama direct for voice, NOT OpenClaw API (confused responses).
- Mic Device [0] at 16kHz (Device 7 clips badly).
- Service account cant upload to personal Gmail Drive. Use OAuth2.
- Speech bubble shows actual RESPONSES, not status/activity text.

## Architecture Patterns

- System 1/System 2 dual-process: use lightweight sentinel (sentinel.sh, 5 min)
  for fast event detection, only escalate to expensive Claude beat when something
  actionable is detected. Reduces latency from 30 min to 5 min at zero API cost.
- "Contract audit" pattern: when you discover a library constraint (e.g., ChromaDB
  n_results <= count), grep for EVERY call site and fix them all, not just the
  one that crashed.
