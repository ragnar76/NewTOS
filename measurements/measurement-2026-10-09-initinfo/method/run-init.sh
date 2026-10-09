#!/usr/bin/env bash
# run-init.sh <kernel8.img> <base card or -> <WxH> <outdir> -- boot QEMU raspi4b with the
# bcm2835-fb sized WxH, serial to a file, QMP screendumps at fixed offsets while the
# boot screen is held (INITINFO_DURATION = 15 s on board-rpi4).
set -uo pipefail
K="$1"; BASE="$2"; RES="$3"; OUT="$4"; X="${RES%x*}"; Y="${RES#*x}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$OUT"; OUT="$(cd "$OUT" && pwd)"; K="$(cd "$(dirname "$K")" && pwd)/$(basename "$K")"
SDIR="$(mktemp -d "${TMPDIR:-/tmp}/initinfo.XXXX")"
cd "$SDIR" || exit 1   # QMP socket path is relative: macOS limits it to 104 bytes
DRV=()
if [ "$BASE" != "-" ]; then CARD="$SDIR/card.img"; cp -c "$BASE" "$CARD"; DRV=(-drive file="$CARD",if=sd,format=raw); fi
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "serial=on" \
  -global bcm2835-fb.xres=$X -global bcm2835-fb.yres=$Y \
  -kernel "$K" ${DRV[@]+"${DRV[@]}"} \
  -serial file:"$OUT/serial-$RES.log" -qmp unix:qmp.sock,server=on,wait=off &
QPID=$!
python3 "$HERE/qmp_timed.py" qmp.sock "$OUT" 6:initinfo-$RES-t06.png 10:initinfo-$RES-t10.png 14:initinfo-$RES-t14.png
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
cd / && rm -rf "$SDIR"
