#!/usr/bin/env bash
# seed.sh <image-path>
#
# Build a fresh card with tools/make-sd-image.sh (the single source of truth
# for a NewTOS card) and add a deliberate long-name fixture on the root:
#
#   C:\A very long file name.txt               -> 8.3 AVERYL~1.TXT + LFN chain
#   C:\A very long folder name\                -> 8.3 AVERYL~2     + LFN chain
#   C:\A very long folder name\inside long.txt -> 8.3 INSIDE~1.TXT + LFN chain
#
# so the run covers a file in the root, a folder in the root and a file in a
# sub-directory. The card also carries the long names macOS itself writes
# (`.fseventsd`, `._*` companions) -- that is where the original finding's
# orphan slots came from.
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
mdutil -i off "$V" >/dev/null 2>&1 || true
export COPYFILE_DISABLE=1

printf 'a long name on the root\r\n'   > "$V/A very long file name.txt"
mkdir -p "$V/A very long folder name"
printf 'a long name in a folder\r\n'   > "$V/A very long folder name/inside long.txt"

find "$V" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$V" 2>/dev/null || true
find "$V" -name '._*' -delete 2>/dev/null || true
sync
diskutil unmount "$V" >/dev/null
echo "seeded $IMG"
