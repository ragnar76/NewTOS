#!/usr/bin/env bash
# (DESK43 U9-0 copy of measurement-2026-10-14-desk43-u8b/method/run-u6a.sh; only the socket directory changed: /tmp/m5j/u90)
# run-u6a.sh <kernel8.img> <card.img> <tag> [drive-u6a.py options]
# Headless QEMU raspi4b (`-append "${APPEND:-serial=on}"`), the card as SD (a
# private copy per run, deleted afterwards), serial on a unix socket read by
# drive-u6a.py, QMP for screendumps. Copy of
# measurement-2026-10-13-mem05-acceptance/method/run-rpi4-drive.sh with the
# U6a driver. Sockets in a short directory (unix path < 104 bytes).
set -uo pipefail
K="$1"; SD="$2"; TAG="$3"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$HERE/../screendumps"
SDIR="$(mktemp -d /tmp/m5j/u90/s.XXXXXX)"
CARD="$(dirname "$SD")/run-$TAG.img"
cp "$SD" "$CARD"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "${APPEND:-serial=on}" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial unix:"$SDIR/ser.sock",server=on,wait=on \
  -qmp unix:"$SDIR/qmp.sock",server=on,wait=off &
QPID=$!
python3 "$HERE/drive-u6a.py" "$SDIR/ser.sock" "$SDIR/qmp.sock" "$OUT" "$TAG" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SDIR/ser.sock" "$SDIR/qmp.sock"; rmdir "$SDIR" 2>/dev/null; rm -f "$CARD"
echo "run $TAG: rc=$RC"
