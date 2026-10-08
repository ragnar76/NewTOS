#!/usr/bin/env bash
# run-res.sh <kernel.img> <base card> <WxH> -- clone card (cp -c), boot QEMU raspi4b with the
# bcm2835-fb sized WxH, serial on a unix socket, drive-m3.py takes a screendump per "[m3] MARK".
set -uo pipefail
K="$1"; BASE="$2"; RES="$3"; X="${RES%x*}"; Y="${RES#*x}"; TAG="r$RES"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$HERE/../screendumps/$RES"; mkdir -p "$OUT"
CARD="$(dirname "$BASE")/run-$TAG.img"; cp -c "$BASE" "$CARD"
SDIR="$(dirname "$BASE")/q.$TAG"; rm -rf "$SDIR"; mkdir -p "$SDIR"; cd "$SDIR" || exit 1
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "serial=on" \
  -global bcm2835-fb.xres=$X -global bcm2835-fb.yres=$Y \
  -kernel "$K" -drive file="$CARD",if=sd,format=raw \
  -serial unix:ser.sock,server=on,wait=on -qmp unix:qmp.sock,server=on,wait=off &
QPID=$!
python3 "$HERE/drive-m3.py" ser.sock qmp.sock "$OUT" "$TAG" --mark-delay 2.5 --end '\[m3\] MARK END' --end-delay 1 --timeout 600
RC=$?
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
cd / && rm -rf "$SDIR" "$CARD"
mv "$OUT/../serial/$TAG-serial.log" "$HERE/../serial/" 2>/dev/null
echo "run $TAG: rc=$RC"
