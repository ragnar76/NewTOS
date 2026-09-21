#!/usr/bin/env bash
# sd-get.sh IMG <src-rel-path> <dest-file> [...]   (read files out of the FAT16 image)
set -euo pipefail
IMG="$1"; shift
DEV=$(hdiutil attach -readonly -imagekey diskimage-class=CRawDiskImage "$IMG" 2>/dev/null | awk '/FAT_16|Windows_FAT_16|DOS_FAT_16/{print $1; exit}')
if [ -z "$DEV" ]; then
  DEV=$(hdiutil attach -readonly -imagekey diskimage-class=CRawDiskImage "$IMG" 2>/dev/null | tail -1 | awk '{print $1}')
fi
MP=$(mount | awk -v d="$DEV" '$1==d{print $3}')
echo "mounted $DEV at $MP"
while [ $# -ge 2 ]; do
  src="$1"; dst="$2"; shift 2
  cp "$MP/$src" "$dst"; echo "  got $src -> $dst"
done
diskutil unmount "$DEV" >/dev/null
hdiutil detach "${DEV%s1}" >/dev/null 2>&1 || hdiutil detach "$DEV" >/dev/null
echo "detached"
