#!/usr/bin/env bash
# run-rpi4.sh <kernel8.img> <card.img> <tag> [drive-m3.py options]
# Headless QEMU raspi4b (`-append "${APPEND:-serial=on}"`, SER01), the card as SD, serial
# on a unix socket the driver reads, QMP for screendumps. The card is a
# private copy per run. The driver ends the run (the host has no `timeout`).
set -uo pipefail
K="$1"; SD="$2"; TAG="$3"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$HERE/../screendumps"
# ZVIEW02: socket dir inside the scratch root S (SOCKROOT); QEMU and the driver
# run with that dir as cwd and use RELATIVE socket names (path limit 104 B).
SDIR="$(mktemp -d "${SOCKROOT:?scratch dir for sockets}/s.XXXXXX")"
cd "$SDIR"
CARD="$(dirname "$SD")/run-$TAG.img"
cp -c "$SD" "$CARD"   # ZVIEW01: APFS clone, deleted after the run
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "${APPEND:-serial=on}" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial unix:ser.sock,server=on,wait=on \
  -qmp unix:qmp.sock,server=on,wait=off &
QPID=$!
python3 "$HERE/drive-m3.py" ser.sock qmp.sock "$OUT" "$TAG" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
cd /; rm -f "$SDIR/ser.sock" "$SDIR/qmp.sock"; [ -n "${KEEP:-}" ] || rm -f "$CARD"  # ZVIEW04: KEEP=1 leaves it to the caller
 rmdir "$SDIR" 2>/dev/null
echo "run $TAG: rc=$RC"
