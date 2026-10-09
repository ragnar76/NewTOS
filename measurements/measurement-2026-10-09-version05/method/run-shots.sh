#!/usr/bin/env bash
# run-shots.sh <kernel8.img> <WxH> <outdir> <tag> <SECONDS:PNG>... -- QEMU raspi4b,
# no SD card, bcm2835-fb sized WxH, -append serial=on, serial to
# <outdir>/<tag>-serial.log, QMP screendumps at wall-clock offsets, then quit.
set -uo pipefail
K="$1"; RES="$2"; OUT="$3"; TAG="$4"; shift 4; X="${RES%x*}"; Y="${RES#*x}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$OUT"; OUT="$(cd "$OUT" && pwd)"; K="$(cd "$(dirname "$K")" && pwd)/$(basename "$K")"
SDIR="$(mktemp -d "${SOCKROOT:?scratch dir for sockets}/s.XXXXXX")"
cd "$SDIR" || exit 1   # relative QMP socket: macOS limits the path to 104 bytes
qemu-system-aarch64 -M raspi4b -no-reboot -display none -append "serial=on" \
  -global bcm2835-fb.xres=$X -global bcm2835-fb.yres=$Y \
  -kernel "$K" -serial file:"$OUT/$TAG-serial.log" -qmp unix:qmp.sock,server=on,wait=off &
QPID=$!
specs=(); for s in "$@"; do specs+=("${s%%:*}:$TAG-${s#*:}"); done
python3 "$HERE/qmp_timed.py" qmp.sock "$OUT" "${specs[@]}"
for _ in $(seq 1 20); do kill -0 "$QPID" 2>/dev/null || break; sleep 0.5; done
kill -9 "$QPID" 2>/dev/null; wait "$QPID" 2>/dev/null
cd / && rm -f "$SDIR/qmp.sock"; rmdir "$SDIR"
