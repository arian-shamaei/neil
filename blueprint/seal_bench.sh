#!/bin/sh
# seal_bench.sh -- Live seal testbench
# Runs the seal renderer in isolation. Edit seal.rs, press 'r' in another
# terminal to rebuild, or use this with cargo-watch:
#
# Usage:
#   ./seal_bench.sh          Run the testbench
#   ./seal_bench.sh watch    Auto-rebuild on file change + run

cd "$(dirname "$0")"

if [ "$1" = "watch" ]; then
    echo "Watching seal.rs for changes... (Ctrl+C to stop)"
    echo "Run in another terminal: ssh -t seal@128.95.31.185 'NEIL_HOME=~/.neil ~/.neil/blueprint/target/release/seal_test'"
    while true; do
        source ~/.cargo/env
        cargo build --release --bin seal_test 2>&1 | tail -2
        echo "[$(date +%H:%M:%S)] Built. Waiting for changes..."
        inotifywait -q -e modify src/seal.rs 2>/dev/null || sleep 2
    done
else
    source ~/.cargo/env
    cargo build --release --bin seal_test 2>&1 | tail -2
    NEIL_HOME=~/.neil exec ./target/release/seal_test
fi
