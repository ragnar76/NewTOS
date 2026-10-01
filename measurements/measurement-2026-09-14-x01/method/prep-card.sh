#!/usr/bin/env bash
# prep-card.sh OUT_IMG APITEST_NCP DESKTOP_NAP QED_NAP AUTORUN
#
# Build one card for an X01 machine run. AUTORUN is either the empty string
# (built-in desktop after C:\AUTO) or a program path for the NEWDESK.INF `#Z`
# autorun line, e.g. 'C:\Apps\TERADESK\DESKTOP.NAP'.
#
# APITEST always goes into \AUTO\ together with C:\APITEST.CFG containing the
# token BATCH, so it never blocks on Cconin() in a headless run.
set -euo pipefail
OUT_IMG="$1"; APITEST="$2"; TERA="$3"; QED="$4"; AUTORUN="${5:-}"
BASE="/Volumes/Daten/Development/PiRusTOS/measurement-2026-09-13-regression/qemu-tmp/sd-base.img"

cp -f "$BASE" "$OUT_IMG"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$OUT_IMG" | head -1 | awk '{print $1}')
diskutil mount "${DEV}s1" >/dev/null
V="/Volumes/NEWTOS"

cp -f "$APITEST" "$V/AUTO/APITEST.NCP"
printf 'BATCH\r\n' > "$V/APITEST.CFG"
cp -f "$TERA" "$V/Apps/TERADESK/DESKTOP.NAP"
cp -f "$QED"  "$V/Apps/QED/QED.NAP"
rm -f "$V/APITEST.LOG"

if [ -n "$AUTORUN" ]; then
    printf '#Z 01 %s@\r\n' "$AUTORUN" > "$V/NEWDESK.INF"
else
    printf 'newdesk placeholder\r\n' > "$V/NEWDESK.INF"
fi

sync
diskutil unmount "${DEV}s1" >/dev/null
hdiutil detach "$DEV" >/dev/null
echo "prepared $OUT_IMG autorun='${AUTORUN:-<built-in desktop>}'"
