#!/usr/bin/env bash
#
# seed-auto.sh <image> <DEST=SRC>...
#
# Fresh QEMU card from tools/make-sd-image.sh (never a reused copy), plus the
# listed files: each argument is "<path on C: relative to root>=<host file>",
# e.g. AUTO/P10TEST.NCP=build/P10TEST.NCP. Recipe of
# measurement-2026-10-02-port06/method/seed-doom.sh; the image is mounted on a
# private mount point next to it, never under /Volumes. make-sd-image.sh is
# NOT changed.
set -euo pipefail
IMG="${1:?usage: seed-auto.sh <image> <DEST=SRC>...}"; shift
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
for spec in "$@"; do
  dst="${spec%%=*}"; src="${spec#*=}"
  mkdir -p "$MNT/$(dirname "$dst")"
  cp "$src" "$MNT/$dst"
  echo "  + C:\\$(echo "$dst" | tr / '\\') ($(wc -c < "$src" | tr -d ' ') B)"
done
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
find "$MNT" -name '._*' -delete 2>/dev/null || true
sync
echo "seeded $IMG"
