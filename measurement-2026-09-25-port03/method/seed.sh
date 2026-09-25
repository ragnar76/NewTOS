#!/usr/bin/env bash
#
# seed.sh <image-path>
#
# Build a fresh card image with tools/make-sd-image.sh and add the PORT03
# fixture, which is ONE copy on top of the standard image:
#
#   C:\QED.NAP + C:\qed.rsc   QED and its resource at the ROOT
#
# QED normally lives in C:\Apps\QED\. It is copied to the root for the same
# reason as in PORT01: the acceptance gesture is a drag of C:\README.TXT onto
# QED's icon, and with both in the SAME window that is one gesture with no
# cross-window drag. The drop path is identical either way -- `fun_win2win`
# (kernel/src/desk/dir.rs:1793) runs on a within-window drop as well.
#
# The dragged file C:\README.TXT and C:\Apps\TERADESK\ come from
# make-sd-image.sh itself. The image is REBUILT per measurement, never copied
# from an older run.
set -euo pipefail
IMG="${1:?usage: seed.sh <image-path>}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../.." && pwd)"

"$WS/tools/make-sd-image.sh" "$IMG" 64 >/dev/null

DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
cleanup() { hdiutil detach "$DEV" >/dev/null 2>&1 || true; }
trap cleanup EXIT
diskutil mount "${DEV}s1" >/dev/null
V=/Volumes/NEWTOS
[ -d "$V" ] || { echo "mount failed"; exit 1; }
# macOS writes AppleDouble `._NAME` companions on a re-mounted FAT volume;
# they would show up as extra desktop items. Off, and swept again before
# unmounting.
mdutil -i off "$V" >/dev/null 2>&1 || true
export COPYFILE_DISABLE=1

cp "$WS/sdk/apps/qed/build/QED.NAP" "$V/QED.NAP"
cp "$WS/sdk/apps/qed/build/qed.rsc" "$V/qed.rsc"

find "$V" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$V" 2>/dev/null || true
find "$V" -name '._*' -delete 2>/dev/null || true
sync
diskutil unmount "$V" >/dev/null
echo "seeded $IMG"
