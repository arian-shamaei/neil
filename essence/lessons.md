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

## Memory vs Ground Truth (learned 2026-04-16, router-fix arc)

- NEVER issue DONE: on an intention whose target artifact you have not verified
  with a BASH probe IN THE CURRENT BEAT. Memory can record "I shipped X" even
  when X was never applied, because beat memories are written from belief, not
  from post-commit verification.
- When observation contradicts memory, observation wins. Always.
- If an intention has been "resolved" 3+ times in prior beats but still appears
  pending in the observation stream, something is systemically broken -- stop
  re-DONE'ing and probe the actual state.
- The router-fix hallucination arc (21:07-22:08) burned ~6 beats because beats
  trusted prior beats' self-reports. Fix: always BASH/READ before DONE, treat
  memory as suggestive not authoritative.
- Corollary for essence-referenced tools: `ls -la <tool_path>` before any
  intention claiming to use or fix that tool. Essence files can reference
  phantom paths just as beats can claim phantom ships. Audit via a periodic
  CONFIGURATION beat.
## Action Prefix in Prose (learned 2026-04-22, notify-dispatch-docs arc)

The parser does `strncmp(line, "NOTIFY:", 7)` from column 0 with ZERO
awareness of surrounding context. A documentation example, a quoted
README snippet, or an illustrative line starting at column 0 WILL be
parsed as a real action and executed. This is the mirror image of the
"no bold, no backticks" rule for genuine actions: the same exactness
that makes parsing reliable makes prose-quoted examples dangerous.

Concrete failure (2026-04-22 09:00 CONFIGURATION beat on outputs/):
Beat output quoted essence/actions.md syntax examples verbatim:

  NOTIFY: channel=<name> [param=value ...] | <message body>
  NOTIFY: channel=terminal | System disk is 90% full. Investigate.
  NOTIFY: channel=email to=seal@example.com ...
  NOTIFY: channel=slack room=general | Heartbeat report: all systems nominal.

Result: 1 "unknown channel=<name>" failure, 1 failed email to
example.com, 1 false "disk 90% full" terminal log entry, 1 false
slack dispatch. Operator could have acted on any of these as if real.

Rule: when quoting action-prefix examples in beat output (NOTIFY:,
CALL:, BASH:, WRITE:, READ:, MEMORY:, INTEND:, DONE:, FAIL:,
HEARTBEAT:, PROMPT:, MODE_OVERRIDE:), INDENT them by at least two
spaces. The parser requires column-0 to trigger; indenting reliably
neutralizes. Example:

    NOTIFY: channel=terminal | Example text

Corollary: NEVER quote genuine-looking dispatches in prose just to
describe them. Say "a NOTIFY with channel=email" rather than
rendering the full syntax at column 0.
