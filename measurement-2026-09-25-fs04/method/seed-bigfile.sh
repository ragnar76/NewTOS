#!/usr/bin/env bash
# seed-bigfile.sh <img>  -- add C:\BIG1M.BIN (1 MiB) so the 32-KiB copy buffer
# is exercised 32 times over, i.e. the question "does a 32-KiB Fread survive as
# one transfer, or fall apart into single sectors?" gets a countable answer.
set -euo pipefail
IMG="$1"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" 2>/dev/null | head -1 | awk '{print $1}')
trap 'hdiutil detach "$DEV" >/dev/null 2>&1 || true' EXIT
diskutil mount "${DEV}s1" >/dev/null
V=$(diskutil info "${DEV}s1" | awk -F': *' '/Mount Point/{print $2}')
head -c 1048576 /dev/urandom > "$V/BIG1M.BIN"
sync
diskutil unmount "${DEV}s1" >/dev/null
hdiutil detach "$DEV" >/dev/null
trap - EXIT
echo "seeded C:\\BIG1M.BIN in $IMG"
