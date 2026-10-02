#!/usr/bin/env bash
#
# seed-doom.sh <image> <DOOM.NAP> [<doom1.wad>]
#
# measurement-2026-10-02-port06/method/seed-doom.sh with ONE change: the WAD
# is optional. Without it the card has C:\DOOM\DOOM.NAP and the #Z autorun
# NEWDESK.INF (PORT06's, unchanged) but no IWAD -- DOOM's I_Error path
# ("no IWAD file was found"), which since PORT10 runs system() for real.
# Fresh QEMU card from tools/make-sd-image.sh (never a reused copy); the image
# is mounted on a private mount point next to it, never under /Volumes.
set -euo pipefail
IMG="${1:?usage: seed-doom.sh <image> <DOOM.NAP> [<doom1.wad>]}"
NAP="${2:?}"; WAD="${3:-}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
P06="$WS/measurements/measurement-2026-10-02-port06/method"
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
[ -n "$WAD" ] && cp "$WAD" "$MNT/DOOM/DOOM1.WAD"
cp "$P06/newdesk-doom.inf" "$MNT/NEWDESK.INF"
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
find "$MNT" -name '._*' -delete 2>/dev/null || true
sync
echo "seeded $IMG: C:\\DOOM\\DOOM.NAP ($(wc -c < "$NAP" | tr -d ' ') B), WAD: ${WAD:-NONE}, #Z autorun"
