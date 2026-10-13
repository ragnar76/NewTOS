#!/usr/bin/env bash
# run-rpi4.sh <kernel8.img> <card.img> <tag> [drive.py options]
# Headless QEMU raspi4b (`-append "${APPEND:-serial=on}"`, SER01), the card as SD, serial
# on a unix socket the driver reads, QMP for screendumps. The card is a
# private copy per run. The driver ends the run (the host has no `timeout`).
set -uo pipefail
K="$1"; SD="$2"; TAG="$3"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$HERE/../screendumps"
SDIR="$(mktemp -d /private/tmp/claude-501/n12/a31.XXXXXX)"   # unix socket path < 104 B
CARD="$(dirname "$SD")/run-$TAG.img"
cp "$SD" "$CARD"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "${APPEND:-serial=on}" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial unix:"$SDIR/ser.sock",server=on,wait=on \
  -qmp unix:"$SDIR/qmp.sock",server=on,wait=off &
QPID=$!
python3 "$HERE/drive.py" "$SDIR/ser.sock" "$SDIR/qmp.sock" "$OUT" "$TAG" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SDIR/ser.sock" "$SDIR/qmp.sock"; rmdir "$SDIR" 2>/dev/null
echo "run $TAG: rc=$RC"
