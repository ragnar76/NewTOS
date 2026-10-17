#!/usr/bin/env bash
# extract-log.sh IMG OUT FILE  -- copy C:\<FILE> off a card image (private mount point).
set -euo pipefail
IMG="$1"; OUT="$2"; F="${3:-APITEST.LOG}"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" 2>/dev/null | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/xmnt.$$"; mkdir -p "$MNT"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
cp "$MNT/$F" "$OUT"
