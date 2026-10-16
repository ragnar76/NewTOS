#!/usr/bin/env bash
# virt-smoke.sh <flat-kernel.img> <scratch> -- qemu-virt boot smoke (machine args as measurement-2026-10-16-desk43-u10-test/
# method/run-virt.sh, serial to a file instead of the driver socket): waits for "[desk] desktop up" (max 120 s), then quits.
# No card on this board, so no Pexec probe here; this only proves the qemu-virt kernel boots to the desktop.
set -uo pipefail
K="$1"; S="$2"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; LOG="$HERE/../serial/virt-smoke-serial.log"
RD="$(mktemp -d "$S/virt.XXXX")"; rm -f "$LOG"
qemu-system-aarch64 -M virt,gic-version=2 -cpu cortex-a72 -m 272M -device ramfb -device qemu-xhci -device usb-kbd -device usb-mouse \
  -display none -no-reboot -kernel "$K" -serial file:"$LOG" &
QPID=$!
for _ in $(seq 1 120); do grep -q "\[desk\] desktop up" "$LOG" 2>/dev/null && break; kill -0 $QPID 2>/dev/null || break; sleep 1; done
sleep 2; kill -9 $QPID 2>/dev/null; wait $QPID 2>/dev/null; rm -rf "$RD"
echo "virt smoke: $(grep -c . "$LOG") serial lines; desktop up: $(grep -c '\[desk\] desktop up' "$LOG"); EXC/PANIC: $(grep -Ec '\[EXC\]|PANIC|panicked' "$LOG")"
