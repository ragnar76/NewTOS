#!/usr/bin/env bash
# OFAM freeze: TeraDesk starts QED via F1 (TERADESK-QED.INF). One clone of base.img per run.
set -uo pipefail
S="${1:?scratch}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OFF=$((63 * 512)); C="$S/cards"; export SOCKROOT="$S"
CARD="$C/tdqed.img"; rm -f "$CARD"; cp -c "$S/base.img" "$CARD"
sed 's|^#Z .*|#Z 01 C:\\Apps\\TERADESK\\DESKTOP.NAP@|' "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
mcopy -o -i "$CARD@@$OFF" "$S/NEWDESK.INF" ::NEWDESK.INF
mcopy -o -i "$CARD@@$OFF" "$HERE/TERADESK-QED.INF" ::TERADESK.INF
mcopy -o -i "$CARD@@$OFF" "$HERE/TERADESK-QED.INF" ::Apps/TERADESK/TERADESK.INF
APPEND=serial=on bash "$HERE/run-rpi4-drive-m3.sh" "$S/kinj-tdqed.img" "$CARD" tdqed --mark-delay 0.5 --end '\[m3\] MARK END' --end-delay 1 --timeout 150
rm -f "$CARD"
