#!/usr/bin/env bash
# run-ev.sh <kernel8.img> <card.img> <tag> [drive-ev.py options] -- DESK43 U9-1/U9-2 (pattern of run-u6a.sh):
# headless QEMU raspi4b, -append "${APPEND:-serial=on}", the card as SD (private copy, deleted afterwards; KEEPCARD=1
# keeps it as <card-dir>/run-<tag>.img), serial on a unix socket read by drive-ev.py, QMP screendumps.
set -uo pipefail
K="$1"; SD="$2"; TAG="$3"; shift 3
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$HERE/../screendumps"
mkdir -p /tmp/m5j/u912/s
SDIR="$(mktemp -d /tmp/m5j/u912/s/r.XXXXXX)"
CARD="$(dirname "$SD")/run-$TAG.img"
cp "$SD" "$CARD"
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "${APPEND:-serial=on}" \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial unix:"$SDIR/ser.sock",server=on,wait=on \
  -qmp unix:"$SDIR/qmp.sock",server=on,wait=off &
QPID=$!
python3 "$HERE/drive-ev.py" "$SDIR/ser.sock" "$SDIR/qmp.sock" "$OUT" "$TAG" "$@"
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
rm -f "$SDIR/ser.sock" "$SDIR/qmp.sock"; rmdir "$SDIR" 2>/dev/null
[ "${KEEPCARD:-0}" = 1 ] || rm -f "$CARD"
echo "run $TAG: rc=$RC"
