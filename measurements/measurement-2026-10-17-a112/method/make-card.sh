#!/usr/bin/env bash
# make-card.sh <img> [acc] -- A112: 64 MiB raw MBR FAT16 card (volume A112C, not NEWTOS*) with C:\A112MESG.NAP and a
# NEWDESK.INF whose #Z line autoruns it; with "acc" also C:\A112ACC.NAC (loaded by the AES at session start).
# Never touches a real card; private mount point. (Adapted from measurement-2026-10-17-a25/method/make-card.sh.)
set -euo pipefail
IMG="${1:?}"; ACC="${2:-}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rm -f "$IMG"; dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" 2>/dev/null | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" A112C 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null; mkdir -p "$MNT"; diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
# with "acc": the -DA112_ACC build under the same name, plus the accessory
if [ "$ACC" = acc ]; then cp -X "$HERE/../bin/A112MSGA.NAP" "$MNT/A112MESG.NAP"; cp -X "$HERE/../bin/A112ACC.NAC" "$MNT/"
else cp -X "$HERE/../bin/A112MESG.NAP" "$MNT/"; fi
sed 's|^#Z .*|#Z 01 C:\\A112MESG.NAP@|' "$HERE/newdesk-template.inf" > "$MNT/NEWDESK.INF"
find "$MNT" -name '._*' -delete 2>/dev/null || true; dot_clean -m "$MNT" 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true; sync
grep '^#Z' "$MNT/NEWDESK.INF"; ls "$MNT"
