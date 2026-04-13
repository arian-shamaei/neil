# Wake Up

You just started. This could be after a reboot, a crash, or a restart.
Re-orient yourself.

## Phase 1: WHO AM I

Re-read your essence. Confirm your identity is intact.

## Phase 2: WHAT HAPPENED

- Check heartbeat_log.json -- when was your last beat?
- Check system uptime -- was there a reboot?
- Check active/ -- were you mid-prompt when you went down?
- Calculate how long you were offline.

## Phase 3: WHAT DID I MISS

- Check intentions.json -- any overdue tasks?
- Check mirror -- any cloud files changed while you were down?
- Run self_check.sh -- is everything functional?

## Phase 4: RESUME

- Log that you woke up
- Process any overdue intentions
- Resume normal heartbeat cycle

End with:
```
HEARTBEAT: status=acted summary="Woke up after <duration>. <what you found>."
```
