#!/usr/bin/env bash
# seed.sh <image-path> [auto]
#
# Build a fresh card image with tools/make-sd-image.sh (it changed with DESK18
# and DESK19, so it is re-run per measurement and never copied from an older
# run), and -- with the second argument `auto` -- add the DESK28 fixture:
#
#   C:\AUTO\AUTOCHD.NCP   method/autochdir.c   -- Dsetpath("\SUB"), runs first
#   C:\AUTO\CURDIR.NCP    sdk/examples/curdir.c -- reports drive+path, runs next
#   C:\AUTO\PROOF.TXT     "PROOF: C:\AUTO"      -- the third discriminator
#
# The AUTO scan is directory order, and the two programs are written in that
# order, so AUTOCHD runs before CURDIR. The three PROOF.TXT copies then name
# the directory CURDIR actually ran in:
#
#   "PROOF: C:\ (root)"  -> the boot drive's root      (EmuTOS, bios.c:685-733)
#   "PROOF: C:\SUB"      -> the previous program's cd  (the leak, control build)
#   "PROOF: C:\AUTO"     -> the program's own folder   (neither; the ticket's
#                                                       withdrawn premise)
set -euo pipefail
IMG="${1:?usage: seed.sh <image-path> [auto]}"
MODE="${2:-plain}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS=/Volumes/Daten/Development/PiRusTOS

"$WS/tools/make-sd-image.sh" "$IMG" 64 >/dev/null

if [ "$MODE" != "auto" ]; then
  echo "seeded $IMG (plain)"
  exit 0
fi

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

cp "$(dirname "$HERE")/tmp/AUTOCHD.NCP" "$V/AUTO/AUTOCHD.NCP"
"$WS/sdk/build.sh" "$WS/sdk/examples/curdir.c" "$V/AUTO/CURDIR.NCP" >/dev/null
printf 'PROOF: C:\\AUTO\r\n' > "$V/AUTO/PROOF.TXT"

find "$V" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$V" 2>/dev/null || true
find "$V" -name '._*' -delete 2>/dev/null || true
sync
diskutil unmount "$V" >/dev/null
echo "seeded $IMG (auto fixture)"
