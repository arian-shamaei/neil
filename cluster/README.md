# Cluster Registry

Child Neil instances registered with this parent Neil. Each file in
`nodes/` is a JSON record for one child Neil (VM, container, embedded
device, etc.) following `schema.json`.

Created by the spawn_vm service when a new child is provisioned; read
by the parent's dispatch logic (future Phase 4 work) to route prompts
to the right child.

## Files

- `schema.json` -- canonical shape of a child Neil record
- `nodes/` -- one JSON file per registered child (created on demand)

## Dry-run mode

When spawn_vm runs with `dry_run=1` (the default during Phase 2), it
writes synthetic records to `nodes/` with ids prefixed `dryrun-` so
they can be distinguished from real cloud resources.
