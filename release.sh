#!/bin/sh
# release.sh -- Package openclaw into a distributable tarball
# Usage: sh release.sh [version]
# Output: openclaw-v<version>.tar.gz in current directory
#
# The tarball contains everything install.sh needs to set up a fresh machine.
# Build artifacts, venvs, runtime state, and secrets are excluded.

set -e

VERSION="${1:-0.1}"
NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
OUTNAME="openclaw-v${VERSION}"
OUTFILE="${OUTNAME}.tar.gz"
STAGING="/tmp/${OUTNAME}"

GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { printf "${CYAN}[release]${NC} %s\n" "$1"; }
ok()    { printf "${GREEN}  [ok]${NC} %s\n" "$1"; }

cat << 'SEAL'

         .-"""-.
        /        \
       |  O    O  |
       |    __    |
       |   /  \   |
        \  '=='  /
    ~~~  '-....-'  ~~~
  ~  Packaging openclaw  ~

SEAL

# ── Validate source ────────────────────────────────────────────────────
info "Validating source at $NEIL_HOME..."

for REQUIRED in \
    essence/identity.md \
    essence/soul.md \
    essence/mission.md \
    essence/overview.md \
    essence/actions.md \
    essence/heartbeat.md \
    tools/autoPrompter/src/autoprompt.c \
    tools/autoPrompter/Makefile \
    tools/autoPrompter/heartbeat.sh \
    tools/autoPrompter/observe.sh \
    memory/zettel/src/zettel.c \
    memory/zettel/Makefile \
    services/handler.sh \
    self/self_check.sh \
    self/snapshot.sh \
    install.sh \
; do
    if [ ! -f "$NEIL_HOME/$REQUIRED" ]; then
        printf "\033[0;31m  [FAIL]\033[0m Missing: %s\n" "$REQUIRED"
        exit 1
    fi
done

ok "All required source files present"

# ── Clean staging area ────────────────────────────────────────────────
rm -rf "$STAGING"
mkdir -p "$STAGING"

info "Copying source files to staging..."

# ── Essence (persona) ────────────────────────────────────────────────
mkdir -p "$STAGING/essence"
cp "$NEIL_HOME"/essence/*.md "$STAGING/essence/"
ok "Essence"

# ── autoPrompter ─────────────────────────────────────────────────────
mkdir -p "$STAGING/tools/autoPrompter/src"
cp "$NEIL_HOME/tools/autoPrompter/src/autoprompt.c" "$STAGING/tools/autoPrompter/src/"
cp "$NEIL_HOME/tools/autoPrompter/Makefile" "$STAGING/tools/autoPrompter/"
for SCRIPT in heartbeat.sh observe.sh; do
    [ -f "$NEIL_HOME/tools/autoPrompter/$SCRIPT" ] && \
        cp "$NEIL_HOME/tools/autoPrompter/$SCRIPT" "$STAGING/tools/autoPrompter/"
done
ok "autoPrompter source"

# ── Zettel ────────────────────────────────────────────────────────────
mkdir -p "$STAGING/memory/zettel/src"
cp "$NEIL_HOME/memory/zettel/src/zettel.c" "$STAGING/memory/zettel/src/"
cp "$NEIL_HOME/memory/zettel/Makefile" "$STAGING/memory/zettel/"
ok "Zettel source"

# ── MemPalace (Python package, no venv or .git) ─────────────────────
mkdir -p "$STAGING/memory/mempalace"
for F in "$NEIL_HOME"/memory/mempalace/*.py "$NEIL_HOME"/memory/mempalace/*.toml "$NEIL_HOME"/memory/mempalace/*.cfg; do
    [ -f "$F" ] && cp "$F" "$STAGING/memory/mempalace/"
done
# Copy the package directory if it exists
if [ -d "$NEIL_HOME/memory/mempalace/mempalace" ]; then
    cp -r "$NEIL_HOME/memory/mempalace/mempalace" "$STAGING/memory/mempalace/"
fi
ok "MemPalace package"

# ── Services (registry only, NOT vault) ──────────────────────────────
mkdir -p "$STAGING/services/registry"
cp "$NEIL_HOME/services/handler.sh" "$STAGING/services/"
if [ -d "$NEIL_HOME/services/registry" ]; then
    cp "$NEIL_HOME"/services/registry/*.md "$STAGING/services/registry/" 2>/dev/null || true
fi
ok "Services"

# ── Input watchers ────────────────────────────────────────────────────
mkdir -p "$STAGING/inputs/watchers"
for W in filesystem.sh schedule.sh webhook.sh vision_inbox.sh; do
    [ -f "$NEIL_HOME/inputs/watchers/$W" ] && \
        cp "$NEIL_HOME/inputs/watchers/$W" "$STAGING/inputs/watchers/"
done
ok "Input watchers"

# ── Output channels ──────────────────────────────────────────────────
mkdir -p "$STAGING/outputs/channels"
for CH in terminal.sh file.sh email.sh slack.sh; do
    [ -f "$NEIL_HOME/outputs/channels/$CH" ] && \
        cp "$NEIL_HOME/outputs/channels/$CH" "$STAGING/outputs/channels/"
done
ok "Output channels"

# ── Self (scripts + lessons, not runtime state) ─────────────────────
mkdir -p "$STAGING/self"
for S in self_check.sh snapshot.sh verify.sh lessons.md; do
    [ -f "$NEIL_HOME/self/$S" ] && cp "$NEIL_HOME/self/$S" "$STAGING/self/"
done
ok "Self scripts"

# ── Plugins (framework only) ────────────────────────────────────────
mkdir -p "$STAGING/plugins/installed" "$STAGING/plugins/available"
[ -f "$NEIL_HOME/plugins/install.sh" ] && \
    cp "$NEIL_HOME/plugins/install.sh" "$STAGING/plugins/"
ok "Plugins framework"

# ── Mirror (script only, no remote data) ────────────────────────────
mkdir -p "$STAGING/mirror"
[ -f "$NEIL_HOME/mirror/sync.sh" ] && \
    cp "$NEIL_HOME/mirror/sync.sh" "$STAGING/mirror/"
ok "Mirror script"

# ── Vision (script only, no captures) ───────────────────────────────
mkdir -p "$STAGING/vision/inbox" "$STAGING/vision/captures"
[ -f "$NEIL_HOME/vision/capture.sh" ] && \
    cp "$NEIL_HOME/vision/capture.sh" "$STAGING/vision/"
ok "Vision script"

# ── Blueprint TUI (source only, no target/) ─────────────────────────
if [ -d "$NEIL_HOME/blueprint/src" ]; then
    mkdir -p "$STAGING/blueprint/src"
    cp -r "$NEIL_HOME/blueprint/src/"* "$STAGING/blueprint/src/"
    [ -f "$NEIL_HOME/blueprint/Cargo.toml" ] && \
        cp "$NEIL_HOME/blueprint/Cargo.toml" "$STAGING/blueprint/"
    [ -f "$NEIL_HOME/blueprint/Cargo.lock" ] && \
        cp "$NEIL_HOME/blueprint/Cargo.lock" "$STAGING/blueprint/"
    ok "Blueprint TUI source"
fi

# ── Top-level files ──────────────────────────────────────────────────
cp "$NEIL_HOME/install.sh" "$STAGING/"
[ -f "$NEIL_HOME/config.toml" ] && cp "$NEIL_HOME/config.toml" "$STAGING/"
[ -f "$NEIL_HOME/QUICKSTART.md" ] && cp "$NEIL_HOME/QUICKSTART.md" "$STAGING/"
[ -f "$NEIL_HOME/README.md" ] && cp "$NEIL_HOME/README.md" "$STAGING/"
[ -f "$NEIL_HOME/.gitignore" ] && cp "$NEIL_HOME/.gitignore" "$STAGING/"
ok "Top-level files"

# ── Create tarball ────────────────────────────────────────────────────
info "Creating tarball..."

cd /tmp
tar -czf "$OUTFILE" "$OUTNAME"

# Move to caller's original directory or home
DEST="${OLDPWD:-$HOME}/$OUTFILE"
mv "/tmp/$OUTFILE" "$DEST" 2>/dev/null || {
    DEST="$HOME/$OUTFILE"
    mv "/tmp/$OUTFILE" "$DEST"
}

# ── Cleanup staging ──────────────────────────────────────────────────
rm -rf "$STAGING"

# ── Summary ──────────────────────────────────────────────────────────
SIZE=$(du -h "$DEST" | cut -f1)
FILE_COUNT=$(tar -tzf "$DEST" | wc -l)

echo ""
info "====================================="
info "  openclaw v${VERSION} packaged!"
info "====================================="
info "File:  $DEST"
info "Size:  $SIZE"
info "Files: $FILE_COUNT"
echo ""
info "To install on a new machine:"
info "  tar xzf $OUTFILE"
info "  cd $OUTNAME"
info "  sh install.sh"
echo ""
info ":D"
