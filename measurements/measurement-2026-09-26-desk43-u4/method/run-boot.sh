#!/usr/bin/env bash
#
# run-boot.sh <kernel8.img> <card.img> <serial.log> <seconds> [png]
#
# Headless QEMU raspi4b boot, serial to a file, bounded by a shell-side kill
# (macOS has no `timeout`). With a 5th argument a QMP screendump is taken 3 s
# after the `desktop up` marker appears in the serial log.
# The QMP socket lives in /tmp because the AF_UNIX path limit is 104 bytes.
set -uo pipefail
K="${1:?}"; IMG="${2:?}"; LOG="${3:?}"; SECS="${4:-70}"; PNG="${5:-}"
rm -f "$LOG"
SOCK=/tmp/u4-$$.sock
qemu-system-aarch64 -M raspi4b -no-reboot -display none \
  -serial file:"$LOG" -qmp unix:"$SOCK",server,nowait \
  -kernel "$K" -drive file="$IMG",if=sd,format=raw &
QPID=$!
if [ -n "$PNG" ]; then
  python3 ../measurement-2026-09-26-gui04/method/qmp_multi.py \
    "$SOCK" "$LOG" "$(dirname "$PNG")" "$SECS" "desktop up:$(basename "$PNG"):3"
else
  for _ in $(seq 1 "$SECS"); do kill -0 "$QPID" 2>/dev/null || break; sleep 1; done
fi
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null; rm -f "$SOCK"
echo "run finished: $LOG ($(wc -l < "$LOG" | tr -d ' ') lines)"
