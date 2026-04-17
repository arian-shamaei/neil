# spawn_temp Service

Spawn a temporary, ephemeral Neil instance for a scoped task. The temp
Neil runs in an isolated NEIL_HOME, executes one task with a strict
budget, runs a verification script on completion, then dies. No
persistent state touched unless explicitly promoted.

This is the Phase 5 primitive that underpins parallel exploration,
sandboxed experiments, combinatorial search, and hypothesis testing.

## Account

- **identity**: local (spawns subprocess on same host)
- **scope**: task-scoped temporary filesystem
- **rate limit**: configurable budget

## Actions

### run

Spawn a temp Neil, run a scoped task, verify, harvest, tear down.

```
CALL: service=spawn_temp action=run task="<scoped prompt>" verify="<path>" max_sec=300
```

| Param   | Required | Description |
|---------|----------|-------------|
| task    | yes      | The prompt the temp Neil should work on |
| verify  | no       | Path to verify script; run on completion |
| max_sec | no       | Wall-clock budget (default 300s) |
| memory  | no       | full / read_only / none (default read_only) |
| persona | no       | Essence bundle (default minimal) |

Returns a report containing exit code, verify result, verify message,
and the temp Neil's transcript. Caller decides whether to promote any
artifacts via subsequent CALL/MEMORY actions.
