# Neil-OS: Cognitive Operating System

Version: 0.1.0
Designed for: an autonomous agent whose "resources" are attention, tokens,
memory, and human trust -- not CPU cycles.

## Core insight

Neil is not an agent running on a machine. Neil IS the cognitive OS that
happens to run on this machine (sealserver for now). Every component maps
to a classical OS concept.

## Component map

| OS concept           | Neil component                                         |
|----------------------|--------------------------------------------------------|
| Kernel               | `essence/` + `guardrails.md` + `beat_router`           |
| Init / systemd       | `tools/autoPrompter/autoprompt` + `neil-init` (Phase 2)|
| Scheduler            | `tools/beat_router/beat_router.sh`                     |
| Syscalls             | MCP tools: `read_file`, `write_file`, `bash`, `call_service` |
| Interrupts / IRQ     | `inputs/watchers/` (webhook, vision_inbox, filesystem, schedule) |
| Device drivers       | Watchers in = drivers in. Channels out = drivers out.  |
| Filesystem           | `memory/palace/` (zettel notes)                        |
| Virtual memory       | `memory/mempalace/` (ChromaDB semantic index)          |
| Cache hierarchy      | essence (L1) -> mempalace (L2) -> palace (L3) -> git (L4) |
| IPC                  | `.neil_stream` + `intentions.json` + `failures.json` + `heartbeat_log.json` + `approvals/` |
| Process              | An intention being executed by a beat                  |
| Daemon               | A long-running watcher or plugin                       |
| Shell                | Blueprint TUI (`blueprint/`)                           |
| Cron                 | `heartbeat.sh` -- the idle-loop ticker                 |
| User                 | The operator (human)                                   |
| Logs                 | `heartbeat_log.json`, `outputs/neil.log`, result files |
| Snapshots            | Git history of `~/.neil/`                              |
| Panic / safe mode    | `self/snapshot.sh` restore                             |
| Resource limits      | `neil-warden` + `state/budget.json` (Phase 4)          |
| Plugin system        | `plugins/available/` + `plugins/installed/`            |

## Directory layout

```
~/.neil/
├── os/                    # NEW: the OS layer (this doc lives here)
│   ├── ARCHITECTURE.md    # this file
│   ├── CONTRACTS.md       # pinned interfaces (state file schemas, tool API)
│   ├── VERSION            # semver
│   ├── tools -> ../tools
│   ├── inputs -> ../inputs
│   ├── outputs -> ../outputs
│   ├── self -> ../self
│   ├── memory -> ../memory
│   ├── essence -> ../essence
│   ├── plugins -> ../plugins
│   ├── services -> ../services
│   └── blueprint -> ../blueprint
│
├── bin/                   # NEW: OS-layer CLIs (callable by Neil via BASH:)
│   └── neil-introspect    # dump current state as JSON/text
│
├── state/                 # NEW: runtime state files (future: budget.json, services.json)
│
├── essence/               # identity/behavior -- the kernel policy
├── tools/                 # autoPrompter, beat_router, + utilities
├── inputs/watchers/       # input drivers: webhook, vision_inbox, filesystem, schedule
├── outputs/channels/      # output drivers: slack, email, terminal, file
├── self/                  # self-health scripts: memory_decay, self_check, snapshot
├── memory/                # palace (notes), mempalace (vectors), zettel (binary)
├── services/              # API service registry + vault
├── plugins/               # available/ + installed/ plugin directories
├── blueprint/             # Rust TUI application
│
├── config.toml            # system configuration
├── heartbeat_log.json     # beat history (last ~1000 entries)
├── intentions.json        # work queue (pending + completed)
├── .neil_stream           # real-time output to TUI
├── .seal_pose.json        # current mood/pose state
├── .blueprint_state.json  # TUI state checkpoint
└── approvals/             # operator-gated signal directory
```

## The three rhythms

Neil's operation spans three oscillators that must phase-lock for stability:

1. **World rhythm** (external): voice, vision, TUI keystrokes, webhooks,
   sensor events, time-of-day
2. **Human rhythm** (retention): user attention, session continuity, trust
   built/eroded over time
3. **Machine rhythm** (self): API token budget, rate limits, VM health,
   memory consolidation, snapshot cadence

Each rhythm has three timescales: fast (ms-s), medium (s-hours), slow
(days-years). **Couplings happen along matching diagonals**. A fast world
signal (keystroke) should trigger fast human response (TUI update) via
fast machine action (tool call). Breaking diagonal coherence is what makes
an agent feel batch-processed rather than alive.

The **north-star parameter** the OS optimizes is human retention:
P(human returns tomorrow | interactions today). Every design decision --
scheduling policy, budget enforcement, interrupt priority -- should be
evaluated against its effect on this.

## The 3C gate (current scheduler policy)

Each beat is classified into one of three modes, enforced by
`beat_router.sh`:

- **Configuration**: study the system, understand what IS, produce MEMORY
- **Characterization**: verify findings, produce DONE/FAIL/INTEND
- **Creativity**: execute a pending intention or failure fix, produce DONE/FAIL

The router reads pending intentions, unresolved failures, and recent beat
history to decide the mode. It prevents "ungrounded creativity"
(proposal spam) by forcing Configuration beats when the last three beats
were all Creativity without prior study.

This is Neil's scheduler policy today. Phase 4 wraps it in a proper
long-running daemon with preemption and rhythm fairness.

## What autoPrompter / the agent can use

All OS tooling is accessible to Neil's agent via normal tool calls:

- **READ the docs**: `READ: ~/.neil/os/ARCHITECTURE.md` and `CONTRACTS.md`
- **Run introspection**: `BASH: neil-introspect` (when `~/.neil/bin` is in PATH)
- **Inspect services**: `BASH: cat ~/.neil/state/services.json` (Phase 2+)
- **Signal an interrupt**: `BASH: neil-ctl signal ...` (Phase 3+)
- **Query budget**: `BASH: cat ~/.neil/state/budget.json` (Phase 4+)

The OS is not a black box. Neil's agent has full READ access to all OS
state and WRITE access to the OS docs themselves (guarded by approvals
in Phase 6). This is how Neil evolves its own OS.

## Phase roadmap

Phase 1 (current): Pure documentation -- ARCHITECTURE.md, CONTRACTS.md,
VERSION, neil-introspect. No behavioral change.

Phase 2: `neil-init` supervisor unifies systemd services under a single
dependency-aware parent.

Phase 3: `neil-busd` pub/sub on `/run/neil/bus.sock` replaces file polling
with real-time signal notification. Enables interrupt-driven beats.

Phase 4: `neil-scheduler` + `neil-warden` provide continuous scheduling
with rhythm fairness, preemption, and agent-visible budget enforcement.

Phase 5: Distribution primitives -- `nodes.toml`, optional TCP bus bridge
for future edge devices (Mac Mini, voice node, camera node).

Phase 6: Self-modification guardrails -- CONTRACTS.md changes require
operator approval, `neil-ctl rollback` uses git history.

See `ancient-crunching-lighthouse.md` in the operator's plans dir for the
full plan. This file (ARCHITECTURE.md) is the canonical, in-system
reference.

## Kill switch

In `config.toml`:

```
[os]
neil_os_enabled = true   # false reverts to pre-os-refactor behavior
```

Every phase has an independent rollback path documented in the plan.
