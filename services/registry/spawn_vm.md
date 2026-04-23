---
service: spawn_vm
phase: 3
category: infrastructure
---

# spawn_vm

Autonomous VM (LXD container) provisioning. Neil calls this to create peer
VMs on demand — e.g. to offload parallel work, isolate experiments, or
run paired-Neil rubber-duck cycles.

## Call surface

### create — launch a peer VM

```
CALL: service=spawn_vm action=create \
      name=<name> \
      persona=<persona_name> \
      memory_mode=<mode> \
      initial_intention="<verbatim first-heartbeat prompt for the peer>"
```

**Parameters:**

| Param | Required | Default | Purpose |
|---|---|---|---|
| `name` | yes | — | Container name. Must be unique; lowercase letters, digits, hyphens. |
| `persona` | no | `minimal` | Persona name from `~/.neil/personas/`. Written to peer's `spawn_config.json`. |
| `memory_mode` | no | `read_only_parent` | One of: `none`, `ephemeral`, `scoped`, `read_only_parent`, `synthesis_gate`, `federated`, `full`. |
| `initial_intention` | no | `""` | **Verbatim text** passed to the peer's first autonomous heartbeat at spawn time. Describes the peer's role, first action, counterpart, and success criterion. Empty string = peer will sit idle asking for direction. |

**These are the ONLY recognized param names.** `role=`, `essence=`, `spec=`, `phase=`, etc. are NOT valid and will be silently dropped by spawn_vm.sh. Put all role-shaping text inside the single `initial_intention="..."` value.

### destroy — remove a peer

```
CALL: service=spawn_vm action=destroy name=<name>
```

### list — show all registered peers

```
CALL: service=spawn_vm action=list
```

### status — inspect one peer

```
CALL: service=spawn_vm action=status name=<name>
```

## Effects of create

1. `lxc launch ubuntu:24.04 <name>` — 60s cold
2. apt install sshd + python + venv inside container — 90s
3. Create `neil` user (non-root; required because claude-agent-sdk refuses root)
4. Inject Neil's `peer_ed25519.pub` into peer's `authorized_keys`
5. Push complete substrate: essence/, neil_agent.py, venv + claude-agent-sdk, credentials, services/handler.sh + registry, state skeleton
6. Write `spawn_config.json` on peer with {name, parent_node, persona, memory_mode, initial_intention}
7. **Fire first autonomous heartbeat on peer via ssh + neil_agent.py**: peer reads its config, acknowledges role, writes `ready.md`, begins first concrete step
8. Register in parent's `~/.neil/state/peers.json` with `status=ready`

## Preconditions

- LXD installed, current user in `lxd` group
- `~/.neil/keys/peer_ed25519[.pub]` exists
- `~/.neil/services/vault/spawn_vm.key` exists (placeholder fine)

## Post-conditions (create)

- Peer reachable: `ssh -i ~/.neil/keys/peer_ed25519 neil@<ip>`
- Peer has `~/.neil/state/ready.md` describing its understanding of role
- Entry in `~/.neil/state/peers.json`:
  ```json
  {"<name>": {"ip": "10.x.y.z", "image": "ubuntu:24.04", "status": "ready", "created_at": "2026-..."}}
  ```

## Tunables

- `NEIL_VM_IMAGE` env var overrides default `ubuntu:24.04`
- `NEIL_HOME` env var overrides `$HOME/.neil`
- `PEER_USER` env var (default `neil`) overrides the non-root user name

## Example — correct CALL for a humanizer implementer peer

```
CALL: service=spawn_vm action=create name=humanizer-a persona=implementer memory_mode=scoped initial_intention="You are the Implementer in a paired-Neil humanizer cluster. Read ~/.neil/projects/humanizer/SPEC.md. Produce Phase 1.1 (detector bench) first. Your counterpart is humanizer-b; reach it via CALL: peer_send peer=humanizer-b message=\"...\". Role lock: ship code, not orchestration."
```

Note the single `initial_intention="..."` containing the full role brief. Do NOT split this into multiple params like `role=implementer spec=SPEC.md phase=1.1` — those param names are not recognized.

## Additional declared params (substrate-consumed)

These params ARE consumed by `tools/spawn_vm/spawn_vm.sh` but were not declared above. Listing them here lets `handler.sh validate_params` see them as valid key tokens and prevents false-positive unknown-param FAILs in `outputs/neil.log`.

- `archetype=worker` — behavioral archetype for the peer. Consumed at `spawn_vm.sh:187` and branched at `spawn_vm.sh:687`. Valid: `worker`, `autonomous`, `relay`. Unknown values fall back to `worker`.
- `transfer_paths=/path/a,/path/b` — comma- or space-separated parent-host paths to rsync into the peer at matching destinations. Consumed at `spawn_vm.sh:504` via `transfer_paths_to_peer()`. Paths missing on parent are skipped with a WARN log.

Both params remain optional; omitting them retains the defaults above.
