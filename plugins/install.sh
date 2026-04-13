#!/bin/sh
# install.sh -- Plugin manager for Neil
# Usage:
#   install.sh add <path>       Install from local path or URL
#   install.sh remove <name>    Remove installed plugin
#   install.sh list             List installed plugins
#   install.sh available        List available plugins
#   install.sh update           Fetch latest catalog
#   install.sh info <name>      Show plugin details

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
PLUGINS="$NEIL_HOME/plugins"
INSTALLED="$PLUGINS/installed"
AVAILABLE="$PLUGINS/available"
REGISTRY="$NEIL_HOME/services/registry"
HANDLER="$NEIL_HOME/services/handler.sh"

cmd_add() {
    SRC="$1"
    if [ -z "$SRC" ]; then
        echo "Usage: install.sh add <path-or-name>"
        return 1
    fi

    # If it's a name, look in available/
    if [ ! -d "$SRC" ] && [ -d "$AVAILABLE/$SRC" ]; then
        SRC="$AVAILABLE/$SRC"
    fi

    if [ ! -d "$SRC" ]; then
        echo "ERROR: plugin not found: $SRC"
        return 1
    fi

    # Read plugin.json
    if [ ! -f "$SRC/plugin.json" ]; then
        echo "ERROR: no plugin.json in $SRC"
        return 1
    fi

    NAME=$(cat "$SRC/plugin.json" | sed -n 's/.*"name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
    if [ -z "$NAME" ]; then
        echo "ERROR: plugin.json missing 'name' field"
        return 1
    fi

    # Check if already installed
    if [ -d "$INSTALLED/$NAME" ]; then
        echo "Plugin '$NAME' already installed. Remove first to reinstall."
        return 1
    fi

    # Copy to installed/
    cp -r "$SRC" "$INSTALLED/$NAME"
    echo "[plugins] copied to installed/$NAME"

    # Symlink registry.md
    if [ -f "$INSTALLED/$NAME/registry.md" ]; then
        ln -sf "$INSTALLED/$NAME/registry.md" "$REGISTRY/$NAME.md"
        echo "[plugins] registry linked: services/registry/$NAME.md"
    fi

    # Append handler if exists
    if [ -f "$INSTALLED/$NAME/handler.sh" ]; then
        echo "" >> "$HANDLER"
        echo "# --- plugin: $NAME ---" >> "$HANDLER"
        cat "$INSTALLED/$NAME/handler.sh" >> "$HANDLER"
        echo "[plugins] handler appended to services/handler.sh"
    fi

    # Run setup if exists
    if [ -f "$INSTALLED/$NAME/setup.sh" ]; then
        echo "[plugins] running setup..."
        sh "$INSTALLED/$NAME/setup.sh"
    fi

    # Create vault template reminder
    if [ -f "$INSTALLED/$NAME/vault.template" ]; then
        echo "[plugins] CREDENTIAL NEEDED:"
        cat "$INSTALLED/$NAME/vault.template"
        echo ""
        echo "Create: $NEIL_HOME/services/vault/$NAME.key"
    fi

    echo "[plugins] installed: $NAME"
}

cmd_remove() {
    NAME="$1"
    if [ -z "$NAME" ]; then
        echo "Usage: install.sh remove <name>"
        return 1
    fi

    if [ ! -d "$INSTALLED/$NAME" ]; then
        echo "Plugin '$NAME' not installed."
        return 1
    fi

    # Remove registry symlink
    rm -f "$REGISTRY/$NAME.md"

    # Remove handler lines (between markers)
    if grep -q "# --- plugin: $NAME ---" "$HANDLER" 2>/dev/null; then
        sed -i "/# --- plugin: $NAME ---/,/# --- plugin:/{/# --- plugin: $NAME ---/d;/# --- plugin:/!d}" "$HANDLER"
        echo "[plugins] handler cleaned"
    fi

    # Remove installed dir
    rm -rf "$INSTALLED/$NAME"
    echo "[plugins] removed: $NAME"
}

cmd_list() {
    echo "Installed plugins:"
    for DIR in "$INSTALLED"/*/; do
        [ -d "$DIR" ] || continue
        NAME=$(basename "$DIR")
        VER=$(cat "$DIR/plugin.json" 2>/dev/null | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
        DESC=$(cat "$DIR/plugin.json" 2>/dev/null | sed -n 's/.*"description"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
        echo "  $NAME ($VER) -- $DESC"
    done
}

cmd_available() {
    echo "Available plugins:"
    for DIR in "$AVAILABLE"/*/; do
        [ -d "$DIR" ] || continue
        NAME=$(basename "$DIR")
        # Skip if already installed
        if [ -d "$INSTALLED/$NAME" ]; then
            STATUS="[installed]"
        else
            STATUS=""
        fi
        VER=$(cat "$DIR/plugin.json" 2>/dev/null | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
        DESC=$(cat "$DIR/plugin.json" 2>/dev/null | sed -n 's/.*"description"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
        echo "  $NAME ($VER) -- $DESC $STATUS"
    done
}

cmd_info() {
    NAME="$1"
    DIR="$INSTALLED/$NAME"
    [ -d "$DIR" ] || DIR="$AVAILABLE/$NAME"
    if [ ! -d "$DIR" ]; then
        echo "Plugin not found: $NAME"
        return 1
    fi
    echo "=== $NAME ==="
    cat "$DIR/plugin.json" 2>/dev/null
    echo ""
    if [ -f "$DIR/vault.template" ]; then
        echo "=== Credentials needed ==="
        cat "$DIR/vault.template"
    fi
}

cmd_update() {
    echo "[plugins] Catalog update not yet configured."
    echo "To add plugins manually, place them in: $AVAILABLE/"
}

case "${1:-}" in
    add)       shift; cmd_add "$@" ;;
    remove)    shift; cmd_remove "$@" ;;
    list)      cmd_list ;;
    available) cmd_available ;;
    info)      shift; cmd_info "$@" ;;
    update)    cmd_update ;;
    *)
        echo "Usage: install.sh <add|remove|list|available|info|update>"
        ;;
esac
