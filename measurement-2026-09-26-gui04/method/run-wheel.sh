#!/usr/bin/env bash
#
# run-wheel.sh <kernel8.img> <card.img> <serial.log> <seconds>
#
# Headless QEMU raspi4b run, serial to a file, bounded by a shell-side kill
# (the host is macOS and has no `timeout`).  No QMP, no screendumps: run A
# reports over Cconws, i.e. over the serial console.  Copied from
# measurement-2026-09-25-port05/method/run-pex.sh.
set -uo pipefail
K="${1:?}"; IMG="${2:?}"; LOG="${3:?}"; SECS="${4:-70}"
rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none \
  -serial file:"$LOG" -kernel "$K" \
  -drive file="$IMG",if=sd,format=raw &
QPID=$!
for _ in $(seq 1 "$SECS"); do
  kill -0 "$QPID" 2>/dev/null || break
  sleep 1
done
kill -9 "$QPID" 2>/dev/null
wait "$QPID" 2>/dev/null
echo "run finished: $LOG ($(wc -l < "$LOG" | tr -d ' ') lines)"
