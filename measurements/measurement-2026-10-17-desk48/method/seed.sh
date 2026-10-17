#!/usr/bin/env bash
# seed.sh <image> -- DESK48 base card: fresh 64 MiB MBR + FAT16 image (private mount point, never a real card) with
# C:\README.TXT, C:\SUB\NOTE.TXT, C:\SUB\OTHER.TXT and method/NEWDESK.INF (INF=<file>: another one). Pattern of the desk62-fix seed.sh.
set -euo pipefail
IMG="${1:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -f "$IMG"
dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" D48TEST 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null
mkdir -p "$MNT"
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
mdutil -i off "$MNT" >/dev/null 2>&1 || true
printf 'DESK48 test card\r\n' > "$MNT/README.TXT"
mkdir "$MNT/SUB"
printf 'DESK48 note\r\n' > "$MNT/SUB/NOTE.TXT"
printf 'DESK48 other\r\n' > "$MNT/SUB/OTHER.TXT"
cp -X "${INF:-$HERE/NEWDESK.INF}" "$MNT/NEWDESK.INF"
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true
sync
ls -laR "$MNT"
shasum -a 256 "${INF:-$HERE/NEWDESK.INF}"
