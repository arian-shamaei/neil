#!/bin/sh
# tui_testbench.sh -- Digital twin testbench for neil-blueprint
MODE="${1:-idle}"
NEIL_HOME="${NEIL_HOME:-$HOME/.neil}"
SESSION="neil_bench"
WIDTH=120
HEIGHT=42
CAP="/tmp/neil_bench"
mkdir -p "$CAP"

tmux kill-session -t "$SESSION" 2>/dev/null
sleep 1

echo "=== NEIL BLUEPRINT TESTBENCH ==="
echo "Mode: $MODE | Size: ${WIDTH}x${HEIGHT}"

tmux new-session -d -s "$SESSION" -x "$WIDTH" -y "$HEIGHT" \
    "NEIL_HOME=$NEIL_HOME $HOME/.local/bin/neil-blueprint"
sleep 3

cap() { tmux capture-pane -t "$SESSION" -p > "$CAP/${1}.txt"; }

echo ""
echo "=== STATIC LAYOUT ==="
cap static
F="$CAP/static.txt"

check() {
    if grep -q "$2" "$F"; then echo "  PASS: $1"
    else echo "  FAIL: $1"; fi
}

head -1 "$F" | grep -q "NEIL" && echo "  PASS: header on line 1" || echo "  FAIL: header not on line 1"
tail -1 "$F" | grep -q "fps" && echo "  PASS: fps on last line" || echo "  FAIL: fps not on last line"
check "sidebar NEIL box" "NEIL"
check "sidebar memory" "memory"
check "sidebar intents" "intents"
check "seal braille art" "[⣿⣶⣴]"
check "speech bubble" "◃"
check "water waves" "[≈∼∿]"
check "input box" "> ─"
check "input cursor" "│_"

echo ""
echo "=== SIDEBAR ORDER (right column) ==="
cat "$F" | while IFS= read -r line; do
    right=$(echo "$line" | rev | cut -c1-28 | rev)
    case "$right" in
        *NEIL*) echo "  -> NEIL status box" ;;
        *memory*) echo "  -> memory box" ;;
        *intents*) echo "  -> intents box" ;;
        *"◃"*) echo "  -> speech bubble" ;;
        *"⣿"*) echo "  -> seal body" ;;
        *"≈"*|*"∼"*|*"∿"*) echo "  -> water line" ;;
        *"~ neil ~"*|*"~ done ~"*) echo "  -> seal label" ;;
    esac
done | uniq

echo ""
echo "=== PROBLEM DETECTION ==="

# Check for "sending to neil" placement
SEND_LINES=$(grep -n "sending to neil" "$F")
if [ -n "$SEND_LINES" ]; then
    echo "  FOUND: 'sending to neil' indicators:"
    echo "$SEND_LINES" | while read line; do echo "    $line"; done
    echo "  (should only appear once, near the input box)"
else
    echo "  OK: no stale 'sending' indicators"
fi

# Check for duplicate neil entries
echo "  Duplicate check:"
prev=""
grep "^ neil " "$F" | while IFS= read -r line; do
    if [ "$line" = "$prev" ]; then
        echo "    DUPLICATE: $line"
    fi
    prev="$line"
done
echo "    (blank = no duplicates)"

# Check loading/working indicator position
echo "  Loading indicator:"
head -1 "$F" | grep -q "working\|o>" && echo "    Header: WORKING" || echo "    Header: idle"
grep -n "sending to neil" "$F" | while read line; do
    num=$(echo "$line" | cut -d: -f1)
    total=$(wc -l < "$F")
    echo "    'sending' at line $num of $total"
done

echo ""
echo "=== ANIMATION (5 frames) ==="
for i in 1 2 3 4 5; do
    cap "anim_$i"
    sleep 0.5
done

W1=$(grep "[≈∼∿]" "$CAP/anim_1.txt" | head -1)
W3=$(grep "[≈∼∿]" "$CAP/anim_3.txt" | head -1)
[ "$W1" != "$W3" ] && echo "  PASS: water animates" || echo "  FAIL: water static"

T1=$(head -1 "$CAP/anim_1.txt" | grep -o "[0-9][0-9]:[0-9][0-9]:[0-9][0-9]")
T5=$(head -1 "$CAP/anim_5.txt" | grep -o "[0-9][0-9]:[0-9][0-9]:[0-9][0-9]")
[ "$T1" != "$T5" ] && echo "  PASS: clock updates ($T1 -> $T5)" || echo "  WARN: same second"

F1=$(grep -o "[0-9]*fps" "$CAP/anim_1.txt")
F5=$(grep -o "[0-9]*fps" "$CAP/anim_5.txt")
echo "  FPS: $F1 -> $F5"

echo ""
echo "=== FULL FRAME ==="
cap final
cat "$CAP/final.txt"

echo ""
echo "=== TESTBENCH COMPLETE ==="
tmux kill-session -t "$SESSION" 2>/dev/null
