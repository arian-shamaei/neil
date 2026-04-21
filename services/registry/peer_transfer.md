---
service: peer_transfer
phase: 1
category: cluster
---

# peer_transfer

Copy files and directories between the parent Neil and a peer Neil over SSH.
The third member of the cluster-communication triad:

| Service | Carries | Bytes/msg |
|---|---|---|
| `peer_send` | a text prompt + captured reply | typically <10 KB |
| `peer_transfer` | files / directories | up to tens of MB |
| `spawn_vm` | the entire standard substrate (one-time at create) | fixed |

## Call surface

```
CALL: service=peer_transfer peer=<peer_name> \
      [direction=<push|pull>] \
      source=<abs_path> \
      dest=<abs_path> \
      [recursive=<true|false>]
```

| Param | Required | Default | Purpose |
|---|---|---|---|
| `peer` | yes | — | Target peer name (must be `status=ready` in peers.json) |
| `direction` | no | `push` | `push` = parent→peer; `pull` = peer→parent |
| `source` | yes | — | Absolute path on the origin side |
| `dest` | yes | — | Absolute path on the destination side |
| `recursive` | no | `false` | `true` to copy a directory tree (scp -r) |

## Effects

- **push**: `scp -i ~/.neil/keys/peer_ed25519 [-r] <source> neil@<peer_ip>:<dest>`
- **pull**: `scp -i ~/.neil/keys/peer_ed25519 [-r] neil@<peer_ip>:<source> <dest>`
- Appends to `~/.neil/state/cluster_activity.jsonl`:
  ```json
  {"ts":"…","event":"peer_transfer_push","peer":"humanizer-a","peer_ip":"10.x",
   "source":"/home/seal/…","dest":"/home/neil/…","bytes":12345,"rc":0}
  ```

## Preconditions

- Peer is `status=ready` in `~/.neil/state/peers.json`
- `~/.neil/keys/peer_ed25519` exists (per spawn_vm setup)
- Source path exists and is readable on origin side
- Destination parent directory exists (or will be created for `push` if scp can)

## When to use

- **Project data to peer**: peer's `initial_intention` references project
  files (SPEC.md, corpus) that aren't part of the standard substrate — push
  them before the peer's first heartbeat or as a follow-up beat.
- **Artifact harvest**: peer has written an artifact the parent needs to
  promote (phase results, proposed_memories, logs) — pull it.
- **Cross-peer relay**: peer-A produced an artifact that peer-B needs —
  parent pulls from A, then pushes to B. (Direct peer→peer is not yet
  supported; route through parent.)

## Anti-goals

- Not a sync service — single-shot copy, no diff/mirror. If you need
  ongoing mirror, re-issue peer_transfer at each checkpoint.
- Not a replacement for `spawn_vm`'s substrate push — standard Neil files
  (essence, agent, credentials, services) always come via spawn_vm.

## Size guidance

Hard cap: 100 MB per call. For larger, split into multiple calls or use
`recursive=true` on a tree that the individual files fit under.

## Example — push humanizer project to peer before first beat

```
CALL: service=peer_transfer peer=humanizer-a recursive=true \
      source=/home/seal/.neil/projects/humanizer \
      dest=/home/neil/.neil/projects/humanizer
```

## Example — pull a result artifact back

```
CALL: service=peer_transfer peer=humanizer-a direction=pull \
      source=/home/neil/.neil/projects/humanizer/state/phase1_detector_aucs.json \
      dest=/home/seal/.neil/projects/humanizer/state/
```
