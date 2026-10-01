#!/usr/bin/env bash
# seed.sh <image-path>
#
# Build a fresh card image with tools/make-sd-image.sh (it changed with DESK19,
# which installs C:\CMDLINE.NCP, so it is re-run per measurement and never
# copied from an older run) and add the fixture these two tickets need:
#
#   C:\DESKT19\PROOF.TXT     "PROOF: C:\DESKT19"       (the Default-dir probe)
#   C:\DESKT19\DROPME.TXT    "dropped file contents"   (the dragged file)
#
# The acceptance gesture is a drag of C:\README.TXT (already on the standard
# image) onto C:\CMDLINE.NCP inside the C:\ window: one window, one gesture,
# and CMDLINE.NCP prints the command line it was handed.
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

mkdir -p "$V/DESKT19"
printf 'PROOF: C:\\DESKT19\r\n'     > "$V/DESKT19/PROOF.TXT"
printf 'dropped file contents\r\n'  > "$V/DESKT19/DROPME.TXT"

find "$V" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$V" 2>/dev/null || true
find "$V" -name '._*' -delete 2>/dev/null || true
sync
diskutil unmount "$V" >/dev/null
echo "seeded $IMG"
