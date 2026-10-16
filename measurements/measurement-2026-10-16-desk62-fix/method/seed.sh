#!/usr/bin/env bash
# seed.sh <image> -- DESK62 base card: fresh 64 MiB MBR + FAT16 image (private mount point, never a real card)
# (the native #G *.NAP / #F *.NCP / #P *.NTP lines in the order of inf.rs default_type_anodes) with C:\QED.NAP + C:\qed.rsc (sdk/apps/qed/build), C:\README.TXT (the text file to drop) and method/NEWDESK.INF:
# one window C:\*.* (char cell 28,2 size 48x20) and a desktop shortcut #X to C:\QED.NAP at icon grid (1,0).
# Pattern of measurements/measurement-2026-10-16-desk43-u10-test/method/seed-card.sh.
set -euo pipefail
IMG="${1:?}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WS="$(cd "$HERE/../../.." && pwd)"
rm -f "$IMG"
dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" D62TEST 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null
mkdir -p "$MNT"
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
mdutil -i off "$MNT" >/dev/null 2>&1 || true
printf 'DESK62 test card\r\n' > "$MNT/README.TXT"
cp -X "$WS/sdk/apps/qed/build/QED.NAP" "$WS/sdk/apps/qed/build/qed.rsc" "$MNT/"
cp -X "${INF:-$HERE/NEWDESK.INF}" "$MNT/NEWDESK.INF"   # INF=<file>: another NEWDESK.INF (desk62-fix: NEWDESK-D2W.INF)
find "$MNT" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$MNT" 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true
sync
ls -la "$MNT"
shasum -a 256 "$WS/sdk/apps/qed/build/QED.NAP" "$WS/sdk/apps/qed/build/qed.rsc" "$HERE/NEWDESK.INF"
