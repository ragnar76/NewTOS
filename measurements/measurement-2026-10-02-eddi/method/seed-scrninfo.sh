#!/usr/bin/env bash
#
# seed-scrninfo.sh <image> <SCRNINFO.NAP>
#
# Fresh QEMU card from tools/make-sd-image.sh (never a reused copy), plus
# C:\SCRNINFO.NAP and a NEWDESK.INF whose #Z autorun line starts it instead of
# the desktop (recipe of measurement-2026-10-02-port06/method/seed-doom.sh).
# The image is mounted on a private mount point next to it, never under
# /Volumes. tools/make-sd-image.sh is not changed.
set -euo pipefail
IMG="${1:?usage: seed-scrninfo.sh <image> <SCRNINFO.NAP>}"
NAP="${2:?}"
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
cp "$NAP" "$MNT/SCRNINFO.NAP"
cp "$HERE/newdesk-scrninfo.inf" "$MNT/NEWDESK.INF"
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
find "$MNT" -name '._*' -delete 2>/dev/null || true
sync
echo "seeded $IMG: C:\\SCRNINFO.NAP ($(wc -c < "$NAP" | tr -d ' ') B), #Z autorun"
