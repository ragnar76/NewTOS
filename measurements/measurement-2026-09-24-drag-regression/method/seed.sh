#!/usr/bin/env bash
# seed.sh <image-path>
# Fresh tools/make-sd-image.sh plus the DESK13 + DESK19 + DESK24 fixture
# folders combined onto ONE image (folder names do not collide), so one
# card serves all four regression batches in this pass.
set -euo pipefail
IMG="${1:?usage: seed.sh <image-path>}"
WS=/Volumes/Daten/Development/PiRusTOS

"$WS/tools/make-sd-image.sh" "$IMG" 64 >/dev/null

DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
cleanup() { hdiutil detach "$DEV" >/dev/null 2>&1 || true; }
trap cleanup EXIT
diskutil mount "${DEV}s1" >/dev/null
V=/Volumes/NEWTOS
[ -d "$V" ] || { echo "mount failed"; exit 1; }
mdutil -i off "$V" >/dev/null 2>&1 || true
export COPYFILE_DISABLE=1

# DESK13 fixture (copy/overwrite conflict + delete)
mkdir -p "$V/DESKT13"
printf 'ALPHA from the root\r\n' > "$V/SAME.TXT"
printf 'BRAVO from the subfolder, which is a good deal longer than the root one\r\n' > "$V/DESKT13/SAME.TXT"
printf 'ONE\r\n' > "$V/DESKT13/ONE.TXT"
printf 'TWOX\r\n' > "$V/DESKT13/TWO.TXT"

# DESK19/20 fixture (drop-on-program + default-dir)
mkdir -p "$V/DESKT19"
printf 'PROOF: C:\\DESKT19\r\n'     > "$V/DESKT19/PROOF.TXT"
printf 'dropped file contents\r\n'  > "$V/DESKT19/DROPME.TXT"

# DESK24/25/26 fixture (folder merge/rename + drive delete + rebuild info line)
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
