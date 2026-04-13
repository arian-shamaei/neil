# Test Service

A test service for verifying the CALL: pipeline. No authentication required.

## Account

- **identity**: public API (no auth)
- **scope**: read-only
- **rate limit**: none

## Actions

### echo

Returns whatever you send. Use to verify the pipeline works.

```
CALL: service=test action=echo message="hello world"
```

| Param   | Required | Description |
|---------|----------|-------------|
| message | yes      | Text to echo back |

### time

Returns the current server time.

```
CALL: service=test action=time
```

No parameters required.

### ip

Returns the server's public IP address.

```
CALL: service=test action=ip
```

No parameters required.
