#!/usr/bin/env python3
"""Copy a raw MBR/FAT16 card image and plant ONE live 8.3 entry in the root.

usage:  plant83.py IN.IMG OUT.IMG SLOT NAME11 [ATTR]

NAME11 is the raw eleven-byte FAT name ("LIVE    TXT"). The entry gets start
cluster 0 and length 0, which is a legal empty file. Used to ask whether a
LIVE short entry *after* an orphan long-name chain changes what `fsck_msdos`
does with that chain.
"""
import struct
import sys

src, dst, slot, name = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
attr = int(sys.argv[5], 0) if len(sys.argv) > 5 else 0x20
assert len(name) == 11, "NAME11 must be exactly 11 characters"

d = bytearray(open(src, 'rb').read())
start = struct.unpack_from('<I', d, 0x1BE + 8)[0] * 512
bps = struct.unpack_from('<H', d, start + 11)[0]
rsv = struct.unpack_from('<H', d, start + 14)[0]
nfat = d[start + 16]
spf = struct.unpack_from('<H', d, start + 22)[0]
root = start + rsv * bps + nfat * spf * bps

e = bytearray(32)
e[0:11] = name.encode('latin1')
e[11] = attr
e[0x16:0x18] = (0x8000).to_bytes(2, 'little')
e[0x18:0x1A] = (0x5901).to_bytes(2, 'little')
d[root + slot * 32:root + slot * 32 + 32] = e
open(dst, 'wb').write(bytes(d))
print(f"{dst}: planted {name!r} attr={attr:#04x} in root slot {slot}")
