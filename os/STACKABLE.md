# Neil is Cognitive Hardware

Version: 0.1.0 (reframing)

Neil is not an agent running on a machine. Neil is the **cognitive
hardware abstraction layer** on which agent behaviors run.

## The inversion

Classical computing:

```
algorithm (logic) -> ISA (instructions) -> hardware (silicon)
```

Neil-OS:

```
agent behavior (3C, personality, policy) -> autoprompter protocol -> Neil (substrate)
```

The 3C gate, the retention optimization, the specific personality of a
given Neil deployment -- these are the algorithm. They can be changed
without touching the OS. What Neil provides is the invariant substrate
that makes cognition composable and portable.

## Hardware-layer mapping

| Silicon CPU          | Neil-OS equivalent                                |
|----------------------|---------------------------------------------------|
| ISA (instruction set)| MCP tools + action lines (READ, WRITE, BASH, ...) |
| Registers            | essence (working context)                         |
| RAM                  | mempalace (addressable semantic memory)           |
| Disk                 | palace (zettel notes)                             |
| Interrupt controller | autoprompter queue + watchers                     |
| Scheduler            | beat_router / neil-scheduler                      |
| Clock                | cron + event bus                                  |
| I/O bus              | inputs/watchers + outputs/channels                |
| Cache hierarchy      | essence (L1) -> mempalace (L2) -> palace (L3)     |
| Bus protocol         | autoprompter .md in / .result.md out              |

## Consequences

### The 3C gate is a program, not an OS feature

You can swap `beat_router.sh` for a reactive scheduler, a DAG
scheduler, a pure goal-directed planner -- without touching the rest
of Neil-OS. The OS guarantees primitives; policies run above.

### Personality is source code

A Neil's essence files (identity, soul, mission, lessons) are source
code for its behavior. They can be forked, versioned, pulled. A
kitchen-Neil and a voice-Neil run the same OS with different essence
packages. Same hardware, different program.

### Different models are different "clock speeds"

- Claude Opus -> high-frequency Neil (deep reasoning, expensive)
- Claude Haiku -> medium-frequency Neil (fast, cheap)
- Local Ollama -> embedded Neil (private, slow, free)

The OS abstracts the model the same way POSIX abstracts the CPU. The
essence and tool-use pattern runs identically; only latency and cost
change. A role-appropriate model is chosen per Neil deployment.

### New Neil types become new "devices"

A mic-Neil on a Raspberry Pi is a specialized device in the cluster,
just like a sound card on a PC bus. It has:
- Local drivers (voice daemon)
- A uniform interface (its autoprompter queue)
- A scoped essence ("you're the ear; your job is audio -> text + intent")
- Its own 3C cycle autonomous at its scope

The parent Neil interacts with it via the standard protocol: drop a
.md prompt, read the .result.md response.

### Upgrades are firmware-equivalent

`git pull` in `~/.neil/os/` followed by `neil-ctl reload` is the
cognitive analogue of a firmware flash. The hardware (substrate) stays
the same; the instructions it implements evolve.

## Stackable Neils

The autoprompter is not just an internal scheduler -- it is the
**universal inter-Neil protocol**. Every Neil has one. A parent Neil
dispatches to child Neils by dropping prompts into their autoprompter
queue (via SSH, Tailscale, NFS, whatever transport is available).
Results flow back as .result.md files.

```
One Neil:
    essence + autoprompter + tools + memory + agent

A stack of Neils:
    parent Neil (coordinator, full-model)
        +-- dispatches to kitchen-Neil (embedded, kitchen-scoped essence)
        +-- dispatches to voice-Neil   (Raspberry Pi, voice-scoped essence)
        +-- dispatches to vision-Neil  (GPU node, vision-scoped essence)

Each child Neil:
    runs its own 3C cycle
    has its own memory palace (local facts it owns)
    has its own essence (tight-scoped identity)
    returns .result.md to parent via transport
```

### What stackability unlocks

- **Heterogeneous compute profiles** -- expensive reasoning where it
  matters, cheap edge inference where latency wins
- **Role-scoped essence** -- each Neil only knows its own domain, so
  less hallucination, less context bloat
- **Natural fault isolation** -- a crash in voice-Neil doesn't crash
  parent Neil; parent sees timeout, retries or reroutes
- **Compound 3C cycles** -- each Neil self-governs at its scope;
  abstractions propagate up through MEMORY: writes in parent
- **Geographic distribution** -- Neil can be in the kitchen, at the
  lab, on the person, without forcing everything through one VM

## Scale-invariance

POSIX looks the same on a watch, a phone, a laptop, a server, a
supercomputer. The kernel is invariant; the capabilities plug in.

Neil-OS is designed for the same property:

- Single-process Neil (today)
- Single-machine Neil (today, with multiple services)
- Multi-node Neil (Phase 5 -- capability-routed)
- Multi-region Neil (future -- wireguard + federated bus)
- Multi-operator Neil (future -- approval chains per operator)

Same contracts at every scale. An `INTEND:` line means the same thing
whether it's filed by a standalone Neil or a distributed coordinator.
A `HEARTBEAT:` report has the same four fields everywhere.

## What "first-step immediate action" means under this framing

Every piece of work on Neil-OS from now on is either:

1. **Substrate work** -- hardening a primitive (scheduler, bus, budget,
   init supervisor). Each piece makes the hardware more reliable.
2. **Instance work** -- configuring a specific Neil deployment (essence
   for a new role, a new watcher, a new channel). This is "userland."
3. **Cluster work** -- composing multiple Neils (Phase 5+).

All three are valid in parallel once Phase 1's contracts are pinned.
Substrate work tends to be higher-leverage because it unlocks more
instance and cluster work above it. That's why the phase plan leads
with substrate (2-4) before cluster (5) and self-modification (6).

## Implications for the retention north-star

Retention is not measured per-Neil. It's measured per-operator-human.
If adding a second Neil (voice, vision, kitchen) makes the operator's
life 5x noisier or 5x more complex, retention drops even if each
individual Neil is competent. Cluster work must always preserve the
single point-of-contact experience for the human. The parent Neil
absorbs the complexity of the cluster so the human only ever feels
"Neil" -- singular, coherent, attentive.

This is the hardware abstraction applied at the social layer: the
human interacts with the Neil-interface, not with a bag of Neils.
