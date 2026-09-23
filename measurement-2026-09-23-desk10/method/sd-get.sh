#!/usr/bin/env bash
# sd-get.sh IMG <rel-path-in-image> <dest-file>   (attach, copy out, detach)
set -euo pipefail
IMG="$1"; SRC="$2"; DST="$3"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage "$IMG" | awk '/FAT_16|Windows_FAT_16|DOS_FAT_16/{print $1; exit}')
MP=$(mount | awk -v d="$DEV" '$1==d{print $3}')
cp "$MP/$SRC" "$DST"
diskutil unmount "$DEV" >/dev/null
hdiutil detach "${DEV%s1}" >/dev/null 2>&1 || hdiutil detach "$DEV" >/dev/null
echo "got $SRC -> $DST"
