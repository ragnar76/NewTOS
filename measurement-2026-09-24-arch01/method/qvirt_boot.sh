#!/usr/bin/env bash
# qvirt_boot.sh <ws> <log> [timeout_s]
# Boot the SHARED tree's qemu-virt kernel headless and stop at
# "[desk] desktop up" (or on a fault / timeout). Same QEMU arguments as
# `xtask run --board qemu-virt`, with the serial redirected to a file.
set -uo pipefail
WS="$1"; LOG="$2"; TMO="${3:-180}"
( cd "$WS" && cargo run -p xtask -- build --board qemu-virt ) || exit 1
ELF="$WS/target/aarch64-unknown-none-softfloat/debug/kernel"
IMG="$(dirname "$LOG")/qemu-virt-kernel.img"
rust-objcopy -O binary "$ELF" "$IMG" || exit 1
rm -f "$LOG"
qemu-system-aarch64 -M virt,gic-version=2 -cpu cortex-a72 -m 272M \
  -device ramfb -device qemu-xhci -device usb-kbd -device usb-mouse \
  -display none -serial file:"$LOG" -no-reboot -kernel "$IMG" &
QPID=$!
t0=$(date +%s)
while :; do
  sleep 0.3
  if [ -f "$LOG" ]; then
    /usr/bin/grep -qF "[desk] desktop up" "$LOG" 2>/dev/null && { echo "qvirt: desktop up"; break; }
    /usr/bin/grep -qE '\[EXC\]|\[PANIC\]' "$LOG" 2>/dev/null && { sleep 2; echo "qvirt: fault"; break; }
  fi
  kill -0 "$QPID" 2>/dev/null || { echo "qvirt: qemu exited"; break; }
  [ $(( $(date +%s) - t0 )) -ge "$TMO" ] && { echo "qvirt: timeout"; break; }
done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$IMG"
exit 0
