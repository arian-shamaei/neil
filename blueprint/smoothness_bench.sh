#!/bin/sh
# smoothness_bench.sh — drive neil-blueprint through each panel and report
# per-section render-time distributions. Run on server (not over SSH session).
set -eu
SESSION="neil_smooth_bench"
BENCH_FILE="${BENCH_FILE:-/tmp/neil_bench.jsonl}"
HOLD="${HOLD:-15}"
BIN="${BIN:-$HOME/.neil/blueprint/target/release/neil-blueprint}"
W="${W:-120}"; H="${H:-42}"

command -v tmux >/dev/null || { echo 'tmux required'; exit 1; }
[ -x "$BIN" ] || { echo "binary not found: $BIN"; exit 1; }

tmux kill-session -t "$SESSION" 2>/dev/null || true
rm -f "$BENCH_FILE"

echo "[bench] launching blueprint in tmux (${W}x${H}, hold=${HOLD}s per panel)"
tmux new-session -d -s "$SESSION" -x "$W" -y "$H"     "NEIL_HOME=$HOME/.neil NEIL_BLUEPRINT_BENCH=1 NEIL_BLUEPRINT_BENCH_FILE=$BENCH_FILE $BIN"
sleep 3

echo "[bench] == Chat (${HOLD}s) =="
sleep "$HOLD"

echo "[bench] == PanelSelector (2s) =="
tmux send-keys -t "$SESSION" Tab
sleep 2

# idx=0..7 → digit 1..8
for entry in 'Memory 1' 'Heartbeat 2' 'Intentions 3' 'System 4' 'Services 5' 'Failures 6' 'Logs 7' 'Cluster 8'; do
    name="${entry% *}"; digit="${entry##* }"
    echo "[bench] == $name (${HOLD}s) =="
    tmux send-keys -t "$SESSION" "$digit"
    sleep "$HOLD"
done

echo "[bench] cleanup"
tmux send-keys -t "$SESSION" Escape
sleep 1
tmux kill-session -t "$SESSION" 2>/dev/null || true

echo
python3 "$(dirname "$0")/bench_report.py" "$BENCH_FILE"
