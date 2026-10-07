#!/usr/bin/env bash
# seed-base.sh <image> -- ONE 64 MiB MBR + FAT16 QEMU base card for the
# FS08-CPUCLK boot runs (rule: one base image, a cp -c clone per run).
# C:\APITEST.NCP (sdk/apps/apitest) and C:\NEWDESK.INF = the vforkt-fix
# template WITHOUT a #Z line (plain desktop). The APITEST run puts a #Z line
# into its own clone (run-boot.sh). Own volume label, private mount point;
# never touches /Volumes/NEWTOS or any real card.
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
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" CPUCLK 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null
mkdir -p "$MNT"
diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
export COPYFILE_DISABLE=1
mdutil -i off "$MNT" >/dev/null 2>&1 || true
cp -X "$WS/sdk/apps/apitest/APITEST.NCP" "$MNT/APITEST.NCP"
sed '/^#Z/d' "$HERE/newdesk-template.inf" > "$MNT/NEWDESK.INF"
find "$MNT" -name '._*' -delete 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true
sync
echo "seeded $IMG"
