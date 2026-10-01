#!/usr/bin/env bash
# sd-prep-d47.sh IMG INF -- put C:\NEWDESK.INF and create C:\D47\CMDLINE.NTP
set -euo pipefail
IMG="$1"; INF="$2"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage "$IMG" 2>/dev/null | awk '/FAT_16|Windows_FAT_16|DOS_FAT_16/{print $1; exit}')
MP=$(mount | awk -v d="$DEV" '$1==d{print $3}')
cp "$INF" "$MP/NEWDESK.INF"
mkdir -p "$MP/D47"
cp "$MP/CMDLINE.NTP" "$MP/D47/CMDLINE.NTP"
rm -f "$MP/QEDTRACE.LOG"
ls -l "$MP/D47"
sync
diskutil unmount "$DEV" >/dev/null
hdiutil detach "${DEV%s1}" >/dev/null 2>&1 || hdiutil detach "$DEV" >/dev/null
echo "prepared $IMG with $INF"
