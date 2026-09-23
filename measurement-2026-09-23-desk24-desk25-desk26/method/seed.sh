#!/usr/bin/env bash
# seed.sh <image-path>
#
# Build a fresh card image with tools/make-sd-image.sh (it changed with DESK18,
# so it is re-run per measurement, never copied from an older run) and add the
# fixture the three tickets need:
#
#   C:\DESKT24\FOLD\INNER.TXT   "CHARLIE inside the source folder"   (DESK24 src)
#   C:\DESKT24\TWO.TXT          "TWO"                                (DESK26)
#   C:\DESKT24\ONE.TXT          "ONE"                                (DESK26 sibling)
#   C:\FOLD\EXIST.TXT           "DELTA already on the root"          (DESK24 dst)
#
# So a drag of C:\DESKT24\FOLD onto the C: desktop icon is a folder copy into
# C:\ whose destination folder name is already taken -- EmuTOS's
# output_path()/get_new_name() case (deskdir.c:734-779).
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
# they would show up as extra desktop items and be copied along. Off, and
# swept again before unmounting.
mdutil -i off "$V" >/dev/null 2>&1 || true
export COPYFILE_DISABLE=1

mkdir -p "$V/DESKT24/FOLD"
printf 'CHARLIE inside the source folder\r\n' > "$V/DESKT24/FOLD/INNER.TXT"
printf 'ONE\r\n'                              > "$V/DESKT24/ONE.TXT"
printf 'TWO\r\n'                              > "$V/DESKT24/TWO.TXT"
mkdir -p "$V/FOLD"
printf 'DELTA already on the root\r\n'        > "$V/FOLD/EXIST.TXT"

find "$V" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$V" 2>/dev/null || true
find "$V" -name '._*' -delete 2>/dev/null || true
sync
diskutil unmount "$V" >/dev/null
echo "seeded $IMG"
