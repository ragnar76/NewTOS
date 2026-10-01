#!/usr/bin/env bash
# pull-log.sh <image> <file-on-card> <dest>
set -euo pipefail
IMG="${1:?}"; NAME="${2:?}"; DEST="${3:?}"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
cleanup() { hdiutil detach "$DEV" >/dev/null 2>&1 || true; }
trap cleanup EXIT
diskutil mount "${DEV}s1" >/dev/null
V=/Volumes/NEWTOS
[ -f "$V/$NAME" ] || { echo "no $NAME on the card"; diskutil unmount "$V" >/dev/null; exit 1; }
cp "$V/$NAME" "$DEST"
diskutil unmount "$V" >/dev/null
echo "pulled $DEST ($(wc -c < "$DEST" | tr -d ' ') bytes)"
