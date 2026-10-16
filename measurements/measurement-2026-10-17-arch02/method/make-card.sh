#!/usr/bin/env bash
# make-card.sh <out.img> <kind> <files...> -- ARCH02 base cards (QEMU raspi4b only): 64 MiB, MBR + one FAT16 partition
# (type 0x06, as tools/make-sd-image.sh), volume label ARCH02 (never NEWTOS: no clash with a real card's mount),
# mounted where diskutil puts it and checked by name. Copied from measurement-2026-10-17-mem02/method/make-card.sh.
#   kind fp   <FPPEXEC.NCP>                      C:\AUTO\FPPEXEC.NCP (runs before the AES) + C:\FPPEXEC.NCP (its child);
#                                                no NEWDESK.INF -> the default desktop comes up afterwards.
#   kind api  <APITEST.NCP> <PTERMCH.NCP> <DTACHILD.NCP>
#                                                C:\APITEST.NCP + C:\PTERMCH.NCP + C:\DTACHILD.NCP, NEWDESK.INF = '#Z 01 C:\APITEST.NCP@'
#                                                (the EmuTOS #Z autorun line, as measurement-2026-10-17-mem02).
# Runs use per-run APFS clones (cp -c) of these.
set -euo pipefail
IMG="$1"; KIND="$2"; shift 2
rm -f "$IMG"; dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
trap 'hdiutil detach "$DEV" >/dev/null 2>&1 || true' EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" ARCH02 100% >/dev/null
V="$(diskutil info "${DEV}s1" | sed -n 's/^ *Mount Point: *//p')"
[ -d "$V" ] && [ "$(basename "$V")" = ARCH02 ] || { echo "error: unexpected mount point '$V'" >&2; exit 1; }
mdutil -i off "$V" >/dev/null 2>&1 || true
export COPYFILE_DISABLE=1
case "$KIND" in
  fp)  mkdir -p "$V/AUTO"; cp -X "$1" "$V/AUTO/FPPEXEC.NCP"; cp -X "$1" "$V/FPPEXEC.NCP";;
  api) cp -X "$1" "$V/APITEST.NCP"; cp -X "$2" "$V/PTERMCH.NCP"; cp -X "$3" "$V/DTACHILD.NCP"
       printf '#Z 01 C:\\APITEST.NCP@\r\n' > "$V/NEWDESK.INF";;
  *) echo "kind? $KIND" >&2; exit 2;;
esac
find "$V" -name '._*' -delete 2>/dev/null || true
rm -rf "$V/.fseventsd" "$V/.Spotlight-V100" "$V/.Trashes" "$V/.TemporaryItems" 2>/dev/null || true
ls -la "$V" "$V/AUTO" 2>/dev/null | grep -v '^total' || true
sync; diskutil unmount "${DEV}s1" >/dev/null
echo "fsck: $(fsck_msdos -n "${DEV}s1" 2>&1 | tr '\n' ' ')"
