#!/usr/bin/env bash
# PRESENT01 copy (sysinfo2 method) + screen size from XR/YR (default 640x480,
# -global bcm2835-fb.xres/yres as measurement-2026-10-07-zview-perf).
# run-rpi4-drive-m3.sh <kernel8.img> <card.img> <tag> [drive-m3.py options]
# Headless QEMU raspi4b (`-append "${APPEND:-serial=on}"`, SER01), the card as
# SD, serial on a unix socket the driver reads, QMP for screendumps. The card
# is used IN PLACE (the caller made it a private per-run copy and deletes it
# after reading back what it needs). SYSINFO02 change against the SYSINFO01
# copy: the sockets live in <card dir>/q.<tag> (the session scratch) under
# relative names (unix socket paths must stay < 104 bytes), and the card is
# not copied a second time. The driver ends the run (the host has no
# `timeout`).
set -uo pipefail
K="$1"; CARD="$2"; TAG="$3"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$HERE/../screendumps"
SDIR="$(dirname "$CARD")/q.$TAG"; rm -rf "$SDIR"; mkdir -p "$SDIR"
cd "$SDIR" || exit 1
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "${APPEND:-serial=on}" \
  -global bcm2835-fb.xres="${XR:-640}" -global bcm2835-fb.yres="${YR:-480}" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial unix:ser.sock,server=on,wait=on \
  -qmp unix:qmp.sock,server=on,wait=off &
QPID=$!
python3 "$HERE/drive-m3.py" ser.sock qmp.sock "$OUT" "$TAG" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
cd / && rm -rf "$SDIR"
echo "run $TAG: rc=$RC"
