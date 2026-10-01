#!/usr/bin/env bash
# prep-app.sh APP_PATH_ON_C OUT_IMG
set -euo pipefail
APP="$1"; OUT_IMG="$2"
BASE="/Volumes/Daten/Development/PiRusTOS/measurement-2026-09-13-regression/qemu-tmp/sd-base.img"

cp -f "$BASE" "$OUT_IMG"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$OUT_IMG" | head -1 | awk '{print $1}')
diskutil mount "${DEV}s1" >/dev/null
V="/Volumes/NEWTOS"
printf '#Z 01 %s@\r\n' "$APP" > "$V/NEWDESK.INF"
sync
diskutil unmount "${DEV}s1" >/dev/null
hdiutil detach "$DEV" >/dev/null
echo "prepared $OUT_IMG autorun=$APP"
