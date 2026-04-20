---
service: peer_send
category: cluster
phase: 1
---

# peer_send

Deliver a message from this Neil to a peer Neil by dropping a prompt into
the peer's autoPrompter queue over SSH. Required for the rubber-duck pattern
and for any multi-Neil coordination.

## Call surface

```
CALL: peer_send peer=<peer_name> message=<text>
```

- `peer` — registered peer name from `~/.neil/state/peers.json` (resolved to IP + user=neil + key)
- `message` — the prompt text for the peer; written as a `.md` file in the peer's queue

## Effects

1. Looks up peer IP from `state/peers.json`; fails if peer is not `status=ready`
2. Writes a transient file `/tmp/peer_send_<ts>.md` with the message on this host
3. `scp` it to `neil@<peer_ip>:/home/neil/.neil/tools/autoPrompter/queue/<ts>_from_<sender>.md`
4. Appends an entry to `~/.neil/state/cluster_activity.jsonl` with event=peer_send
5. Prints a receipt to stdout that Neil can cite in its heartbeat

## Preconditions

- Peer was created by `spawn_vm` (substrate pushed, neil user exists)
- `~/.neil/keys/peer_ed25519` exists
- Target peer is `status=ready` in the registry

## Post-conditions

- Peer's autoPrompter picks up the new `.md` file within one inotify tick
- Peer processes it as a normal prompt, essence + spawn_config loaded
- Peer can reply via its OWN `CALL: peer_send peer=<sender> message=...`

## Non-goals

- Not a synchronous RPC. Drop-and-forget; peer responds on its own cadence.
- Not encrypted end-to-end (relies on SSH transport security).
- Not persistent — if peer is offline, scp fails and caller sees FAIL.
