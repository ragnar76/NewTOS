#!/usr/bin/env bash
# fsck.sh <image> ...  -- `fsck_msdos -n` on the FAT16 partition of each raw
# card image (the image itself starts with the MBR, so fsck has to be pointed
# at the partition device, not the file).
set -uo pipefail
for IMG in "$@"; do
  echo "== $IMG"
  DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" 2>/dev/null | head -1 | awk '{print $1}')
  [ -n "$DEV" ] || { echo "   attach failed"; continue; }
  fsck_msdos -n "${DEV}s1" 2>&1 | sed 's/^/   /'
  hdiutil detach "$DEV" >/dev/null 2>&1
done
