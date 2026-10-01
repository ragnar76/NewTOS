#!/usr/bin/env bash
# seed-tera.sh <image-path>
#
# The TeraDesk half of the PORT03 verification: the SAME card as seed.sh, with
# C:\NEWDESK.INF replaced by the #Z autorun line that starts TeraDesk
# (method/newdesk-tera.inf, taken unchanged from measurement-2026-09-21-t04 --
# the mechanism the project decided on, not a hand-made launch).
# TeraDesk links cflib.a as well, so it is the second cflib client the pin swap
# has to leave standing.
set -euo pipefail
IMG="${1:?usage: seed-tera.sh <image-path>}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
cleanup() { hdiutil detach "$DEV" >/dev/null 2>&1 || true; }
trap cleanup EXIT
diskutil mount "${DEV}s1" >/dev/null
V=/Volumes/NEWTOS
[ -d "$V" ] || { echo "mount failed"; exit 1; }
mdutil -i off "$V" >/dev/null 2>&1 || true
export COPYFILE_DISABLE=1

cp "$HERE/newdesk-tera.inf" "$V/NEWDESK.INF"

find "$V" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$V" 2>/dev/null || true
find "$V" -name '._*' -delete 2>/dev/null || true
sync
diskutil unmount "$V" >/dev/null
echo "seeded (TeraDesk autorun) $IMG"
