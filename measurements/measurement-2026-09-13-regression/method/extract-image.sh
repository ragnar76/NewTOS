#!/usr/bin/env bash
# extract-image.sh AREA IMG OUT_DIR
set -euo pipefail
AREA="$1"; IMG="$2"; OUT_DIR="$3"
mkdir -p "$OUT_DIR"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
diskutil mount "${DEV}s1" >/dev/null
V="/Volumes/NEWTOS"
case "$AREA" in
  aes) BASE=AES;;
  vdi) BASE=VDI;;
  raster) BASE=RASTER;;
  *) echo "unknown area $AREA" >&2; exit 1;;
esac
cp -f "$V/$BASE.BIN" "$OUT_DIR/$BASE.BIN" 2>&1 || echo "no $BASE.BIN"
cp -f "$V/$BASE.LOG" "$OUT_DIR/$BASE.LOG" 2>&1 || echo "no $BASE.LOG"
diskutil unmount "${DEV}s1" >/dev/null
hdiutil detach "$DEV" >/dev/null
echo "extracted $BASE.BIN/.LOG to $OUT_DIR"
