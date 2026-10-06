#!/usr/bin/env bash
#
# seed-card-libtest.sh <image> -- minimal 64 MiB MBR + FAT16 QEMU card for
# TASK/LDG-M2: \AUTO\LIBTEST.NCP (runs before the AES) + the NEWDESK.INF
# template without a #Z line (plain desktop afterwards). Reduced from
# measurement-2026-10-06-ldg-m1/method/seed-card.sh (no QED/TeraDesk/Doom:
# LIBTEST needs none of them; keeps the QEMU card small). Deliberately NOT
# tools/make-sd-image.sh (it writes through the fixed path /Volumes/NEWTOS);
# the image is mounted on a private mount point next to it, no card touched.
set -euo pipefail
IMG="${1:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NCP="${NCP:-$HERE/../bin/LIBTEST.NCP}"
rm -f "$IMG"
dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" LDGM2 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null
mkdir -p "$MNT"
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
mdutil -i off "$MNT" >/dev/null 2>&1 || true
printf 'LDG-M2 libtest card\r\n' > "$MNT/README.TXT"
mkdir -p "$MNT/AUTO"
cp -X "$NCP" "$MNT/AUTO/LIBTEST.NCP"
sed '/^#Z/d' "$HERE/newdesk-template.inf" > "$MNT/NEWDESK.INF"
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true
sync
echo "seeded $IMG: \\AUTO\\LIBTEST.NCP ($(wc -c < "$NCP" | tr -d ' ') bytes)"
