#!/usr/bin/env bash
# pull-log.sh <image> <dest>   -- copy C:\BALL.LOG off the card
set -euo pipefail
IMG="${1:?}"; DEST="${2:?}"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
cleanup() { hdiutil detach "$DEV" >/dev/null 2>&1 || true; }
trap cleanup EXIT
diskutil mount "${DEV}s1" >/dev/null
V=/Volumes/NEWTOS
[ -f "$V/BALL.LOG" ] || { echo "no BALL.LOG on the card"; diskutil unmount "$V" >/dev/null; exit 1; }
cp "$V/BALL.LOG" "$DEST"
diskutil unmount "$V" >/dev/null
echo "pulled $DEST ($(wc -c < "$DEST" | tr -d ' ') bytes)"
