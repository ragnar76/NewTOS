#!/usr/bin/env bash
# extract-log.sh IMG OUT_FILE   -- pull C:\APITEST.LOG off a card image.
set -euo pipefail
IMG="$1"; OUT="$2"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
diskutil mount "${DEV}s1" >/dev/null
cp -f "/Volumes/NEWTOS/APITEST.LOG" "$OUT" 2>/dev/null || echo "no APITEST.LOG on $IMG" >&2
diskutil unmount "${DEV}s1" >/dev/null
hdiutil detach "$DEV" >/dev/null
