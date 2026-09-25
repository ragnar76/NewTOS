#!/usr/bin/env bash
# run-qemu.sh KERNEL8 IMG_SRC OUT_PREFIX SECONDS
# Boots the instrumented kernel on a FRESH copy of the card image, lets it run
# for SECONDS, then kills QEMU.  macOS has no `timeout`, so the bound is a
# background sleep + kill.
set -uo pipefail
K="$1"; SRC="$2"; OUT="$3"; SECS="${4:-90}"
IMG="${OUT}.img"
cp "$SRC" "$IMG"
LOG="${OUT}-serial.log"
rm -f "$LOG"
qemu-system-aarch64 -M raspi4b -no-reboot -display none \
  -serial file:"$LOG" -kernel "$K" \
  -drive file="$IMG",if=sd,format=raw &
QPID=$!
( sleep "$SECS"; kill -9 "$QPID" 2>/dev/null ) &
WPID=$!
wait "$QPID" 2>/dev/null
kill -9 "$WPID" 2>/dev/null
echo "log: $LOG ($(wc -l < "$LOG") lines)"
