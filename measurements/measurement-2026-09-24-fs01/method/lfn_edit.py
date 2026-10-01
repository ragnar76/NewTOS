#!/usr/bin/env python3
"""Copy a raw MBR/FAT16 card image and mark the named ROOT slots erased.

usage:  lfn_edit.py IN.IMG OUT.IMG SLOT[,SLOT...]

Used to isolate which orphan long-name slot of the "before" image makes
`fsck_msdos -n` die: the same image, one slot set at a time.
"""
import struct
import sys

src, dst, slots = sys.argv[1], sys.argv[2], sys.argv[3]
want = [int(x) for x in slots.split(',') if x != '']

d = bytearray(open(src, 'rb').read())
start = struct.unpack_from('<I', d, 0x1BE + 8)[0] * 512
bps = struct.unpack_from('<H', d, start + 11)[0]
rsv = struct.unpack_from('<H', d, start + 14)[0]
nfat = d[start + 16]
spf = struct.unpack_from('<H', d, start + 22)[0]
root = start + rsv * bps + nfat * spf * bps

for s in want:
    d[root + s * 32] = 0xE5
open(dst, 'wb').write(bytes(d))
print(f"{dst}: erased root slots {want}")
