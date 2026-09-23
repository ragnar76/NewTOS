#!/usr/bin/env python3
"""Why `fsck_msdos -n` dies on the two whole-drive-wipe images.

usage:  lfn_orphan_probe.py IMG [--fix OUT.IMG]

Read-only by default: prints every root-directory slot of a raw MBR/FAT16
image, marking the LFN slots (attribute 0x0F) whose short entry is erased
(first byte 0xE5) -- i.e. ORPHAN long-name slots.

GEMDOS `Fdelete`/`Ddelete` erase the 8.3 entry and leave the preceding LFN
slots untouched (classic TOS had no long names at all).  On a card whose files
were written by macOS -- which gives `.fseventsd`, `.Spotlight-V100` and every
name that is not plain 8.3 an LFN chain -- a whole-drive delete therefore
leaves the chains behind.

With `--fix` the orphan slots are marked erased in a COPY of the image, which
is how the finding was proven: the same image then passes `fsck_msdos -n`
Phase 1/2/3 cleanly (`serial/fsck.txt`, section on ctl-nolfn.img).

This is a pre-existing GEMDOS filesystem-layer gap, not a desktop one: it
reproduces identically on the HEAD build (`d25-before.img`).
"""
import struct
import sys

img_path = sys.argv[1]
fix = sys.argv[3] if len(sys.argv) > 3 and sys.argv[2] == '--fix' else None

d = bytearray(open(img_path, 'rb').read())
start = struct.unpack_from('<I', d, 0x1BE + 8)[0] * 512
bps = struct.unpack_from('<H', d, start + 11)[0]
rsv = struct.unpack_from('<H', d, start + 14)[0]
nfat = d[start + 16]
rootent = struct.unpack_from('<H', d, start + 17)[0]
spf = struct.unpack_from('<H', d, start + 22)[0]
root = start + rsv * bps + nfat * spf * bps

orphans = 0
live = 0
erased = 0
for i in range(rootent):
    off = root + i * 32
    if d[off] == 0:
        break
    attr = d[off + 11]
    name = bytes(d[off:off + 11])
    if attr == 0x0F:
        # An LFN slot belongs to the next short entry that is not an LFN slot.
        j = off + 32
        while j < root + rootent * 32 and d[j + 11] == 0x0F:
            j += 32
        orphan = d[j] == 0xE5 or d[j] == 0
        orphans += orphan
        print(f"  slot {i:3d} LFN  {'ORPHAN' if orphan else 'live  '} {name!r}")
        if orphan and fix:
            d[off] = 0xE5
    elif d[off] == 0xE5:
        erased += 1
        print(f"  slot {i:3d} 8.3  erased {name!r}")
    else:
        live += 1
        print(f"  slot {i:3d} 8.3  live   {name!r} attr={attr:#04x}")

print(f"root: {live} live, {erased} erased, {orphans} ORPHAN LFN slots")
if fix:
    open(fix, 'wb').write(bytes(d))
    print("wrote", fix, "with the orphan LFN slots marked erased")
