#!/bin/bash
# phase1_readiness.sh -- orchestrator-scoped readiness audit for humanizer Phase 1
# Aggregates phase1_*.sh verify scripts + infrastructure dimensions into single gate.
# Emits state/readiness.json, exits 0 iff all dimensions pass.

set -uo pipefail

NEIL="${NEIL_HOME:-$HOME/.neil}"
PROJECT_DIR="$NEIL/projects/humanizer"
VERIFY_DIR="$NEIL/self/verify/humanizer"
STATE_DIR="$PROJECT_DIR/state"
STATE_FILE="$STATE_DIR/readiness.json"

mkdir -p "$STATE_DIR"

all_pass=1
results_json=""

add_result() {
    local key="$1"
    local value="$2"
    local is_num="${3:-0}"
    if [ -n "$results_json" ]; then
        results_json="${results_json},"
    fi
    if [ "$is_num" = "1" ]; then
        results_json="${results_json}\n    \"$key\": $value"
    else
        results_json="${results_json}\n    \"$key\": \"$value\""
    fi
}

# Dimension (a): Phase 1 peer verify scripts exist + executable + pass
for phase in style_extractor edit_scorer semantic_fidelity detector_calibration; do
    script="$VERIFY_DIR/phase1_${phase}.sh"
    if [ ! -f "$script" ]; then
        add_result "verify_${phase}" "missing"
        all_pass=0
    elif [ ! -x "$script" ]; then
        add_result "verify_${phase}" "not_executable"
        all_pass=0
    else
        if "$script" >/dev/null 2>&1; then
            add_result "verify_${phase}" "pass"
        else
            add_result "verify_${phase}" "fail"
            all_pass=0
        fi
    fi
done

# Dimension (b): Author corpus present with sufficient paragraphs
CORPUS="$PROJECT_DIR/author_corpus/mamishev_clean.jsonl"
MIN_LINES=50
if [ -f "$CORPUS" ]; then
    lines=$(wc -l < "$CORPUS" 2>/dev/null || echo 0)
    add_result "corpus_line_count" "$lines" 1
    if [ "$lines" -ge "$MIN_LINES" ]; then
        add_result "corpus_paragraphs" "pass"
    else
        add_result "corpus_paragraphs" "fail"
        all_pass=0
    fi
else
    add_result "corpus_line_count" "0" 1
    add_result "corpus_paragraphs" "missing"
    all_pass=0
fi

# Dimension (c): spawn_vm service registered (canonical format is .md with YAML frontmatter)
SPAWN_REG_MD="$NEIL/services/registry/spawn_vm.md"
SPAWN_REG_JSON="$NEIL/services/registry/spawn_vm.json"
SPAWN_REG_DIR="$NEIL/services/registry/spawn_vm"
if [ -f "$SPAWN_REG_MD" ] || [ -f "$SPAWN_REG_JSON" ] || [ -d "$SPAWN_REG_DIR" ]; then
    add_result "spawn_vm_registered" "pass"
else
    add_result "spawn_vm_registered" "missing"
    all_pass=0
fi

# Dimension (d): Project structure (SPEC.md, SEED_PROMPT.md)
for f in SPEC.md SEED_PROMPT.md; do
    if [ -f "$PROJECT_DIR/$f" ]; then
        add_result "doc_${f%.md}" "pass"
    else
        add_result "doc_${f%.md}" "missing"
        all_pass=0
    fi
done

# Emit state JSON
if [ "$all_pass" = "1" ]; then
    all_pass_json="true"
else
    all_pass_json="false"
fi

printf "{\n  \"timestamp\": \"%s\",\n  \"all_pass\": %s,\n  \"dimensions\": {%b\n  }\n}\n" \
    "$(date -Iseconds)" \
    "$all_pass_json" \
    "$results_json" > "$STATE_FILE"

if [ "$all_pass" = "1" ]; then
    exit 0
else
    exit 1
fi