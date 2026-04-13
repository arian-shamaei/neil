# Weather

Get current weather conditions for any location.

## Actions

### current
```
CALL: service=weather action=current location="Seattle,WA"
```
| Param    | Required | Description |
|----------|----------|-------------|
| location | yes      | City name or city,country code |

### forecast
```
CALL: service=weather action=forecast location="Seattle,WA" days=3
```
| Param    | Required | Description |
|----------|----------|-------------|
| location | yes      | City name |
| days     | no       | Days ahead (default 3, max 5) |
