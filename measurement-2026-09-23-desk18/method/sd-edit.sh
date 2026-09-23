#!/usr/bin/env bash
# sd-edit.sh IMG  (put <src> <dest-rel> | rm <dest-rel>)...
# sd-put.sh plus a delete, for the DESK14 "file not found" card.
# Attaches the image the agent itself created, edits it, detaches it again;
# it never touches a physical card.
set -euo pipefail
IMG="$1"; shift
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage "$IMG" | awk '/FAT_16|Windows_FAT_16|DOS_FAT_16/{print $1; exit}')
if [ -z "$DEV" ]; then
  DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage "$IMG" | tail -1 | awk '{print $1}')
fi
MP=$(mount | awk -v d="$DEV" '$1==d{print $3}')
echo "mounted $DEV at $MP"
while [ $# -ge 1 ]; do
  case "$1" in
    put) src="$2"; dst="$3"; shift 3
         mkdir -p "$MP/$(dirname "$dst")"; cp "$src" "$MP/$dst"; echo "  put $src -> $dst" ;;
    rm)  dst="$2"; shift 2
         rm -f "$MP/$dst"; echo "  rm $dst" ;;
    *)   echo "unknown op $1" >&2; exit 2 ;;
  esac
done
dot_clean -m "$MP" 2>/dev/null || true
sync
diskutil unmount "$DEV" >/dev/null
hdiutil detach "${DEV%s1}" >/dev/null 2>&1 || hdiutil detach "$DEV" >/dev/null
echo "detached"
