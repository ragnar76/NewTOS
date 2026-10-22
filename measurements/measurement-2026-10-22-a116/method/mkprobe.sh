#!/usr/bin/env bash
# mkprobe.sh <img> -- A116 probe card: 64 MiB MBR FAT16 (label A116C, never a real card), C:\A116W.NAP autorun by the #Z line of
# C:\NEWDESK.INF, C:\A116ACC.NAC loaded by the AES at session start. Probe binaries from $S/bin (built by build-probe.sh).
# (Adapted from measurement-2026-10-17-a112/method/make-card.sh.)
set -euo pipefail
IMG="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
rm -f "$IMG"; dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" 2>/dev/null | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" A116C 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null; mkdir -p "$MNT"; diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
cp -X "$S/bin/A116W.NAP" "$S/bin/A116ACC.NAC" "$MNT/"
sed 's|^#Z .*|#Z 01 C:\\A116W.NAP@|' "$HERE/probe-newdesk-template.inf" > "$MNT/NEWDESK.INF"
find "$MNT" -name '._*' -delete 2>/dev/null || true; dot_clean -m "$MNT" 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true; sync
grep '^#Z' "$MNT/NEWDESK.INF"; ls "$MNT"
