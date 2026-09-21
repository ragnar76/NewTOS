#!/usr/bin/env bash
# prep-card.sh BASE_IMG OUT_IMG APITEST_NCP|- DESKTOP_NAP|- QED_NAP|- AUTORUN
#
# One card per T05/T06 run, from the make-sd-image.sh base. '-' keeps the
# base's file. APITEST (when given) goes into \AUTO\ with C:\APITEST.CFG =
# BATCH so a headless run never blocks on Cconin(). AUTORUN is '' (built-in
# desktop after \AUTO\) or a program path for the NEWDESK.INF `#Z` line.
set -euo pipefail
BASE="$1"; OUT_IMG="$2"; APITEST="$3"; TERA="$4"; QED="$5"; AUTORUN="${6:-}"
cp -f "$BASE" "$OUT_IMG"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$OUT_IMG" | head -1 | awk '{print $1}')
diskutil mount "${DEV}s1" >/dev/null
V="/Volumes/NEWTOS"
if [ "$APITEST" != "-" ]; then
    cp -f "$APITEST" "$V/AUTO/APITEST.NCP"
    printf 'BATCH\r\n' > "$V/APITEST.CFG"
    rm -f "$V/APITEST.LOG"
fi
[ "$TERA" != "-" ] && cp -f "$TERA" "$V/Apps/TERADESK/DESKTOP.NAP"
[ "$QED"  != "-" ] && cp -f "$QED"  "$V/Apps/QED/QED.NAP"
if [ -n "$AUTORUN" ]; then
    printf '#Z 01 %s@\r\n' "$AUTORUN" > "$V/NEWDESK.INF"
else
    printf 'newdesk placeholder\r\n' > "$V/NEWDESK.INF"
fi
sync
diskutil unmount "${DEV}s1" >/dev/null
hdiutil detach "$DEV" >/dev/null
echo "prepared $OUT_IMG apitest=$APITEST tera=$TERA qed=$QED autorun='${AUTORUN:-<built-in desktop>}'"
