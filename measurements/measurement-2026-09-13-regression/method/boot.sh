#!/usr/bin/env bash
# boot.sh IMG SERIAL_LOG
set -uo pipefail
IMG="$1"; LOG="$2"
KERNEL8=/Volumes/Daten/Development/PiRusTOS/kernel8.img
rm -f "$LOG"
exec qemu-system-aarch64 -M raspi4b -no-reboot -display none \
  -serial file:"$LOG" \
  -kernel "$KERNEL8" \
  -drive file="$IMG",if=sd,format=raw
