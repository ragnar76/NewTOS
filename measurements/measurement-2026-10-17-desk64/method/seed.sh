#!/usr/bin/env bash
# seed.sh <image> -- DESK64 base card: fresh 64 MiB MBR + FAT16 image (private mount point, never a real card) with
# method/NEWDESK.INF (one window C:\*.* at char cell 28,2 size 48x20; #Q = the EmuTOS defaults; #K with the BACKGRND
# slot 21 = 'B' so Ctrl+B opens Set background) and C:\README.TXT. Pattern of ../../measurement-2026-10-16-desk62-fix/method/seed.sh
# without QED.
set -euo pipefail
IMG="${1:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -f "$IMG"
dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" D64TEST 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null
mkdir -p "$MNT"
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
mdutil -i off "$MNT" >/dev/null 2>&1 || true
printf 'DESK64 test card\r\n' > "$MNT/README.TXT"
cp -X "${INF:-$HERE/NEWDESK.INF}" "$MNT/NEWDESK.INF"   # INF=<file>: another NEWDESK.INF (run kr: the INF run kb saved)
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true
sync
ls -la "$MNT"
shasum -a 256 "${INF:-$HERE/NEWDESK.INF}"
