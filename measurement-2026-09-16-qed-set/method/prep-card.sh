#!/usr/bin/env bash
# prep-card.sh OUT_IMG QED_NAP
#
# Derived from measurement-2026-09-14-x01/method/prep-card.sh, reduced to what
# this measurement changes: a copy of the regression base card with one
# QED.NAP swapped in and a NEWDESK.INF that `#Z`-autoruns it. Nothing else on
# the card is touched, so the two runs differ ONLY in QED.NAP.
set -euo pipefail
OUT_IMG="$1"; QED="$2"
BASE="/Volumes/Daten/Development/PiRusTOS/measurement-2026-09-13-regression/qemu-tmp/sd-base.img"

cp -f "$BASE" "$OUT_IMG"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$OUT_IMG" | head -1 | awk '{print $1}')
diskutil mount "${DEV}s1" >/dev/null
V="/Volumes/NEWTOS"

cp -f "$QED" "$V/Apps/QED/QED.NAP"
printf '#Z 01 %s@\r\n' 'C:\Apps\QED\QED.NAP' > "$V/NEWDESK.INF"

sync
diskutil unmount "${DEV}s1" >/dev/null
hdiutil detach "$DEV" >/dev/null
echo "prepared $OUT_IMG with $(basename "$QED")"
