#!/usr/bin/env bash
#
# seed-port09.sh <image-path> <napdir> <parent>
#
# Fresh card image from tools/make-sd-image.sh (never a reused copy) plus the
# PORT09 probe: C:\ENVPAR.NAP (= <napdir>/<parent>, ENVPAR.NAP or ENVPARA.NAP),
# C:\ENVKID.NAP, and a NEWDESK.INF whose #Z autorun line starts C:\ENVPAR.NAP
# instead of our desktop (recipe of measurement-2026-09-25-port05/method/
# seed-pex.sh). C:\ARGVTEST.NAP is already on the image (make-sd-image.sh).
set -euo pipefail
IMG="${1:?}"; NAPDIR="${2:?}"; PAR="${3:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
"$WS/tools/make-sd-image.sh" "$IMG" 64 >/dev/null
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
cleanup() { hdiutil detach "$DEV" >/dev/null 2>&1 || true; }
trap cleanup EXIT
diskutil mount "${DEV}s1" >/dev/null
V=/Volumes/NEWTOS
[ -d "$V" ] || { echo "mount failed"; exit 1; }
mdutil -i off "$V" >/dev/null 2>&1 || true
export COPYFILE_DISABLE=1
cp "$NAPDIR/$PAR" "$V/ENVPAR.NAP"
cp "$NAPDIR/ENVKID.NAP" "$V/ENVKID.NAP"
sed 's/PEXPAR/ENVPAR/' "$WS/measurements/measurement-2026-09-25-port05/method/newdesk-pex.inf" > "$V/NEWDESK.INF"
find "$V" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$V" 2>/dev/null || true
find "$V" -name '._*' -delete 2>/dev/null || true
sync
diskutil unmount "$V" >/dev/null
echo "seeded $IMG (parent=$PAR)"
