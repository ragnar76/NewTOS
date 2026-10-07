#!/usr/bin/env bash
# ofam-fsck.sh <scratch> -- re-run desk3a (rename, Info dialog edit) and DOOM with the run clone KEPT,
# fsck_msdos -n on it, sha256, delete. (The first runs of both deleted their clone.)
set -uo pipefail
S="${1:?}"; HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; C="$S/cards"; export SOCKROOT="$S"; OFF=$((63 * 512))
fs() { local tag=$1; local R="$C/run-$tag.img"
  DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$R" 2>/dev/null | head -1 | awk '{print $1}')
  fsck_msdos -n "${DEV}s1" > "$HERE/../checks/$tag-fsck.txt" 2>&1; hdiutil detach "$DEV" >/dev/null 2>&1
  echo "fsck $tag:"; sed -n '3,$p' "$HERE/../checks/$tag-fsck.txt" | head -6
  shasum -a 256 "$R" | sed "s|  .*|  $tag run clone (deleted)|" >> "$HERE/../checks/sha256-scratch-images.txt"; rm -f "$R"; }
KEEP=1 APPEND=serial=on bash "$HERE/run-rpi4-drive-m3.sh" "$S/kinj-evb-desk3a.img" "$C/desk3a.img" desk3a-fsck --mark-delay 0.5 --end '\[m3\] MARK H_DRIVE_C' --end-delay 1 --timeout 150 | tail -1
fs desk3a-fsck
cp -c "$C/doom.img" "$C/doom-run.img"
sed 's|^#Z .*|#Z 01 C:\\DOOM\\DOOM.NAP@|' "$HERE/newdesk-template.inf" > "$S/NEWDESK.INF"
mcopy -o -i "$C/doom-run.img@@$OFF" "$S/NEWDESK.INF" ::NEWDESK.INF
KEEP=1 APPEND=serial=on bash "$HERE/run-rpi4-drive-m3.sh" "$S/kinj-doom.img" "$C/doom-run.img" doom-fsck --mark-delay 0.5 --end '\[m3\] MARK END' --end-delay 1 --timeout 400 | tail -1
rm -f "$C/doom-run.img"; fs doom-fsck
