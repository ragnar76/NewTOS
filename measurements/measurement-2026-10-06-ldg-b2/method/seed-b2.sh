#!/usr/bin/env bash
# seed-b2.sh <image>  -- trimmed copy of measurement-2026-10-06-ldg-m3's
# seed-card.sh: a fresh private 64 MiB MBR+FAT16 image (never a real card,
# never tools/make-sd-image.sh) holding only C:\LDGDEMO\ (the M3 binaries,
# ../../measurement-2026-10-06-ldg-m3/bin/LDGDEMO, unchanged) and a
# NEWDESK.INF whose #Z line autoruns C:\LDGDEMO\CLIENT\LDGDEMO.NAP.
set -euo pipefail
IMG="${1:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC="$HERE/../../measurement-2026-10-06-ldg-m3/bin/LDGDEMO"
rm -f "$IMG"
dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" B2TEST 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null
mkdir -p "$MNT"
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
mdutil -i off "$MNT" >/dev/null 2>&1 || true
mkdir -p "$MNT/LDGDEMO/CLIENT" "$MNT/LDGDEMO/LIBS"
cp -X "$SRC/CLIENT/LDGDEMO.NAP" "$SRC/CLIENT/CLIENT.RSC" "$MNT/LDGDEMO/CLIENT/"
cp -X "$SRC/LIBS/EX_LIB.NLD" "$SRC/LIBS/POPUP.NLD" "$MNT/LDGDEMO/LIBS/"
sed 's|^#Z .*|#Z 01 C:\\LDGDEMO\\CLIENT\\LDGDEMO.NAP@|' "$HERE/newdesk-template.inf" > "$MNT/NEWDESK.INF"
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true
sync
echo "seeded $IMG: $(grep '^#Z' "$MNT/NEWDESK.INF")"
