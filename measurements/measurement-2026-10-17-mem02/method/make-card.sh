#!/usr/bin/env bash
# make-card.sh <out.img> <MXPROBE.NCP> <BALL.NAP> -- MEM02 base card (QEMU raspi4b only): 64 MiB, MBR + one FAT16
# partition (type 0x06, as tools/make-sd-image.sh), volume MXBASE (never NEWTOS: no clash with a real card's mount),
# C:\AUTO\MXPROBE.NCP (runs before the AES), C:\BALL.NAP, NEWDESK.INF = the EmuTOS `#Z` autorun line for BALL.NAP
# (as measurements/measurement-2026-09-14-cookie/method/prep-card.sh). Runs use per-run APFS clones (cp -c) of it.
set -euo pipefail
IMG="$1"; PROBE="$2"; BALL="$3"
rm -f "$IMG"; dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
trap 'hdiutil detach "$DEV" >/dev/null 2>&1 || true' EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" MXBASE 100% >/dev/null
V="$(diskutil info "${DEV}s1" | sed -n 's/^ *Mount Point: *//p')"
[ -d "$V" ] && [ "$(basename "$V")" = MXBASE ] || { echo "error: unexpected mount point '$V'" >&2; exit 1; }
mdutil -i off "$V" >/dev/null 2>&1 || true
export COPYFILE_DISABLE=1
mkdir -p "$V/AUTO"
cp "$PROBE" "$V/AUTO/MXPROBE.NCP"
cp "$BALL" "$V/BALL.NAP"
printf '#Z 01 C:\\BALL.NAP@\r\n' > "$V/NEWDESK.INF"
find "$V" -name '._*' -delete 2>/dev/null || true
rm -rf "$V/.fseventsd" "$V/.Spotlight-V100" "$V/.Trashes" "$V/.TemporaryItems" 2>/dev/null || true
sync; diskutil unmount "${DEV}s1" >/dev/null
echo "fsck: $(fsck_msdos -n "${DEV}s1" 2>&1 | tr '\n' ' ')"
