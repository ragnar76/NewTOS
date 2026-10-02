#!/usr/bin/env bash
# fetch-report.sh <sd.img> <outdir> -- copy C:\SCRNINFO.TXT off the card.
set -euo pipefail
IMG="${1:?}"; OUT="${2:?}"
MNT="$(dirname "$IMG")/mnt.$$"
mkdir -p "$MNT" "$OUT"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
ls -l "$MNT"
cp "$MNT/SCRNINFO.TXT" "$OUT/SCRNINFO.TXT"
