#!/usr/bin/env python3
"""List a FAT16 directory of a raw MBR image, and optionally dump a file.

usage: lsimg.py IMG [DIRPATH] [--cat FILE]
Read-only; the image is never modified. Used to check what a desktop file
operation actually wrote, without mounting anything.
"""
import sys, struct

img = open(sys.argv[1], 'rb').read()
args = sys.argv[2:]
cat = None
if '--cat' in args:
    i = args.index('--cat'); cat = args[i+1]; args = args[:i]
path = args[0] if args else ''

start = struct.unpack_from('<I', img, 0x1BE + 8)[0] * 512
bps = struct.unpack_from('<H', img, start + 11)[0]
spc = img[start + 13]
rsv = struct.unpack_from('<H', img, start + 14)[0]
nfat = img[start + 16]
rootent = struct.unpack_from('<H', img, start + 17)[0]
spf = struct.unpack_from('<H', img, start + 22)[0]
fat = start + rsv * bps
root = fat + nfat * spf * bps
data = root + rootent * 32
def cluster(n):
    return data + (n - 2) * spc * bps
def fatnext(n):
    return struct.unpack_from('<H', img, fat + n * 2)[0]
def entries(off, cl=None):
    out = []
    if cl is None:
        blob = img[off:off + rootent * 32]
    else:
        blob = b''
        while 2 <= cl < 0xFFF8:
            blob += img[cluster(cl):cluster(cl) + spc * bps]
            cl = fatnext(cl)
    for i in range(0, len(blob), 32):
        e = blob[i:i+32]
        if not e or e[0] == 0:
            break
        if e[0] == 0xE5 or e[11] & 0x0F == 0x0F:
            continue
        name = e[0:8].decode('latin1').rstrip()
        ext = e[8:11].decode('latin1').rstrip()
        nm = name + ('.' + ext if ext else '')
        out.append((nm, e[11], struct.unpack_from('<H', e, 26)[0],
                    struct.unpack_from('<I', e, 28)[0],
                    struct.unpack_from('<H', e, 24)[0],
                    struct.unpack_from('<H', e, 22)[0]))
    return out
cur = entries(root)
curcl = None
for part in [p for p in path.split('\\') if p]:
    hit = [e for e in cur if e[0].upper() == part.upper()]
    if not hit:
        print("no such directory:", part); sys.exit(1)
    curcl = hit[0][2]
    cur = entries(None, curcl)
if cat:
    hit = [e for e in cur if e[0].upper() == cat.upper()]
    if not hit:
        print("ABSENT:", cat); sys.exit(2)
    nm, attr, cl, size, date, time = hit[0]
    blob = b''
    while 2 <= cl < 0xFFF8:
        blob += img[cluster(cl):cluster(cl) + spc * bps]
        cl = fatnext(cl)
    print(f"{nm} size={size} date={date:#06x} time={time:#06x}")
    print(repr(blob[:size]))
else:
    for nm, attr, cl, size, date, time in cur:
        kind = 'DIR ' if attr & 0x10 else '    '
        # DESK42: the raw attribute byte too -- the read-only switch of
        # ADFFINFO is checked by reading it back off the card.
        print(f"{kind}{nm:<14} {size:>8}  attr={attr:#04x} date={date:#06x} time={time:#06x}")
