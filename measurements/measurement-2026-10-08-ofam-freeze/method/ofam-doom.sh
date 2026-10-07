#!/usr/bin/env bash
# OFAM freeze: DOOM start, run, Esc/F10/Y quit. Card = base.img clone + C:\DOOM\{DOOM.NAP,DOOM1.WAD}.
set -uo pipefail
S="${1:?scratch}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; WS="$(cd "$HERE/../../.." && pwd)"
OFF=$((63 * 512)); C="$S/cards"; export SOCKROOT="$S"
if [ ! -f "$C/doom.img" ]; then cp -c "$S/base.img" "$C/doom.img"
  mmd -i "$C/doom.img@@$OFF" ::DOOM
  mcopy -o -i "$C/doom.img@@$OFF" "$WS/sdk/apps/doom/build/DOOM.NAP" ::DOOM/DOOM.NAP
  mcopy -o -i "$C/doom.img@@$OFF" "$WS/Applications/doom-wad/doom1.wad" ::DOOM/DOOM1.WAD
fi
CARD="$C/doom-run.img"; rm -f "$CARD"; cp -c "$C/doom.img" "$CARD"
sed 's|^#Z .*|#Z 01 C:\\DOOM\\DOOM.NAP@|' "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
mcopy -o -i "$CARD@@$OFF" "$S/NEWDESK.INF" ::NEWDESK.INF
APPEND=serial=on bash "$HERE/run-rpi4-drive-m3.sh" "$S/kinj-doom.img" "$CARD" doom --mark-delay 0.5 --end '\[m3\] MARK END' --end-delay 1 --timeout 400
rm -f "$CARD"
