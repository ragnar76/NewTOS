#!/usr/bin/env bash
#
# pull.sh <image> <card path> <dest>   -- copy one file off a QEMU card image.
# Private mount point next to the image, never under /Volumes (see seed.sh).
set -euo pipefail
IMG="${1:?}"; SRC="${2:?}"; DEST="${3:?}"
MNT="$(dirname "$IMG")/pull.$$"
mkdir -p "$MNT"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
if [ -f "$MNT/$SRC" ]; then cp "$MNT/$SRC" "$DEST"; echo "pulled $SRC ($(wc -c < "$DEST" | tr -d ' ') bytes)"; else echo "no $SRC on the card"; fi
