#!/usr/bin/env bash
#
# seed.sh <image> <NEWDESK.INF> <dir with CRCHILD.NAP + CRASHTST.NAP>
#
# A minimal fresh QEMU card for the 0c runs: 64 MiB, MBR + one FAT16
# partition (the layout tools/make-sd-image.sh builds), holding only
# NEWDESK.INF (#Z autorun line), C:\CRCHILD.NAP and C:\CRASHTST.NAP.
# Deliberately NOT tools/make-sd-image.sh: that script writes through the
# fixed path /Volumes/NEWTOS; this one uses its own volume label and mounts
# the image on a private mount point next to it, never touching any card.
set -euo pipefail
IMG="${1:?}"; INF="${2:?}"; PROGS="${3:?}"
rm -f "$IMG"
dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" C0CTEST 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null
mkdir -p "$MNT"
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
mdutil -i off "$MNT" >/dev/null 2>&1 || true
printf 'ARCH03 0c crash test card\r\n' > "$MNT/README.TXT"
cp "$INF" "$MNT/NEWDESK.INF"
cp "$PROGS/CRCHILD.NAP" "$MNT/CRCHILD.NAP"
cp "$PROGS/CRASHTST.NAP" "$MNT/CRASHTST.NAP"
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true
sync
ls -la "$MNT"
echo "seeded $IMG with $(basename "$INF")"
