#!/usr/bin/env bash
#
# run-scrninfo.sh <kernel8.img> <sd.img> <serial.log> [seconds]
#
# Headless QEMU raspi4b with the seeded card. The host has no `timeout`, so
# QEMU runs in the background and is stopped after the probe's summary line
# ("SCRNINFO: ...") reached the serial log, or after <seconds> (default 90).
set -uo pipefail
K="$1"; SD="$2"; LOG="$3"; LIMIT="${4:-90}"
rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial file:"$LOG" \
  -kernel "$K" -drive file="$SD",if=sd,format=raw &
QPID=$!
t=0
while [ "$t" -lt "$LIMIT" ]; do
    kill -0 "$QPID" 2>/dev/null || break
    if grep -q 'SCRNINFO: ' "$LOG" 2>/dev/null; then
        sleep 3   # let the program's exit and the file close settle
        break
    fi
    sleep 1; t=$((t + 1))
done
kill "$QPID" 2>/dev/null; sleep 1; kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
echo "run: ${t}s, summary line: $(grep -a 'SCRNINFO: ' "$LOG" | head -1)"
