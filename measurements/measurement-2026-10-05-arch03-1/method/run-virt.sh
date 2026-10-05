#!/usr/bin/env bash
# run-virt.sh <kernel.img (flat)> <tag> [drive.py options]
# qemu-virt as `xtask run --board qemu-virt` (GICv2, cortex-a72, 272 MiB,
# ramfb, xHCI kbd/mouse), serial on a unix socket the driver reads/writes,
# QMP for screendumps. The driver ends the run (the host has no `timeout`).
set -uo pipefail
K="$1"; TAG="$2"; shift 2
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$HERE/../screendumps"
SDIR="$(mktemp -d /private/tmp/claude-501/a31.XXXXXX)"   # unix socket path < 104 B
qemu-system-aarch64 -M virt,gic-version=2 -cpu cortex-a72 -m 272M \
  -device ramfb -device qemu-xhci -device usb-kbd -device usb-mouse \
  -display none -no-reboot -kernel "$K" \
  -serial unix:"$SDIR/ser.sock",server=on,wait=on \
  -qmp unix:"$SDIR/qmp.sock",server=on,wait=off &
QPID=$!
python3 "$HERE/drive.py" "$SDIR/ser.sock" "$SDIR/qmp.sock" "$OUT" "$TAG" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SDIR/ser.sock" "$SDIR/qmp.sock"; rmdir "$SDIR" 2>/dev/null
echo "run $TAG: rc=$RC"
