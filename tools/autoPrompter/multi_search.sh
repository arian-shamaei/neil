#!/bin/bash
# multi_search.sh — Multi-query memory retrieval
#
# Instead of searching mempalace with a single query, this script
# generates 2 queries from different signals and merges results.
#
# Signal 1: Primary query (from extract_query — last beat's question/summary)
# Signal 2: Keywords from the current prompt file (content-based)
#
# Deduplicates by source file to avoid showing the same note twice.
# Returns merged results, primary query first (higher relevance).
#
# Usage: multi_search.sh <palace_path> <venv_activate> <primary_query> [prompt_file]

set -euo pipefail

PALACE="$1"
VENV="$2"
PRIMARY_QUERY="$3"
PROMPT_FILE="${4:-}"

# Activate mempalace venv
. "$VENV" 2>/dev/null

# --- Search 1: Primary query (always runs) ---
PRIMARY_RESULTS=""
if [ -n "$PRIMARY_QUERY" ]; then
    PRIMARY_RESULTS=$(mempalace --palace "$PALACE" search "$PRIMARY_QUERY" --results 3 2>/dev/null) || true
fi

# --- Search 2: Extract keywords from prompt file ---
SECONDARY_RESULTS=""
if [ -n "$PROMPT_FILE" ] && [ -f "$PROMPT_FILE" ]; then
    # Extract meaningful words from prompt: skip common markdown/stop words,
    # take the most distinctive terms
    KEYWORDS=$(sed -e 's/#//g' -e 's/[^a-zA-Z ]/ /g' "$PROMPT_FILE" \
        | tr ' ' '\n' \
        | awk 'length >= 5' \
        | sort | uniq -c | sort -rn \
        | grep -viE '^[[:space:]]*[0-9]+[[:space:]]+(the|this|that|with|from|have|been|will|your|about|which|their|would|could|should|these|those|other|after|before|being|every|where|there|still|here|into|some|what|when|than|then|them|they|more|most|also|each|just|like|make|over|such|only|very|first|through|between|under|again|does|done|during|while|since|until|above|below|never|always|often|maybe|might|shall|shall|check|phase|below|above|using|based|cycle|prompt|action|memory|status|print|start|write|field|lines|queue|queue|notes|system|heartbeat|observe|reason|report|rules)' \
        | head -5 \
        | awk '{print $2}' \
        | tr '\n' ' ' \
        | sed 's/ *$//')

    if [ -n "$KEYWORDS" ]; then
        SECONDARY_RESULTS=$(mempalace --palace "$PALACE" search "$KEYWORDS" --results 2 2>/dev/null) || true
    fi
fi

# --- Merge results ---
# Print primary results first (these are the most relevant)
if [ -n "$PRIMARY_RESULTS" ]; then
    echo "$PRIMARY_RESULTS"
fi

# Append secondary results, but skip notes already shown in primary
if [ -n "$SECONDARY_RESULTS" ] && [ -n "$PRIMARY_RESULTS" ]; then
    # Extract source filenames from primary results to deduplicate
    PRIMARY_SOURCES=$(echo "$PRIMARY_RESULTS" | grep -oP 'Source: \K\S+' || true)

    # Filter secondary results: print blocks that don't match primary sources
    echo "$SECONDARY_RESULTS" | awk -v sources="$PRIMARY_SOURCES" '
    BEGIN {
        n = split(sources, arr, "\n")
        for (i = 1; i <= n; i++) seen[arr[i]] = 1
    }
    /Source:/ {
        match($0, /Source: ([^ ]+)/, m)
        skip = (m[1] in seen)
    }
    /^\[/ { skip = 0 }  # Reset on new result block
    /Source:/ {
        match($0, /Source: ([^ ]+)/, m)
        if (m[1] in seen) skip = 1
    }
    !skip { print }
    '
elif [ -n "$SECONDARY_RESULTS" ]; then
    echo "$SECONDARY_RESULTS"
fi
