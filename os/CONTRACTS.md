# Neil-OS Contracts

Version: 0.1.0

Pinned interfaces between Neil's subsystems. Breaking any schema or tool
signature in this file requires a CONTRACTS.md version bump plus an
operator-signed approval file in `~/.neil/approvals/`.

All state files are **append-only JSON-lines** (one JSON object per line)
unless noted otherwise. This keeps them trivially parseable by shell and
Python, and cheap to tail for IPC.

## State files

### heartbeat_log.json

One line per beat. Keeps the last ~1000 entries (trimmed by autoPrompter).

```json
{
  "timestamp": "YYYY-MM-DDTHH-MM-SS",
  "prompt": "<source .md filename>",
  "status": "ok | acted | error | killed | unknown",
  "summary": "<<=256 chars>",
  "action": "<what was done this beat>",
  "question": "<a question Neil raised>",
  "improvement": "<small incremental fix>",
  "contribution": "<larger creative/planning output>",
  "node_id": "<hostname>"
}
```

Required fields: timestamp, prompt, status. The four report fields
(action, question, improvement, contribution) are populated by beats
that completed the structured report (most do after the Phase-0 template
update). `node_id` added in OS Phase 1 (default: hostname).

### intentions.json

Work queue. One line per intention.

```json
{
  "created": "YYYY-MM-DDTHH-MM-SS",
  "priority": "low | medium | high | critical",
  "due": "<YYYY-MM-DDTHH:MM:SS or empty>",
  "tag": "<optional short label>",
  "description": "<what to do>",
  "status": "pending | completed | cancelled",
  "node_id": "<hostname, Phase 1+>"
}
```

Intentions are created via Neil's `INTEND:` action line and completed
via `DONE:`. autoPrompter (`record_intentions`, `complete_intentions` in
autoprompt.c) owns the writer side.

### failures.json

```json
{
  "timestamp": "YYYY-MM-DDTHH-MM-SS",
  "source": "<component name>",
  "error": "<one-line error>",
  "severity": "low | medium | high | critical",
  "resolution": "pending | resolved",
  "context": "<optional location>",
  "node_id": "<hostname, Phase 1+>"
}
```

Written via Neil's `FAIL:` action line. Resolved when the related work
ships (marked manually or by a subsequent heartbeat).

### .neil_stream

Live output from the current beat to the blueprint TUI. Overwritten
each beat. NOT JSON-lines. Format:

```
{"status":"running","prompt":"<name>"}
<free text, tool calls, tool results>
{"status":"done","exit_code":<int>}
```

The header and footer lines are JSON objects. The middle content is
free-form: Claude's text, plus streamed action markers (`READ: path`,
`WRITE: path=X (N bytes)`, `$ bash cmd`, etc.) written by the agent or
by `stream_action` in autoprompt.c.

### .seal_pose.json

Single JSON object, overwritten each update. Drives blueprint TUI seal
animation.

```json
{
  "eyes": "normal|wide|closed|focused",
  "mouth": "neutral|open|closed|smile",
  "body": "swim|dive|surface",
  "indicator": "none|thought|bubbles|exclaim",
  "label": "<short text>"
}
```

### .blueprint_state.json

Written by the blueprint TUI every ~2.5s. Read-only for other
subsystems.

```json
{
  "running": true,
  "view": "chat | panel_selector | panel:<name>",
  "terminal_size": "WxH",
  "stream_length": <int>,
  "scroll_offset": <int>,
  "auto_scroll": true,
  "input_buffer": "<short preview>",
  "last_user_message": "<last human input>",
  "sidebar_visible": true,
  "user_active": true,
  "streaming": false,
  "last_input_time": "<seconds ago>"
}
```

### state/services.json (Phase 2+)

Liveness tracking by neil-init.

### state/budget.json (Phase 4+)

Agent-visible budget tracking by neil-warden.

## Agent tool surface (MCP)

Neil's agent (`tools/autoPrompter/agent/neil_agent.py`) exposes exactly
these MCP tools. Adding or renaming tools is a CONTRACTS.md breaking
change.

| Tool             | Input                         | Effect                              |
|------------------|-------------------------------|-------------------------------------|
| `read_file`      | `{path: str}`                 | Read file, return <=50KB of text    |
| `write_file`     | `{path: str, content: str}`   | Overwrite or create file            |
| `bash`           | `{command: str}`              | Run shell command (60s timeout)     |
| `call_service`   | `{service, action, params}`   | Dispatch to `services/handler.sh`   |

Tools must be bare strings in Neil's output (`bash ...`) for the CLI
path to fire. The SDK-enforced form (tool_use blocks) is automatic in
the agent SDK path.

## Action lines (declarative, parsed from final text)

These are NOT tool calls. They're declarative markers the C daemon
parses from Neil's final text output. Each must appear bare at the
start of a line.

| Prefix      | Purpose                              | Parser in autoprompt.c |
|-------------|--------------------------------------|------------------------|
| `MEMORY:`   | Store a zettel note                  | `extract_memories`     |
| `HEARTBEAT:`| Log beat status (+ 4 report fields)  | `log_heartbeat`        |
| `INTEND:`   | Queue a deferred task                | `record_intentions`    |
| `DONE:`     | Mark an intention completed          | `complete_intentions`  |
| `FAIL:`     | Log an unresolved failure            | `record_failures`      |
| `NOTIFY:`   | Send fire-and-forget output          | `dispatch_notifications` |
| `PROMPT:`   | Queue a self-prompt (max 1 per beat) | `queue_self_prompt`    |

### HEARTBEAT: structured report (required fields)

```
HEARTBEAT: status=<ok|acted|error>
ACTION: <what you did this beat>
QUESTION: <a genuine question>
IMPROVEMENT: <small concrete improvement>
CONTRIBUTION: <larger creative thought>
```

All four fields are required (enforced by heartbeat_reprompt.md
re-prompt flow in autoprompt.c).

## Environment variables

| Var              | Default              | Used by                            |
|------------------|----------------------|------------------------------------|
| `NEIL_HOME`      | `$HOME/.neil`        | All components                     |
| `NEIL_NODE_ID`   | `$(hostname)`        | All JSON writes (Phase 1+)         |
| `NEIL_MAX_TURNS` | 10                   | neil_agent.py                      |
| `NEIL_PROMPT_NAME` | (set by autoprompt)| neil_agent.py for stream header    |
| `ZETTEL_HOME`    | `$NEIL_HOME/memory/palace` | zettel binary                |

## Config file: config.toml

Flat key=value TOML. The autoprompt.c parser only supports single-value
strings/ints per key (no arrays). Sections are advisory -- the parser
reads all keys from all sections.

Known keys:

```toml
[ai]
provider = "agent-sdk"
command = "<path to python>"
args = "<path to neil_agent.py>"
system_prompt_flag = "--system-prompt"
prompt_flag = "-p"
agent_manages_stream = 1

[heartbeat]
interval = 30
max_daily = 0
quiet_start = "23:00"
quiet_end = "07:00"
mode_routing = true          # false disables beat_router
max_react_turns = 6

[heartbeat.report]            # required report fields (data-driven)
ACTION = "what you did this beat"
QUESTION = "a genuine question you have"
IMPROVEMENT = "small concrete improvement"
CONTRIBUTION = "larger creative or planning thought"

[os]
neil_os_enabled = true        # Phase 1+: master kill switch for OS layer

[services]
max_calls_per_cycle = 10
max_notify_per_cycle = 3
max_react_turns = 6

[blueprint]
tick_rate = 500
layout = "default"
```

## Breaking changes = semver bumps

- Add new field (non-breaking): **0.1.x** patch
- Add new tool / action line: **0.1.x** patch
- Remove or rename field: **0.2.0** minor
- Remove tool / change tool signature: **0.2.0** minor
- Redesign state file format entirely: **1.0.0** major

## How Neil uses this file

Neil's agent has `READ: ~/.neil/os/CONTRACTS.md` available. When Neil
proposes a schema change in a CONTRIBUTION, it should reference
the CONTRACTS.md version and state the compatibility impact. A
breaking change without an approval file in `~/.neil/approvals/` is
rejected by neil-init in Phase 6.
