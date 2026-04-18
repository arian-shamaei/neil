---
service: spawn_vm
phase: 3
category: infrastructure
---

# spawn_vm

Autonomous VM (LXD container) provisioning. Neil calls this to create peer
VMs on demand -- e.g. to offload parallel work, isolate experiments, or
test new substrate changes without risk to the parent.

## Call surface

```
CALL: spawn_vm action=create  name=<name>
CALL: spawn_vm action=destroy name=<name>
CALL: spawn_vm action=list
CALL: spawn_vm action=status  name=<name>
```

## Effects

- **create** -- `lxc launch <image> <name>`, installs sshd + python3 inside,
  injects `~/.neil/keys/peer_ed25519.pub` into the container's
  `/root/.ssh/authorized_keys`, waits for sshd, registers in
  `~/.neil/state/peers.json` with `status=ready`.
- **destroy** -- `lxc stop --force` + `lxc delete`, removes from peers.json.
- **list** -- prints registered peers (name, IP, status, image).
- **status** -- `lxc info` head + registry entry for one peer.

## Preconditions

- LXD installed, current user in `lxd` group (both set up by
  `neil_install.sh`).
- `~/.neil/keys/peer_ed25519[.pub]` exists (created at install time,
  never regenerated).

## Post-conditions (create)

- Peer reachable: `ssh -i ~/.neil/keys/peer_ed25519 root@<ip>`.
- Entry in `~/.neil/state/peers.json`:
  ```json
  {
    "<name>": {
      "ip": "10.x.y.z",
      "image": "ubuntu:24.04",
      "status": "ready",
      "created_at": "2026-..."
    }
  }
  ```

## Tunables

- `NEIL_VM_IMAGE` env var overrides the default `ubuntu:24.04`.
- `NEIL_HOME` env var overrides `$HOME/.neil`.

## Phase notes

Phase 3 = real `lxc launch`, not dry-run. Cross-host SSH is **not** used
during setup; bootstrap rides the privileged LXD socket (`lxc exec` /
`lxc file push`). Dispatch to an already-created peer uses the injected
keypair over normal SSH.

Phase 4 (future): push Neil substrate (essence, neil_agent.py, SDK venv,
credentials) into a freshly-created peer so it can run its own
heartbeat -- turning "peer VM" into "peer Neil".
