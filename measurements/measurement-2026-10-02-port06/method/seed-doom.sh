#!/usr/bin/env bash
#
# seed-doom.sh <image> <DOOM.NAP> <doom1.wad>
#
# Fresh QEMU card from tools/make-sd-image.sh (never a reused copy), plus
# C:\DOOM\DOOM.NAP, C:\DOOM\DOOM1.WAD and a NEWDESK.INF whose #Z autorun line
# starts C:\DOOM\DOOM.NAP instead of the desktop (recipe of
# measurement-2026-09-28-port07/method/seed-ball.sh). The WAD is staged here,
# on the scratch image only -- tools/make-sd-image.sh is not changed.
# The image is mounted on a private mount point, never under /Volumes.
set -euo pipefail
IMG="${1:?usage: seed-doom.sh <image> <DOOM.NAP> <doom1.wad>}"
NAP="${2:?}"; WAD="${3:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
"$WS/tools/make-sd-image.sh" "$IMG" 64 >/dev/null
MNT="$(dirname "$IMG")/mnt.$$"
mkdir -p "$MNT"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
mkdir -p "$MNT/DOOM"
cp "$NAP" "$MNT/DOOM/DOOM.NAP"
cp "$WAD" "$MNT/DOOM/DOOM1.WAD"
cp "$HERE/newdesk-doom.inf" "$MNT/NEWDESK.INF"
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
find "$MNT" -name '._*' -delete 2>/dev/null || true
sync
echo "seeded $IMG: C:\\DOOM\\DOOM.NAP ($(wc -c < "$NAP" | tr -d ' ') B), DOOM1.WAD ($(wc -c < "$WAD" | tr -d ' ') B), #Z autorun"
