#!/usr/bin/env bash
#
# FS04: a FOUR-partition FAT16 card image, the shape of the physical card
# (C:-F:, 4 x 500 MB).  tools/make-sd-image.sh builds ONE partition, which
# cannot reproduce a copy that CROSSES a partition boundary -- that is what
# this script adds, and nothing else: the content of C: is taken verbatim
# from a standard tools/make-sd-image.sh image, so the desktop that comes up
# is the same desktop every other measurement uses.
#
# Total size 2048 MiB = a power of two (QEMU's SD model rejects other sizes).
# Partition 1..3 are 500 MiB each -- the same size as on the physical card,
# so newfs_msdos picks the same cluster size and the same FAT length; the
# remainder goes to partition 4.
#
# Usage: make-sd-image-4part.sh <repo> <out.img> <tmpdir>
set -euo pipefail
REPO="${1:?repo}"; IMG="${2:?image path}"; TMP="${3:?tmpdir}"
STD="$TMP/std-1part.img"

# 1. the standard one-partition image, for its CONTENT
if [ ! -f "$STD" ] || [ "${FS04_REBUILD_STD:-0}" = "1" ]; then
  "$REPO/tools/make-sd-image.sh" "$STD" 64 >"$TMP/make-sd-image.log" 2>&1
  echo "standard image built"
else
  echo "standard image reused: $STD"
fi

# 2. the four-partition card
rm -f "$IMG"
dd if=/dev/zero of="$IMG" bs=1m count=2048 status=none
DEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$IMG" | head -1 | awk '{print $1}')
SDEV=""
cleanup() {
  [ -n "$SDEV" ] && hdiutil detach "$SDEV" >/dev/null 2>&1 || true
  hdiutil detach "$DEV" >/dev/null 2>&1 || true
}
trap cleanup EXIT
diskutil partitionDisk "$DEV" 4 MBR \
  "MS-DOS FAT16" FS04C 500M \
  "MS-DOS FAT16" FS04D 500M \
  "MS-DOS FAT16" FS04E 500M \
  "MS-DOS FAT16" FS04F R >/dev/null
for v in FS04C FS04D FS04E FS04F; do mdutil -i off "/Volumes/$v" >/dev/null 2>&1 || true; done

# 3. content of the standard image -> partition 1
SDEV=$(hdiutil attach -imagekey diskimage-class=CRawDiskImage -nomount "$STD" | head -1 | awk '{print $1}')
diskutil mount "${SDEV}s1" >/dev/null
SRC=$(diskutil info "${SDEV}s1" | awk -F': *' '/Mount Point/{print $2}')
echo "source volume: $SRC"
(cd "$SRC" && tar cf - --exclude .Spotlight-V100 --exclude .fseventsd \
     --exclude .Trashes --exclude .TemporaryItems --exclude ._\* .) \
  | (cd /Volumes/FS04C && tar xf -)
sync
for v in FS04C FS04D FS04E FS04F; do
  rm -rf "/Volumes/$v/.Spotlight-V100" "/Volumes/$v/.fseventsd" "/Volumes/$v/.Trashes" "/Volumes/$v/.TemporaryItems" 2>/dev/null || true
  dot_clean -m "/Volumes/$v" 2>/dev/null || true
done
# a small text file on D: as well, so a copy D:->C: is available too
printf 'FS04 probe file on D:\r\n' > /Volumes/FS04D/DPROBE.TXT
sync
diskutil unmount "${SDEV}s1" >/dev/null
hdiutil detach "$SDEV" >/dev/null; SDEV=""
for n in 1 2 3 4; do diskutil unmount "${DEV}s${n}" >/dev/null 2>&1 || true; done
hdiutil detach "$DEV" >/dev/null
trap - EXIT

python3 - "$IMG" <<'PY'
import sys, struct
d = open(sys.argv[1], 'rb').read(512)
assert d[510] == 0x55 and d[511] == 0xAA, "missing 0x55AA"
for i in range(4):
    e = 0x1BE + i*16
    typ = d[e+4]
    start = struct.unpack('<I', d[e+8:e+12])[0]
    cnt = struct.unpack('<I', d[e+12:e+16])[0]
    print(f"part{i}: type=0x{typ:02x} start_lba={start} sectors={cnt} ({cnt*512/1048576:.1f} MiB)")
PY
echo "done: $IMG"
