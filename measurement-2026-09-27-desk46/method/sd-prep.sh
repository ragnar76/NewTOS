#!/usr/bin/env bash
# sd-prep.sh IMG INF  -- put C:\NEWDESK.INF and create C:\D46\SYSINFO.NCP
set -euo pipefail
IMG="$1"; INF="$2"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage "$IMG" 2>/dev/null | awk '/FAT_16|Windows_FAT_16|DOS_FAT_16/{print $1; exit}')
MP=$(mount | awk -v d="$DEV" '$1==d{print $3}')
cp "$INF" "$MP/NEWDESK.INF"
mkdir -p "$MP/D46"
cp "$MP/SYSINFO.NCP" "$MP/D46/SYSINFO.NCP"
rm -f "$MP/QEDTRACE.LOG"
ls -l "$MP/D46"
sync
diskutil unmount "$DEV" >/dev/null
hdiutil detach "${DEV%s1}" >/dev/null 2>&1 || hdiutil detach "$DEV" >/dev/null
echo "prepared $IMG with $INF"
