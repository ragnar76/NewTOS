#!/usr/bin/env bash
#
# seed-pex.sh <image-path> <napdir> [inf]
#
# Build a fresh card image with tools/make-sd-image.sh (the single source of
# truth for the card layout -- never a hand-crafted image, never a reused copy)
# and add the PORT05 fixture:
#
#   C:\PEXPAR.NAP   the EL0 parent  (method/pexpar.c)
#   C:\PEXKID.NAP   the EL0 child   (method/pexkid.c)
#   C:\NEWDESK.INF  <- method/newdesk-pex.inf, whose `#Z` autorun line starts
#                      PEXPAR.NAP INSTEAD of our desktop -- the same mechanism
#                      that starts TeraDesk (measurement-2026-09-21-t04).
#
# Everything else on the card -- including the two desk accessories, which are
# EL0 processes in the cooperative run-set and therefore part of the state
# under test -- comes from make-sd-image.sh unchanged.
#
# With a 4th argument `noexit` the child installed as C:\PEXKID.NAP is the
# PEXKIDX build (same source, no `appl_exit()`) -- see pexkid.c's header for
# the separate accessory-barrier defect that call runs into.
set -euo pipefail
IMG="${1:?usage: seed-pex.sh <image-path> <napdir> [inf] [noexit]}"
NAPDIR="${2:?usage: seed-pex.sh <image-path> <napdir> [inf] [noexit]}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INF="${3:-$HERE/newdesk-pex.inf}"
KID="PEXKID.NAP"; PAR="PEXPAR.NAP"
case "${4:-}" in
  noexit) KID="PEXKIDX.NAP" ;;   # child without appl_exit()
  qed)    PAR="PEXPARQ.NAP" ;;   # parent launches C:\APPS\QED\QED.NAP
esac
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

cp "$NAPDIR/$PAR" "$V/PEXPAR.NAP"
cp "$NAPDIR/$KID" "$V/PEXKID.NAP"
cp "$INF" "$V/NEWDESK.INF"

find "$V" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$V" 2>/dev/null || true
find "$V" -name '._*' -delete 2>/dev/null || true
sync
diskutil unmount "$V" >/dev/null
echo "seeded $IMG (inf=$(basename "$INF"), parent=$PAR, child=$KID)"
