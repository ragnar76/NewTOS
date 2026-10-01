#!/usr/bin/env bash
#
# seed-wheel.sh <image-path> <napdir> [inf]
#
# Build a fresh card image with tools/make-sd-image.sh (the single source of
# truth for the card layout -- never a hand-crafted image, never a reused copy)
# and add the GUI04 fixture:
#
#   C:\WHEELT.NAP   the vex_wheelv acceptance program (method/wheeltst.c)
#   C:\NEWDESK.INF  <- method/newdesk-wheel.inf, whose `#Z` autorun line starts
#                      WHEELT.NAP INSTEAD of our desktop -- the same mechanism
#                      that starts TeraDesk (measurement-2026-09-21-t04) and
#                      the PORT05 reproducer pair.
#
# Pass a different <inf> (e.g. the stock NEWDESK.INF that make-sd-image.sh
# writes) to get the plain desktop instead -- that is run B, where the wheel
# has to scroll a desktop window.
#
# Everything else on the card comes from make-sd-image.sh unchanged.
set -euo pipefail
IMG="${1:?usage: seed-wheel.sh <image-path> <napdir> [inf]}"
NAPDIR="${2:?usage: seed-wheel.sh <image-path> <napdir> [inf]}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INF="${3:-$HERE/newdesk-wheel.inf}"
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

cp "$NAPDIR/WHEELT.NAP" "$V/WHEELT.NAP"
if [ "$INF" != "-" ]; then
  cp "$INF" "$V/NEWDESK.INF"
fi

find "$V" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$V" 2>/dev/null || true
find "$V" -name '._*' -delete 2>/dev/null || true
sync
diskutil unmount "$V" >/dev/null
echo "seeded $IMG (inf=$(basename "$INF"))"
