#!/usr/bin/env bash
# boot-qmp.sh KERNEL8 IMG SERIAL_LOG QMP_SOCK
# The emulator line of measurement-2026-09-14-a28/method/boot-qmp.sh, verbatim.
set -uo pipefail
KERNEL8="$1"; IMG="$2"; LOG="$3"; SOCK="$4"
rm -f "$LOG" "$SOCK"
exec qemu-system-aarch64 -M raspi4b -no-reboot -display none \
  -serial file:"$LOG" \
  -kernel "$KERNEL8" \
  -drive file="$IMG",if=sd,format=raw \
  -qmp unix:"$SOCK",server,nowait
