# Plugins

Neil's extensible capability system. A plugin adds a new service that Neil
can call via CALL: lines.

## What is a plugin

A plugin is a directory containing:

```
my-plugin/
  plugin.json       metadata (name, version, description, author)
  registry.md       service description (copied to services/registry/)
  handler.sh        shell handler (sourced by services/handler.sh)
  setup.sh          optional: runs on install (install deps, etc.)
  vault.template    optional: describes what credentials are needed
```

## Installing a plugin

```sh
~/.neil/plugins/install.sh add <path-or-url>
```

This copies the plugin to `installed/`, symlinks registry.md into
`services/registry/`, and appends the handler to `services/handler.sh`.

## Plugin catalog

`available/` contains known plugins that can be browsed and installed.
Fetch the latest catalog:

```sh
~/.neil/plugins/install.sh update
```

This pulls from the community catalog repo.

## Neil can install plugins autonomously

Neil can discover and install plugins by:
1. Browsing `available/` for what's there
2. Outputting: CALL: service=plugins action=install name=<plugin-name>
3. The handler runs install.sh, makes the new service available
4. Next invocation, Neil can CALL: the new service

## Creating a plugin

1. Create a directory with plugin.json, registry.md, handler.sh
2. Test handler.sh locally
3. Add to `available/` or contribute to the community catalog

## Commands

```sh
install.sh add <path>          Install a plugin from local path
install.sh remove <name>       Remove an installed plugin
install.sh list                List installed plugins
install.sh available           List available (not yet installed) plugins
install.sh update              Fetch latest catalog from community repo
```
