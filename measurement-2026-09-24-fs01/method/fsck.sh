#!/usr/bin/env bash
# fsck.sh <image> ...  -- `fsck_msdos -n` on the FAT16 partition of each raw
# card image (the image starts with the MBR, so fsck has to be pointed at the
# partition device, not the file).
#
# Unlike the DESK24/25/26 copy this runs fsck_msdos under `script`, i.e. on a
# pty: its stdout is then line-buffered. Piping it into `sed` (what the
# earlier run did) makes stdout a fully buffered pipe, and a process that dies
# on a signal never flushes that buffer -- which is why the two rc=133 images
# looked as if they had produced "no output at all".
set -uo pipefail
for IMG in "$@"; do
  echo "== $(basename "$IMG")"
  DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" 2>/dev/null | head -1 | awk '{print $1}')
  [ -n "$DEV" ] || { echo "   attach failed"; continue; }
  OUT=$(mktemp)
  script -q "$OUT" fsck_msdos -n "${DEV}s1" >/dev/null 2>&1
  RC=$?
  sed 's/^/   /' "$OUT"
  rm -f "$OUT"
  echo "   rc=$RC"
  hdiutil detach "$DEV" >/dev/null 2>&1
done
