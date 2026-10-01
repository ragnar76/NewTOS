#!/usr/bin/env bash
# sd-put.sh IMG  <src-file> <dest-rel-path> [<src> <dest> ...]
# Copies files into the FAT16 image (attach, copy, detach). Ejects cleanly.
set -euo pipefail
IMG="$1"; shift
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage "$IMG" | awk '/FAT_16|Windows_FAT_16|DOS_FAT_16/{print $1; exit}')
if [ -z "$DEV" ]; then
  DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage "$IMG" | tail -1 | awk '{print $1}')
fi
MP=$(mount | awk -v d="$DEV" '$1==d{print $3}')
echo "mounted $DEV at $MP"
while [ $# -ge 2 ]; do
  src="$1"; dst="$2"; shift 2
  mkdir -p "$MP/$(dirname "$dst")"
  cp "$src" "$MP/$dst"
  echo "  put $src -> $dst"
done
sync
diskutil unmount "$DEV" >/dev/null
hdiutil detach "${DEV%s1}" >/dev/null 2>&1 || hdiutil detach "$DEV" >/dev/null
echo "detached"
