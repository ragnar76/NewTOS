#!/usr/bin/env bash
# seed-present-card.sh <image> -- 32 MiB MBR+FAT16 QEMU card with C:\Apps\PRESENT\{PRESENT.NAP,SLIDES.TXT,*.PNG}
# SLIDES.TXT = docs/presentation/ofam-2026-slides.txt with "@spacing 5" prepended (as on the real card);
# NEWDESK.INF autoruns C:\Apps\PRESENT\PRESENT.NAP (#Z). Private mount point, never a real card.
set -euo pipefail
IMG="${1:?}"; NAP="${NAP:?PRESENT.NAP path}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="${WS:?workspace}"
rm -f "$IMG"; dd if=/dev/zero of="$IMG" bs=1m count=32 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" PRESTEST 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null
mkdir -p "$MNT"; diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
mkdir -p "$MNT/Apps/PRESENT"
cp -X "$NAP" "$MNT/Apps/PRESENT/PRESENT.NAP"
( echo "@spacing 5"; cat "$WS/docs/presentation/ofam-2026-slides.txt" ) > "$MNT/Apps/PRESENT/SLIDES.TXT"
cp -X "$WS"/docs/presentation/images/*.PNG "$MNT/Apps/PRESENT/"
sed 's|^#Z .*|#Z 01 C:\\Apps\\PRESENT\\PRESENT.NAP@|' "$HERE/newdesk-template.inf" > "$MNT/NEWDESK.INF"
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true
sync
grep '^#Z' "$MNT/NEWDESK.INF"; ls -l "$MNT/Apps/PRESENT"
