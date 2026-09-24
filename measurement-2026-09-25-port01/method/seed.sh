#!/usr/bin/env bash
#
# seed.sh <image-path> <before-dir>
#
# Build a fresh card image with tools/make-sd-image.sh (it changed with PORT01
# -- it now installs C:\ARGVTEST.NAP -- so it is re-run per measurement and
# never copied from an older run) and add the PORT01 fixture:
#
#   C:\ARGVOLD.NAP   ARGVTEST linked with the COMMITTED crt0 ("before")
#   C:\QED.NAP       QED, current build ("after"), copied to the ROOT
#   C:\QEDOLD.NAP    the same QED objects linked with the committed crt0
#   C:\qed.rsc       QED's resource, beside both (rsrc_load looks in the
#                    launched program's own directory)
#
# QED normally lives in C:\Apps\QED\. It is copied to the root for this
# measurement for one reason: the acceptance gesture is a drag of
# C:\README.TXT onto the program's icon, and with both in the SAME window that
# is one gesture with no cross-window drag. The drop path is identical either
# way -- `fun_win2win` (kernel/src/desk/dir.rs:1793) runs on a within-window
# drop as well as a cross-window one.
#
# The dragged file is C:\README.TXT, which make-sd-image.sh already installs.
set -euo pipefail
IMG="${1:?usage: seed.sh <image-path> <before-dir>}"
BEFORE="${2:?usage: seed.sh <image-path> <before-dir>}"
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

cp "$BEFORE/ARGVOLD.NAP" "$V/ARGVOLD.NAP"
cp "$BEFORE/QEDOLD.NAP"  "$V/QEDOLD.NAP"
cp "$WS/sdk/apps/qed/build/QED.NAP" "$V/QED.NAP"
cp "$WS/sdk/apps/qed/build/qed.rsc" "$V/qed.rsc"

find "$V" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$V" 2>/dev/null || true
find "$V" -name '._*' -delete 2>/dev/null || true
sync
diskutil unmount "$V" >/dev/null
echo "seeded $IMG"
