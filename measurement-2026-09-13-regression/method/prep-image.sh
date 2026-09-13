#!/usr/bin/env bash
# prep-image.sh AREA TSV_PATH OUT_IMG
# AREA: aes | vdi | raster
set -euo pipefail
AREA="$1"; TSV="$2"; OUT_IMG="$3"
BASE="/Volumes/Daten/Development/PiRusTOS/measurement-2026-09-13-regression/qemu-tmp/sd-base.img"
BUILD="/Volumes/Daten/Development/PiRusTOS/tosdump/build"

cp -f "$BASE" "$OUT_IMG"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$OUT_IMG" | head -1 | awk '{print $1}')
diskutil mount "${DEV}s1" >/dev/null
V="/Volumes/NEWTOS"

case "$AREA" in
  aes)
    cp -f "$BUILD/DUMPAES.NAP" "$V/DUMPAES.NAP"
    cp -f "$TSV" "$V/AES.TSV"
    printf '#Z 01 C:\\DUMPAES.NAP@\r\n' > "$V/NEWDESK.INF"
    ;;
  vdi)
    cp -f "$BUILD/DUMPVDI.NAP" "$V/DUMPVDI.NAP"
    cp -f "$TSV" "$V/VDI.TSV"
    printf '#Z 01 C:\\DUMPVDI.NAP@\r\n' > "$V/NEWDESK.INF"
    ;;
  raster)
    cp -f "$BUILD/DUMPRAST.NAP" "$V/DUMPRAST.NAP"
    cp -f "$TSV" "$V/RASTER.TSV"
    printf '#Z 01 C:\\DUMPRAST.NAP@\r\n' > "$V/NEWDESK.INF"
    ;;
  *)
    echo "unknown area $AREA" >&2; exit 1;;
esac

sync
diskutil unmount "${DEV}s1" >/dev/null
hdiutil detach "$DEV" >/dev/null
echo "prepared $OUT_IMG for area=$AREA tsv=$TSV"
