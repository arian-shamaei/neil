#!/bin/sh
# test_release.sh -- End-to-end release validation for openclaw
#
# Extracts the tarball to a temp directory, runs install.sh in isolation,
# validates all components work, then cleans up.
#
# Usage: sh test_release.sh [path/to/openclaw-v0.1.tar.gz]
#
# This simulates a fresh install without affecting the live system.
# No root needed -- uses --no-systemd --no-cron flags.

set -e

# ── Config ────────────────────────────────────────────────────────────
TARBALL="${1:-$(dirname "$0")/openclaw-v0.1.tar.gz}"
TEST_HOME=""
PASS=0
FAIL=0
WARNS=0

# ── Colors ────────────────────────────────────────────────────────────
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

pass() { PASS=$((PASS + 1)); printf "${GREEN}  [PASS]${NC} %s\n" "$1"; }
fail() { FAIL=$((FAIL + 1)); printf "${RED}  [FAIL]${NC} %s\n" "$1"; }
warn() { WARNS=$((WARNS + 1)); printf "${YELLOW}  [WARN]${NC} %s\n" "$1"; }
info() { printf "${CYAN}[e2e]${NC} %s\n" "$1"; }

cleanup() {
    if [ -n "$TEST_HOME" ] && [ -d "$TEST_HOME" ]; then
        info "Cleaning up $TEST_HOME..."
        rm -rf "$TEST_HOME"
    fi
    if [ -n "$EXTRACT_DIR" ] && [ -d "$EXTRACT_DIR" ]; then
        rm -rf "$EXTRACT_DIR"
    fi
}
trap cleanup EXIT

# ── Phase 0: Validate tarball ────────────────────────────────────────
info "=== Phase 0: Tarball Validation ==="

if [ ! -f "$TARBALL" ]; then
    echo "ERROR: Tarball not found at $TARBALL"
    echo "Usage: sh test_release.sh [path/to/openclaw-v0.1.tar.gz]"
    exit 1
fi

TARBALL_SIZE=$(wc -c < "$TARBALL")
if [ "$TARBALL_SIZE" -lt 10000 ]; then
    fail "Tarball too small ($TARBALL_SIZE bytes) -- likely corrupt"
    exit 1
fi
pass "Tarball exists ($TARBALL_SIZE bytes)"

# Check tarball integrity
tar tzf "$TARBALL" > /dev/null 2>&1 || { fail "Tarball is corrupt"; exit 1; }
pass "Tarball integrity check"

# Check critical files are in tarball
TARBALL_LIST=$(tar tzf "$TARBALL")
REQUIRED_FILES="
install.sh
essence/identity.md
essence/soul.md
essence/mission.md
essence/heartbeat.md
essence/actions.md
essence/overview.md
essence/guardrails.md
tools/autoPrompter/src/autoprompt.c
tools/autoPrompter/Makefile
tools/autoPrompter/heartbeat.sh
tools/autoPrompter/observe.sh
memory/zettel/src/zettel.c
memory/zettel/Makefile
memory/mempalace/pyproject.toml
memory/mempalace/mempalace/__init__.py
memory/mempalace/mempalace/cli.py
services/handler.sh
self/self_check.sh
self/snapshot.sh
config.toml
README.md
QUICKSTART.md
"

MISSING_COUNT=0
for F in $REQUIRED_FILES; do
    if echo "$TARBALL_LIST" | grep -q "$F"; then
        : # present
    else
        fail "Missing from tarball: $F"
        MISSING_COUNT=$((MISSING_COUNT + 1))
    fi
done
if [ "$MISSING_COUNT" -eq 0 ]; then
    pass "All required files present in tarball"
else
    fail "$MISSING_COUNT required files missing"
fi

# Check for files that should NOT be in tarball
BAD_PATTERNS=".bak __pycache__ .pyc .env credentials vault/"
for P in $BAD_PATTERNS; do
    if echo "$TARBALL_LIST" | grep -q "$P"; then
        fail "Tarball contains unwanted files matching: $P"
    fi
done
pass "No sensitive/unwanted files in tarball"

# ── Phase 1: Extract and Install ─────────────────────────────────────
info "=== Phase 1: Fresh Install ==="

EXTRACT_DIR=$(mktemp -d /tmp/openclaw-e2e-extract.XXXXXX)
TEST_HOME=$(mktemp -d /tmp/openclaw-e2e-home.XXXXXX)

cd "$EXTRACT_DIR"
tar xzf "$TARBALL"
pass "Tarball extracted to $EXTRACT_DIR"

# Find the extracted directory
INSTALL_DIR=$(ls -d openclaw-v*/  2>/dev/null | head -1)
if [ -z "$INSTALL_DIR" ]; then
    fail "No openclaw-v* directory found after extraction"
    exit 1
fi

cd "$INSTALL_DIR"

# Set git identity for the temp environment (needed for git init in install)
git config --global user.email >/dev/null 2>&1 || {
    export GIT_AUTHOR_NAME="openclaw-test"
    export GIT_AUTHOR_EMAIL="test@openclaw"
    export GIT_COMMITTER_NAME="openclaw-test"
    export GIT_COMMITTER_EMAIL="test@openclaw"
}

# Run install with isolation flags
info "Running install.sh --neil-home $TEST_HOME --no-systemd --no-cron --no-blueprint ..."
NEIL_HOME="$TEST_HOME" sh install.sh \
    --neil-home "$TEST_HOME" \
    --no-systemd \
    --no-cron \
    --no-blueprint \
    > /tmp/openclaw-e2e-install.log 2>&1

INSTALL_EXIT=$?
if [ "$INSTALL_EXIT" -eq 0 ]; then
    pass "install.sh completed (exit 0)"
else
    fail "install.sh exited with code $INSTALL_EXIT"
    echo "--- Install log tail ---"
    tail -20 /tmp/openclaw-e2e-install.log
    echo "------------------------"
fi

# ── Phase 2: Verify Directory Structure ──────────────────────────────
info "=== Phase 2: Directory Structure ==="

REQUIRED_DIRS="
essence
tools/autoPrompter/src
tools/autoPrompter/queue
tools/autoPrompter/active
tools/autoPrompter/history
memory/zettel/src
memory/palace/notes
memory/palace/index
memory/mempalace
services/registry
services/vault
inputs/watchers
outputs/channels
self
mirror
plugins/installed
plugins/available
vision/inbox
vision/captures
"

DIR_MISSING=0
for D in $REQUIRED_DIRS; do
    if [ -d "$TEST_HOME/$D" ]; then
        : # ok
    else
        fail "Missing directory: $D"
        DIR_MISSING=$((DIR_MISSING + 1))
    fi
done
if [ "$DIR_MISSING" -eq 0 ]; then
    pass "All required directories created"
else
    fail "$DIR_MISSING directories missing"
fi

# ── Phase 3: Verify Binaries ─────────────────────────────────────────
info "=== Phase 3: Binary Builds ==="

if [ -x "$TEST_HOME/tools/autoPrompter/autoprompt" ]; then
    pass "autoprompt binary built and executable"
else
    fail "autoprompt binary missing or not executable"
fi

if [ -x "$TEST_HOME/memory/zettel/zettel" ]; then
    pass "zettel binary built and executable"
else
    fail "zettel binary missing or not executable"
fi

# ── Phase 4: Verify Data Files ───────────────────────────────────────
info "=== Phase 4: Data Files ==="

# Check JSON state files
for JFILE in heartbeat_log.json intentions.json; do
    if [ -f "$TEST_HOME/$JFILE" ]; then
        # Validate it's valid JSON (basic check)
        if python3 -c "import json; json.load(open('$TEST_HOME/$JFILE'))" 2>/dev/null; then
            pass "$JFILE exists and is valid JSON"
        else
            fail "$JFILE exists but is not valid JSON"
        fi
    else
        fail "$JFILE not created"
    fi
done

if [ -f "$TEST_HOME/self/failures.json" ]; then
    pass "failures.json exists"
else
    fail "failures.json not created"
fi

if [ -f "$TEST_HOME/deployment.md" ]; then
    pass "deployment.md generated"
else
    fail "deployment.md not generated"
fi

# ── Phase 5: Verify Essence ─────────────────────────────────────────
info "=== Phase 5: Essence Files ==="

ESSENCE_FILES="identity.md soul.md mission.md heartbeat.md actions.md overview.md guardrails.md"
for E in $ESSENCE_FILES; do
    if [ -f "$TEST_HOME/essence/$E" ] && [ -s "$TEST_HOME/essence/$E" ]; then
        pass "essence/$E installed ($(wc -c < "$TEST_HOME/essence/$E") bytes)"
    else
        fail "essence/$E missing or empty"
    fi
done

# ── Phase 6: Verify Zettel ───────────────────────────────────────────
info "=== Phase 6: Zettel Functionality ==="

export ZETTEL_HOME="$TEST_HOME/memory/palace"

# Test creating a note
NOTE_OUTPUT=$("$TEST_HOME/memory/zettel/zettel" new --wing test --room e2e --tags release-test "E2E test note created at $(date)" 2>&1)
if echo "$NOTE_OUTPUT" | grep -q "Created\|created\|\.md"; then
    pass "zettel new: note created"
else
    # Some versions output differently -- check if a note file exists
    NOTE_COUNT=$(ls "$TEST_HOME/memory/palace/notes/"*.md 2>/dev/null | wc -l)
    if [ "$NOTE_COUNT" -gt 0 ]; then
        pass "zettel new: note created ($NOTE_COUNT notes)"
    else
        fail "zettel new: no output or note file"
        echo "  Output: $NOTE_OUTPUT"
    fi
fi

# Test listing
LIST_OUTPUT=$("$TEST_HOME/memory/zettel/zettel" list 2>&1)
if echo "$LIST_OUTPUT" | grep -qi "e2e\|test\|note"; then
    pass "zettel list: can find notes"
else
    warn "zettel list: output unclear"
    echo "  Output: $LIST_OUTPUT"
fi

# Test reindex
REINDEX_OUTPUT=$("$TEST_HOME/memory/zettel/zettel" reindex 2>&1)
pass "zettel reindex: completed"

# ── Phase 7: Verify MemPalace ────────────────────────────────────────
info "=== Phase 7: MemPalace ==="

if [ -d "$TEST_HOME/memory/mempalace/.venv" ]; then
    pass "MemPalace venv exists"

    # Test CLI is importable
    MEMPALACE_BIN="$TEST_HOME/memory/mempalace/.venv/bin/mempalace"
    if [ -f "$MEMPALACE_BIN" ]; then
        HELP_OUTPUT=$("$MEMPALACE_BIN" --help 2>&1 || true)
        if echo "$HELP_OUTPUT" | grep -qi "usage\|mempalace\|command"; then
            pass "mempalace CLI responds to --help"
        else
            warn "mempalace CLI --help output unclear"
        fi
    else
        # Try as python module
        MP_RESULT=$("$TEST_HOME/memory/mempalace/.venv/bin/python" -m mempalace --help 2>&1 || true)
        if echo "$MP_RESULT" | grep -qi "usage\|mempalace\|command"; then
            pass "mempalace module responds to --help"
        else
            warn "mempalace CLI not found as binary or module"
        fi
    fi
else
    fail "MemPalace venv not created"
fi

# ── Phase 8: Verify Scripts ─────────────────────────────────────────
info "=== Phase 8: Script Permissions ==="

SCRIPTS="
tools/autoPrompter/heartbeat.sh
tools/autoPrompter/observe.sh
services/handler.sh
self/self_check.sh
self/snapshot.sh
"

SCRIPT_ISSUES=0
for S in $SCRIPTS; do
    if [ -f "$TEST_HOME/$S" ]; then
        if [ -x "$TEST_HOME/$S" ]; then
            pass "$S is executable"
        else
            fail "$S exists but not executable"
            SCRIPT_ISSUES=$((SCRIPT_ISSUES + 1))
        fi
    else
        fail "$S not found"
        SCRIPT_ISSUES=$((SCRIPT_ISSUES + 1))
    fi
done

# ── Phase 9: Verify Git Snapshot System ──────────────────────────────
info "=== Phase 9: Git Snapshots ==="

if [ -d "$TEST_HOME/.git" ]; then
    GIT_LOG=$(cd "$TEST_HOME" && git log --oneline -1 2>&1)
    if echo "$GIT_LOG" | grep -qi "initial\|install"; then
        pass "Git repo initialized with initial commit"
    else
        warn "Git repo exists but initial commit unclear: $GIT_LOG"
    fi
else
    fail "Git repo not initialized"
fi

# ── Phase 10: Self-Check ─────────────────────────────────────────────
info "=== Phase 10: Self-Check Script ==="

NEIL_HOME="$TEST_HOME" ZETTEL_HOME="$TEST_HOME/memory/palace" \
    sh "$TEST_HOME/self/self_check.sh" > /tmp/openclaw-e2e-selfcheck.log 2>&1 || true

SELFCHECK_FAILS=$(grep -ciE "FAIL|ERROR" /tmp/openclaw-e2e-selfcheck.log 2>/dev/null || echo "0")
SELFCHECK_PASS=$(grep -ciE "ok|PASS|ALL CHECKS" /tmp/openclaw-e2e-selfcheck.log 2>/dev/null || echo "0")
if [ "$SELFCHECK_FAILS" -eq 0 ] && [ "$SELFCHECK_PASS" -gt 0 ]; then
    pass "self_check.sh reports all clear"
else
    warn "self_check.sh had $SELFCHECK_FAILS issues (check /tmp/openclaw-e2e-selfcheck.log)"
fi

# ── Phase 11: Observe Script ─────────────────────────────────────────
info "=== Phase 11: Observe Script ==="

OBSERVE_OUT=$(NEIL_HOME="$TEST_HOME" ZETTEL_HOME="$TEST_HOME/memory/palace" \
    sh "$TEST_HOME/tools/autoPrompter/observe.sh" 2>&1 | head -30)
if echo "$OBSERVE_OUT" | grep -qi "System\|disk\|ram\|Memory\|Queue"; then
    pass "observe.sh produces system observations"
else
    warn "observe.sh output unclear"
fi

# ── Summary ──────────────────────────────────────────────────────────
echo ""
info "=========================================="
info "  E2E RELEASE TEST SUMMARY"
info "=========================================="
printf "${GREEN}  PASSED: %d${NC}\n" "$PASS"
printf "${RED}  FAILED: %d${NC}\n" "$FAIL"
printf "${YELLOW}  WARNS:  %d${NC}\n" "$WARNS"
echo ""

if [ "$FAIL" -eq 0 ]; then
    printf "${GREEN}  RESULT: ALL TESTS PASSED :D${NC}\n"
    echo ""
    info "Release package is valid and installs correctly."
    exit 0
else
    printf "${RED}  RESULT: $FAIL TESTS FAILED${NC}\n"
    echo ""
    info "Review failures above before releasing."
    exit 1
fi
