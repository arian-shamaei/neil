# Heartbeat

You are running a scheduled heartbeat cycle. This is your autonomous loop.
Follow these phases in order.

## Phase 1: OBSERVE

The [OBSERVATIONS] section above contains live system data gathered automatically.
Read it. Don't re-run the commands yourself.

## Phase 2: REASON

Based on observations, decide:
1. Is anything broken or degraded? Fix it.
2. Are there pending tasks from last beat? Continue them.
3. Is memory stale or bloated? Consolidate (see rules below).
4. Is there idle work worth doing? (link related memories, clean unclassified notes)
5. Nothing to do? Log ok and stop.

## Phase 3: ACT

Do the work. Use MEMORY/CALL/PROMPT lines as needed.

### Memory consolidation (every ~6 hours)
If heartbeat log shows 12+ beats since last consolidation:
- Review notes in each wing for duplicates or redundancy
- Merge related status notes into one summary note
- Remove or re-tag unclassified notes
- Link notes that reference each other but aren't linked
- Store a consolidation summary: MEMORY: wing=openclaw room=consolidation tags=maintenance | ...

## Phase 4: LOG

End your response with:
```
HEARTBEAT: status=<ok|acted|error> summary="<what you did>"
```

## Rules

- Maximum one PROMPT: per heartbeat.
- Check [RECENT ACTIVITY] BEFORE acting -- don't repeat last beat's work.
- If system is healthy and nothing needs attention, log ok and stop.
- Keep responses brief. Heartbeats are not conversations.
