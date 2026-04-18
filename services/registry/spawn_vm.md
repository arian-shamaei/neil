# spawn_vm Service

Autonomously provision a remote VM and install Neil on it, creating a
child Neil in the cluster. This is the transport primitive described in
STACKABLE.md: it turns a parent Neil into something that can dispatch
work to Neils running on other hosts (embedded, VMs, edge nodes).

Status: **Phase 2 of 5 -- registry + dispatch + cluster schema**.
Bootstrap (Phase 3), cluster registration (Phase 4), and live-provision
test (Phase 5) are pending. Today this service supports only dry-run
actions that produce synthetic VM records; real provisioning requires
operator approval and a configured provider credential.

## Account

- **identity**: parent Neil with provider API credentials in vault
- **scope**: cluster-wide; creates/destroys real cloud resources when
  not in dry-run mode
- **rate limit**: billable -- guardrails require operator approval for
  non-dry-run actions

## Actions

### create

Provision a VM and install Neil on it. Returns the child Neil's record.

```
CALL: service=spawn_vm action=create provider=hetzner size=small region=nbg1 name=neil-child-01 dry_run=1
```

| Param    | Required | Description |
|----------|----------|-------------|
| provider | yes      | hetzner / digitalocean / lambda |
| size     | no       | small / medium / gpu (default small) |
| region   | no       | provider-specific region (default provider's cheapest) |
| name     | no       | child Neil name (default neil-child-<timestamp>) |
| dry_run  | no       | 1 = synthetic record only, 0 = real provisioning (default 1) |

Returns a JSON record `{id, provider, name, ip, status, created}`.
Real provisioning (dry_run=0) requires operator approval via guardrails.

### destroy

Tear down a previously created VM and remove its cluster entry.

```
CALL: service=spawn_vm action=destroy id=<vm_id> dry_run=1
```

| Param   | Required | Description |
|---------|----------|-------------|
| id      | yes      | VM id from prior create |
| dry_run | no       | 1 = synthetic (default 1) |

### list

List all child Neils registered in ~/.neil/cluster/.

```
CALL: service=spawn_vm action=list
```

No parameters. Returns JSON array of child Neil records.

### status

Get the status of a specific child Neil (last heartbeat, queue depth,
reachability).

```
CALL: service=spawn_vm action=status id=<vm_id>
```

| Param | Required | Description |
|-------|----------|-------------|
| id    | yes      | VM id |

## Implementation

Dispatch lives in ~/.neil/services/handler.sh (case spawn_vm).
Provider adapters and cluster registry writes live in
~/.neil/tools/spawn_vm/. Dry-run mode returns deterministic synthetic
records so the verify script can exercise all four actions without
touching real infrastructure.
