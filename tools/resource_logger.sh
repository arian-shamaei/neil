#!/bin/bash
# Resource logger: appends system stats every 60s from 17:21 to 18:21
LOGFILE="${NEIL_HOME:-$HOME/.neil}/long-term-prompt.txt"
END_TIME=$(date -d "2026-04-13 18:21:00" +%s)

# Wait until 17:21 if we're early
TARGET_START=$(date -d "2026-04-13 17:21:00" +%s)
NOW=$(date +%s)
if [ "$NOW" -lt "$TARGET_START" ]; then
    sleep $(( TARGET_START - NOW ))
fi

while true; do
    NOW=$(date +%s)
    if [ "$NOW" -ge "$END_TIME" ]; then
        echo "" >> "$LOGFILE"
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] === LOGGING COMPLETE ===" >> "$LOGFILE"
        break
    fi

    {
        echo ""
        echo "[$(date '+%Y-%m-%d %H:%M:%S')]"
        echo "  uptime: $(uptime)"
        echo "  memory: $(free -h | awk '/^Mem:/{print "used=" $3 " free=" $4 " available=" $7}')"
        echo "  disk:   $(df -h / | awk 'NR==2{print "used=" $3 " avail=" $4 " use%=" $5}')"
        echo "  load:   $(cat /proc/loadavg | awk '{print $1, $2, $3}')"
        echo "  procs:  $(ps aux | wc -l) processes"
    } >> "$LOGFILE"

    sleep 60
done
