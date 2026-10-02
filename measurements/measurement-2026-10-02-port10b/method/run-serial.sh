#!/usr/bin/env bash
# run-serial.sh <kernel8.img> <sd.img> <serial.log> <seconds>
# Headless QEMU raspi4b, serial into a file, stopped after <seconds>
# (the host has no `timeout`). For AUTO-folder programs whose verdict is a
# serial line.
set -uo pipefail
K="$1"; SD="$2"; LOG="$3"; SECS="$4"
rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -serial file:"$LOG" \
  -kernel "$K" -drive file="$SD",if=sd,format=raw &
QPID=$!
sleep "$SECS"
kill "$QPID" 2>/dev/null; sleep 1; kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
echo "ran $SECS s -> $LOG ($(wc -c < "$LOG" | tr -d ' ') B)"
