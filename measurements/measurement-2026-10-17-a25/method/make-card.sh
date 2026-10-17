#!/usr/bin/env bash
# make-card.sh <img> -- A25: 64 MiB raw MBR FAT16 card (volume A25C, not NEWTOS*) with C:\A25REINI.NAP and a
# NEWDESK.INF whose #Z line autoruns it. Never touches a real card; private mount point.
set -euo pipefail
IMG="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -f "$IMG"; dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" 2>/dev/null | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" A25C 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null; mkdir -p "$MNT"; diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
cp -X "$HERE/../bin/A25REINI.NAP" "$MNT/"
sed 's|^#Z .*|#Z 01 C:\\A25REINI.NAP@|' "$HERE/newdesk-template.inf" > "$MNT/NEWDESK.INF"
find "$MNT" -name '._*' -delete 2>/dev/null || true; dot_clean -m "$MNT" 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true; sync
grep '^#Z' "$MNT/NEWDESK.INF"; ls "$MNT"
