#!/usr/bin/env bash
# run-virt.sh <scratch> <kernel.img (flat)> <case> [append]
# qemu-virt as `xtask run --board qemu-virt` (GICv2, cortex-a72, 272 MiB,
# ramfb, xHCI kbd/mouse), serial to <scratch>/serial/<case>.log, one
# screendump 3 s after "[desk] desktop up" (qmp_on_marker.py). Adapted from
# measurement-2026-10-06-vforkt-fix/method/run-virt.sh. Only our own pid is
# killed; QMP socket relative to a private dir.
set -uo pipefail
S="$1"; K="$2"; CASE="$3"; APP="${4:-memprot=on}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$S/serial" "$S/shots"
LOG="$S/serial/$CASE.log"; rm -f "$LOG"
SD="$(mktemp -d "$S/q.XXXXXX")"
cd "$SD"
qemu-system-aarch64 -M virt,gic-version=2 -cpu cortex-a72 -m 272M \
  -device ramfb -device qemu-xhci -device usb-kbd -device usb-mouse \
  -display none -no-reboot -kernel "$K" -append "$APP" \
  -serial file:"$LOG" -qmp unix:qmp.sock,server=on,wait=off &
QPID=$!
MARK_DELAY=3 python3 "$HERE/qmp_on_marker.py" qmp.sock "$LOG" "$S/shots" "[desk] desktop up" 1 "${TIMEOUT:-120}" "$CASE"
cd "$S"
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SD/qmp.sock"; rmdir "$SD" 2>/dev/null
echo "run $CASE done"
