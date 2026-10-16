#!/usr/bin/env bash
# make-card.sh <img> -- DESK43 U10: the QEMU card of this run, a 64 MiB raw image with one MBR FAT16 partition
# (volume U10C, deliberately not NEWTOS*) holding only C:\README.TXT. Neither the desktop program (embedded) nor the
# console needs anything on C:. Each run takes an APFS clone (cp -c, run.py) and deletes it afterwards.
set -euo pipefail
IMG="${1:?}"
rm -f "$IMG"; dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" 2>/dev/null | head -1 | awk '{print $1}')
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" U10C 100% >/dev/null
MP=$(diskutil info "${DEV}s1" | sed -n 's/^ *Mount Point: *//p')
printf 'NewTOS DESK43 U10 QEMU card\r\n' > "$MP/README.TXT"
rm -rf "$MP/.Spotlight-V100" "$MP/.fseventsd" "$MP/.Trashes" "$MP/.TemporaryItems"; dot_clean -m "$MP" 2>/dev/null || true; sync
diskutil unmount "${DEV}s1" >/dev/null; hdiutil detach "$DEV" >/dev/null
