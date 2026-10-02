#!/usr/bin/env bash
#
# seed.sh <image> [<host file>=<card path> ...]
#
# GUI07: fresh QEMU card from tools/make-sd-image.sh (not changed), then copy
# the given files onto it. Card paths are relative to C:\ and use '/'.
# Mounted on a PRIVATE mount point next to the image -- never under /Volumes,
# so a real card mounted as NEWTOS/BOOT can never be touched (PORT06's
# seed-scale.sh pattern; PORT07's seed-ball.sh mounted at /Volumes/NEWTOS and
# is deliberately not reused).
set -euo pipefail
IMG="${1:?usage: seed.sh <image> [host=card ...]}"; shift
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
"$WS/tools/make-sd-image.sh" "$IMG" 64 >/dev/null
MNT="$(dirname "$IMG")/mnt.$$"
mkdir -p "$MNT"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
for pair in "$@"; do
    src="${pair%%=*}"; dst="${pair#*=}"
    mkdir -p "$MNT/$(dirname "$dst")"
    cp "$src" "$MNT/$dst"
done
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
find "$MNT" -name '._*' -delete 2>/dev/null || true
sync
echo "seeded $(basename "$IMG"): $*"
