#!/usr/bin/env bash
# put-inf.sh IMG INF   — replace C:\NEWDESK.INF in the FAT16 image, delete QEDTRACE.LOG
set -euo pipefail
IMG="$1"; INF="$2"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage "$IMG" 2>/dev/null | awk '/FAT_16|Windows_FAT_16|DOS_FAT_16/{print $1; exit}')
MP=$(mount | awk -v d="$DEV" '$1==d{print $3}')
cp "$INF" "$MP/NEWDESK.INF"
rm -f "$MP/QEDTRACE.LOG"
sync
diskutil unmount "$DEV" >/dev/null
hdiutil detach "${DEV%s1}" >/dev/null 2>&1 || hdiutil detach "$DEV" >/dev/null
echo "put $INF -> $IMG"
