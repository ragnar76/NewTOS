#!/usr/bin/env bash
# seed-u6a.sh <image> <newdesk.inf> <file>... -- DESK43 U6a: a fresh 64 MiB
# MBR + FAT16 card holding the given programs in C:\ and the given NEWDESK.INF.
# Pattern of measurement-2026-10-13-mem05-acceptance/method/seed-card.sh: own
# volume label, private mount point next to the image, never a real card.
set -euo pipefail
IMG="${1:?}"; INF="${2:?}"; shift 2
rm -f "$IMG"
dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" U6ATEST 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null
mkdir -p "$MNT"
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
mdutil -i off "$MNT" >/dev/null 2>&1 || true
printf 'DESK43 U6a test card\r\n' > "$MNT/README.TXT"
for f in "$@"; do cp -X "$f" "$MNT/"; done
cp -X "$INF" "$MNT/NEWDESK.INF"
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true
sync
echo "seeded $IMG: $(ls "$MNT" | tr '\n' ' ') #Z: $(grep '^#Z' "$MNT/NEWDESK.INF" || echo none)"
