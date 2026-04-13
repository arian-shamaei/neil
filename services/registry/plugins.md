# Plugins

Manage Neil's plugin system. Install, remove, and browse capabilities.

## Account

- **identity**: local (no auth needed)
- **scope**: plugin management

## Actions

### list
List installed plugins.
```
CALL: service=plugins action=list
```

### available
List plugins available for installation.
```
CALL: service=plugins action=available
```

### install
Install a plugin by name.
```
CALL: service=plugins action=install name=<plugin-name>
```

### remove
Remove an installed plugin.
```
CALL: service=plugins action=remove name=<plugin-name>
```

### info
Show details about a plugin.
```
CALL: service=plugins action=info name=<plugin-name>
```
