#!/usr/bin/env bash
# seed.sh <image-path> [qedroot]
#
# Build a fresh card image with tools/make-sd-image.sh (the single source of
# truth for the card -- never hand-craft one), and -- with the second argument
# `qedroot` -- add the DESK35 fixtures:
#
#   C:\GEMWIN.NAP  method/gemwin.c, built with sdk/build.sh -- the smallest GEM
#                  program that paints only its own window AND terminates by
#                  itself (see the file's header for why neither DUMPVDI.NAP nor
#                  QED.NAP can play that role)
#   C:\QED.NAP     a copy of C:\Apps\QED\QED.NAP  (the GEM editor make-sd-image
#   C:\qed.rsc     a copy of C:\Apps\QED\qed.rsc   installs two levels down)
#
# Why the copy: the DESK35 action list has to double-click the program in a
# window, and the click coordinates are baked into the script. In the "before"
# build every folder step opens a NEW window (that is DESK34), so navigating
# C:\ -> Apps -> QED would put the program icon at a different place in the two
# builds and the before/after pictures would no longer be the same gesture.
# With the program in the root, both builds make exactly the same three clicks.
# QED finds its resource through `sh_find`'s app-directory step, so the .rsc
# has to travel with it.
set -euo pipefail
IMG="${1:?usage: seed.sh <image-path> [qedroot]}"
MODE="${2:-plain}"
WS=/Volumes/Daten/Development/PiRusTOS

"$WS/tools/make-sd-image.sh" "$IMG" 64 >/dev/null

if [ "$MODE" != "qedroot" ]; then
  echo "seeded $IMG (plain)"
  exit 0
fi

DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
cleanup() { hdiutil detach "$DEV" >/dev/null 2>&1 || true; }
trap cleanup EXIT
diskutil mount "${DEV}s1" >/dev/null
V=/Volumes/NEWTOS
[ -d "$V" ] || { echo "mount failed"; exit 1; }
# macOS writes AppleDouble `._NAME` companions on a re-mounted FAT volume; they
# would show up as extra window items. Off, and swept again before unmounting.
mdutil -i off "$V" >/dev/null 2>&1 || true
export COPYFILE_DISABLE=1

cp "$V/Apps/QED/QED.NAP" "$V/QED.NAP"
cp "$V/Apps/QED/qed.rsc" "$V/qed.rsc"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"$WS/sdk/build.sh" "$HERE/gemwin.c" "$V/GEMWIN.NAP" >/dev/null

find "$V" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$V" 2>/dev/null || true
find "$V" -name '._*' -delete 2>/dev/null || true
sync
diskutil unmount "$V" >/dev/null
echo "seeded $IMG (QED in the root)"
