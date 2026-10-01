#!/usr/bin/env bash
# boot-qmp.sh IMG SERIAL_LOG QMP_SOCK
set -uo pipefail
IMG="$1"; LOG="$2"; SOCK="$3"
KERNEL8=/Volumes/Daten/Development/PiRusTOS/kernel8.img
rm -f "$LOG" "$SOCK"
exec qemu-system-aarch64 -M raspi4b -no-reboot -display none \
  -serial file:"$LOG" \
  -kernel "$KERNEL8" \
  -drive file="$IMG",if=sd,format=raw \
  -qmp unix:"$SOCK",server,nowait
