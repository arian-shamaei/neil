#!/bin/sh
# usws_consolidate.sh -- Unihemispheric Slow-Wave Sleep for Neil
#
# Inspired by seal biology: real seals sleep with one brain hemisphere
# while the other stays alert for predators. Applied to Neil:
#
#   Alert hemisphere: input watchers, queue monitor, event response
#   Sleep hemisphere: memory consolidation, index maintenance, cleanup
#
# This script IS the sleep hemisphere. Called by adaptive_gate.sh when
# the system is in cool/cold mode and a heartbeat tick is being SKIPPED.
# Instead of doing nothing during skipped ticks, we do background
# maintenance that doesn't require Claude invocation (saves tokens).
#
# Exit codes: 0 = did work, 1 = nothing to do

NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
PALACE="$NEIL_HOME/memory/palace"
ZETTEL="$NEIL_HOME/memory/zettel/zettel"
MEMPALACE_DIR="$NEIL_HOME/memory/mempalace"
LOG="$NEIL_HOME/usws_log.json"
LOCK="/tmp/usws_consolidate.lock"

# Prevent concurrent runs
if [ -f "$LOCK" ]; then
    LOCK_AGE=$(( $(date +%s) - $(stat -c %Y "$LOCK" 2>/dev/null || echo 0) ))
    if [ "$LOCK_AGE" -lt 300 ]; then
        echo "[usws] skipping: another consolidation in progress"
        exit 1
    fi
    rm -f "$LOCK"
fi
touch "$LOCK"
trap 'rm -f "$LOCK"' EXIT

WORK_DONE=0
SUMMARY=""

# --- Task 1: Rebuild zettel index if stale ---
# rooms.idx might be out of sync with notes
if [ -x "$ZETTEL" ] && [ -d "$PALACE/notes" ]; then
    export ZETTEL_HOME="$PALACE"
    NOTE_COUNT=$(ls "$PALACE/notes/" 2>/dev/null | wc -l)
    INDEX_LINES=$(wc -l < "$PALACE/index/rooms.idx" 2>/dev/null || echo 0)

    # If note count diverges significantly from index, rebuild
    if [ "$NOTE_COUNT" -gt 0 ] && [ "$INDEX_LINES" -lt "$NOTE_COUNT" ]; then
        "$ZETTEL" reindex >/dev/null 2>&1
        WORK_DONE=$((WORK_DONE + 1))
        SUMMARY="${SUMMARY}reindexed zettel ($NOTE_COUNT notes); "
    fi
fi

# --- Task 2: Find orphaned notes (no links, no tags) ---
# These are knowledge fragments that could be better connected
ORPHANS=""
ORPHAN_COUNT=0
if [ -d "$PALACE/notes" ]; then
    for NOTE in "$PALACE/notes"/*.md; do
        [ -f "$NOTE" ] || continue
        # Check if note has non-empty links or tags
        HAS_LINKS=0
        HAS_TAGS=0
        grep -q '^links: \[.\+\]' "$NOTE" 2>/dev/null && HAS_LINKS=1
        grep -q '^tags: \[.\+\]' "$NOTE" 2>/dev/null && HAS_TAGS=1
        if [ "$HAS_LINKS" -eq 0 ] && [ "$HAS_TAGS" -eq 0 ]; then
            ORPHAN_COUNT=$((ORPHAN_COUNT + 1))
            ORPHANS="${ORPHANS}$(basename "$NOTE") "
        fi
    done
    if [ "$ORPHAN_COUNT" -gt 0 ]; then
        WORK_DONE=$((WORK_DONE + 1))
        SUMMARY="${SUMMARY}found $ORPHAN_COUNT orphan notes; "
    fi
fi

# --- Task 3: Sync mempalace index with notes ---
# Check if new notes exist that aren't indexed
if [ -d "$MEMPALACE_DIR" ] && [ -d "$PALACE/notes" ]; then
    INDEXED_COUNT=0
    # mempalace stores count in its metadata
    if [ -f "$MEMPALACE_DIR/.last_index_count" ]; then
        INDEXED_COUNT=$(cat "$MEMPALACE_DIR/.last_index_count" 2>/dev/null || echo 0)
    fi
    ACTUAL_COUNT=$(ls "$PALACE/notes/" 2>/dev/null | wc -l)

    DRIFT=$((ACTUAL_COUNT - INDEXED_COUNT))
    if [ "$DRIFT" -gt 3 ]; then
        # More than 3 unindexed notes -- trigger reindex
        if [ -x "$MEMPALACE_DIR/mine.sh" ]; then
            "$MEMPALACE_DIR/mine.sh" >/dev/null 2>&1
            echo "$ACTUAL_COUNT" > "$MEMPALACE_DIR/.last_index_count"
            WORK_DONE=$((WORK_DONE + 1))
            SUMMARY="${SUMMARY}reindexed mempalace (+$DRIFT notes); "
        fi
    fi
fi

# --- Task 4: Heartbeat log rotation ---
# If log has 200+ entries, archive older ones
if [ -f "$NEIL_HOME/heartbeat_log.json" ]; then
    LOG_LINES=$(wc -l < "$NEIL_HOME/heartbeat_log.json")
    if [ "$LOG_LINES" -gt 200 ]; then
        ARCHIVE="$NEIL_HOME/heartbeat_log.archive.json"
        # Move all but last 50 to archive
        KEEP=50
        ARCHIVE_COUNT=$((LOG_LINES - KEEP))
        head -"$ARCHIVE_COUNT" "$NEIL_HOME/heartbeat_log.json" >> "$ARCHIVE"
        tail -"$KEEP" "$NEIL_HOME/heartbeat_log.json" > "$NEIL_HOME/heartbeat_log.json.tmp"
        mv "$NEIL_HOME/heartbeat_log.json.tmp" "$NEIL_HOME/heartbeat_log.json"
        WORK_DONE=$((WORK_DONE + 1))
        SUMMARY="${SUMMARY}archived $ARCHIVE_COUNT old beats; "
    fi
fi

# --- Task 5: Stale lock cleanup ---
# Remove locks older than 10 minutes (indicates crashed processes)
for LOCKFILE in /tmp/neil_*.lock /tmp/usws_*.lock; do
    [ -f "$LOCKFILE" ] || continue
    LOCK_AGE=$(( $(date +%s) - $(stat -c %Y "$LOCKFILE" 2>/dev/null || echo 0) ))
    if [ "$LOCK_AGE" -gt 600 ]; then
        rm -f "$LOCKFILE"
        WORK_DONE=$((WORK_DONE + 1))
        SUMMARY="${SUMMARY}cleaned stale lock $(basename "$LOCKFILE"); "
    fi
done

# --- Log results ---
if [ "$WORK_DONE" -gt 0 ]; then
    TS=$(date +%Y-%m-%dT%H:%M:%S)
    # Remove trailing "; "
    SUMMARY=$(echo "$SUMMARY" | sed 's/; $//')
    printf '{"timestamp":"%s","tasks":%d,"summary":"%s"}\n' \
        "$TS" "$WORK_DONE" "$SUMMARY" >> "$LOG"
    echo "[usws] completed $WORK_DONE tasks: $SUMMARY"
    exit 0
else
    echo "[usws] nothing to do"
    exit 1
fi
