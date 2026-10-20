#!/usr/bin/env bash
# mkcard.sh <img> -- fresh 64 MiB MBR + FAT16 QEMU card (label SDK01D, private mount point, never a
# real card; pattern of measurement-2026-10-20-sdk01a-test/method/mkcard.sh). The two templates are
# built with `make` (sdk/mk/newtos.mk -> sdk/bin/newtos-cc) and put on the card with the include's own
# `make card DEST=...`: HELLO.NAP to C:\ (+ NEWDESK.INF autorun line #Z), HELLO.NCP to C:\AUTO\.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"; . "$HERE/env.sh"
IMG="${1:?}"
rm -f "$IMG"; dd if=/dev/zero of="$IMG" bs=1m count=64 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
MNT="$(dirname "$IMG")/mnt.$$"
cleanup() { diskutil unmount "$MNT" >/dev/null 2>&1 || true; hdiutil detach "$DEV" >/dev/null 2>&1 || true; rmdir "$MNT" 2>/dev/null || true; }
trap cleanup EXIT
diskutil partitionDisk "$DEV" 1 MBR "MS-DOS FAT16" SDK01D 100% >/dev/null
diskutil unmount "${DEV}s1" >/dev/null
mkdir -p "$MNT"; diskutil mount -mountPoint "$MNT" "${DEV}s1" >/dev/null
mdutil -i off "$MNT" >/dev/null 2>&1 || true
for t in gem-hello console-hello; do make -C "$WS/sdk/templates/$t" clean all; done
make -C "$WS/sdk/templates/gem-hello" card DEST="$MNT"
make -C "$WS/sdk/templates/console-hello" card DEST="$MNT/AUTO"
printf '#R 02\r\n#Z 01 C:\\HELLO.NAP@\r\n' > "$MNT/NEWDESK.INF"
find "$MNT" -name "._*" -print -delete 2>/dev/null || true
rm -rf "$MNT/.fseventsd" "$MNT/.Spotlight-V100" "$MNT/.Trashes" 2>/dev/null || true
sync
echo "== card content"; (cd "$MNT" && find . 2>/dev/null | sort) || true
echo "== sha256"; (cd "$MNT" && shasum -a 256 HELLO.NAP AUTO/HELLO.NCP NEWDESK.INF)
