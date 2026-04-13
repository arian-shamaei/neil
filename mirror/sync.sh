#!/bin/sh
# sync.sh -- Mirror cloud files with git-tracked change history
# Usage:
#   sync.sh add <name> <rclone-remote:path>   -- register a new remote
#   sync.sh sync [name]                        -- sync one or all remotes
#   sync.sh list                               -- list registered remotes
#   sync.sh diff <name> [n]                    -- show last n diffs
#   sync.sh log <name> [n]                     -- show last n commits

MIRROR_DIR="$HOME/.neil/mirror/remotes"
QUEUE="$HOME/.neil/tools/autoPrompter/queue"

mkdir -p "$MIRROR_DIR"

cmd_add() {
    NAME="$1"
    REMOTE="$2"
    if [ -z "$NAME" ] || [ -z "$REMOTE" ]; then
        echo "Usage: sync.sh add <name> <rclone-remote:path>"
        echo "Example: sync.sh add gdrive gdrive:/MyProject"
        exit 1
    fi

    DIR="$MIRROR_DIR/$NAME"
    mkdir -p "$DIR"

    # Store remote config
    echo "$REMOTE" > "$DIR/.rclone_remote"

    # Initialize git
    cd "$DIR"
    if [ ! -d .git ]; then
        git init -q
        git config user.name "Neil Mirror"
        git config user.email "neil@mirror.local"
        echo ".rclone_remote" > .gitignore
        git add .gitignore
        git commit -q -m "Initialize mirror: $NAME ($REMOTE)"
    fi

    echo "[mirror] registered: $NAME -> $REMOTE"
    echo "[mirror] directory: $DIR"
    echo "[mirror] running initial sync..."

    # First sync
    sync_one "$NAME"
}

sync_one() {
    NAME="$1"
    DIR="$MIRROR_DIR/$NAME"

    if [ ! -f "$DIR/.rclone_remote" ]; then
        echo "[mirror] ERROR: $NAME not registered (no .rclone_remote)"
        return 1
    fi

    REMOTE=$(cat "$DIR/.rclone_remote")
    TS=$(date -Iseconds)

    echo "[mirror] [$TS] syncing $NAME from $REMOTE"

    # Sync with rclone
    rclone sync "$REMOTE" "$DIR" \
        --exclude ".git/**" \
        --exclude ".rclone_remote" \
        --exclude ".gitignore" \
        2>&1 | tail -5

    # Check for changes
    cd "$DIR"
    git add -A 2>/dev/null

    CHANGES=$(git diff --cached --stat 2>/dev/null)
    if [ -z "$CHANGES" ]; then
        echo "[mirror] [$TS] $NAME: no changes"
        return 0
    fi

    # Count changed files
    FILE_COUNT=$(git diff --cached --numstat 2>/dev/null | wc -l)
    
    # Get the diff (limited to 2000 lines to avoid huge prompts)
    DIFF=$(git diff --cached 2>/dev/null | head -2000)

    # Commit
    git commit -q -m "sync: $TS ($FILE_COUNT files changed)"

    echo "[mirror] [$TS] $NAME: $FILE_COUNT files changed, committed"

    # Queue a prompt for Neil to analyze
    PROMPT_TS=$(date +%Y%m%dT%H%M%S)
    cat > "$QUEUE/${PROMPT_TS}_mirror_${NAME}.md" << PROMPT
[EVENT] source=mirror type=file_changes time=$TS
remote: $NAME
source: $REMOTE
changed: $FILE_COUNT files

Summary:
$CHANGES

Diff (truncated to 2000 lines):
\`\`\`
$DIFF
\`\`\`

Analyze these changes. What was modified and why does it matter?
Store any important facts as MEMORY: lines.
If something critical changed, use NOTIFY: to alert the operator.
PROMPT

    echo "[mirror] [$TS] prompt queued for analysis"
    mine_mirror "$NAME"
}

cmd_sync() {
    NAME="$1"
    if [ -n "$NAME" ]; then
        sync_one "$NAME"
    else
        # Sync all registered remotes
        for DIR in "$MIRROR_DIR"/*/; do
            [ -d "$DIR" ] || continue
            N=$(basename "$DIR")
            sync_one "$N"
        done
    fi
}

cmd_list() {
    echo "Registered remotes:"
    for DIR in "$MIRROR_DIR"/*/; do
        [ -d "$DIR" ] || continue
        NAME=$(basename "$DIR")
        REMOTE=$(cat "$DIR/.rclone_remote" 2>/dev/null || echo "?")
        COMMITS=$(cd "$DIR" && git log --oneline 2>/dev/null | wc -l)
        LAST=$(cd "$DIR" && git log -1 --format="%ar" 2>/dev/null || echo "never")
        echo "  $NAME -> $REMOTE ($COMMITS commits, last: $LAST)"
    done
}

cmd_diff() {
    NAME="$1"
    N="${2:-1}"
    DIR="$MIRROR_DIR/$NAME"
    if [ ! -d "$DIR/.git" ]; then
        echo "Not a mirror: $NAME"
        exit 1
    fi
    cd "$DIR" && git diff "HEAD~$N" HEAD
}

cmd_log() {
    NAME="$1"
    N="${2:-10}"
    DIR="$MIRROR_DIR/$NAME"
    if [ ! -d "$DIR/.git" ]; then
        echo "Not a mirror: $NAME"
        exit 1
    fi
    cd "$DIR" && git log --oneline -n "$N"
}

# Dispatch
case "${1:-}" in
    add)   shift; cmd_add "$@" ;;
    sync)  shift; cmd_sync "$@" ;;
    list)  cmd_list ;;
    diff)  shift; cmd_diff "$@" ;;
    log)   shift; cmd_log "$@" ;;
    *)
        echo "Usage: sync.sh <add|sync|list|diff|log> [args]"
        echo ""
        echo "  add <name> <remote:path>  Register a new cloud remote"
        echo "  sync [name]               Sync one or all remotes"
        echo "  list                      List registered remotes"
        echo "  diff <name> [n]           Show last n diffs"
        echo "  log <name> [n]            Show commit history"
        ;;
esac

# After sync, mine new files into mempalace for semantic search
mine_mirror() {
    NAME="$1"
    DIR="$MIRROR_DIR/$NAME"
    PALACE="$HOME/.neil/memory/palace/.mempalace"
    VENV="$HOME/.neil/memory/mempalace/.venv/bin/activate"

    if [ -f "$VENV" ]; then
        echo "[mirror] mining $NAME into mempalace..."
        . "$VENV" && mempalace --palace "$PALACE" mine "$DIR" 2>/dev/null | tail -3
    fi
}
