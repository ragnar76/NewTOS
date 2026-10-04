#!/usr/bin/env bash
# seed-doom.sh <image> <DOOM.NAP> <doom1.wad> <NEWDESK.INF>
# Like seed.sh (own volume label, private mount point, no card touched):
# C:\DOOM\DOOM.NAP + C:\DOOM\DOOM1.WAD + NEWDESK.INF with the #Z line.
set -euo pipefail
IMG="${1:?}"; NAP="${2:?}"; WAD="${3:?}"; INF="${4:?}"
rm -f "$IMG"
dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" C0CDOOM 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null
mkdir -p "$MNT"
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
mdutil -i off "$MNT" >/dev/null 2>&1 || true
mkdir -p "$MNT/DOOM"
cp "$NAP" "$MNT/DOOM/DOOM.NAP"
cp "$WAD" "$MNT/DOOM/DOOM1.WAD"
cp "$INF" "$MNT/NEWDESK.INF"
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
sync
echo "seeded $IMG"
