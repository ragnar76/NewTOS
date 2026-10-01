#!/usr/bin/env bash
# sd-prep-d50.sh IMG INF -- put C:\NEWDESK.INF and build C:\D50\:
#   C:\D50\SUB50\INSIDE.TXT   a folder to open in the same window
#   C:\D50\CMDLINE.NTP        the parameter-taking program (Cancel case)
#   C:\D50\HELLO.NCP          a console program that starts and ends
set -euo pipefail
IMG="$1"; INF="$2"
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage "$IMG" 2>/dev/null | awk '/FAT_16|Windows_FAT_16|DOS_FAT_16/{print $1; exit}')
MP=$(mount | awk -v d="$DEV" '$1==d{print $3}')
cp "$INF" "$MP/NEWDESK.INF"
mkdir -p "$MP/D50/SUB50"
printf 'A file inside SUB50.\r\n' > "$MP/D50/SUB50/INSIDE.TXT"
cp "$MP/CMDLINE.NTP" "$MP/D50/CMDLINE.NTP"
cp "$MP/HELLO.NCP" "$MP/D50/HELLO.NCP"
rm -f "$MP/QEDTRACE.LOG"
ls -lR "$MP/D50"
sync
diskutil unmount "$DEV" >/dev/null
hdiutil detach "${DEV%s1}" >/dev/null 2>&1 || hdiutil detach "$DEV" >/dev/null
echo "prepared $IMG with $INF"
