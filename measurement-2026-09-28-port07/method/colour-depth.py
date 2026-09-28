#!/usr/bin/env python3
"""How many distinct colours a screendump holds, and whether every one of
them is representable in the VDI's five bits per gun.

usage: colour-depth.py PNG [PNG ...]

The NewTOS framebuffer is 32-bpp XRGB8888 and holds eight bits per gun; a
NewTOS VDI *device word* holds five (the Falcon truecolour pixel), which the
backend expands back to eight as (c5 << 3) | (c5 >> 2). So a picture that
reached the screen through the VDI has only colours of that shape, and a
picture that was stored into the framebuffer directly need not have. That is
the whole test.
"""
import sys, zlib, struct


def readpng(p):
    d = open(p, 'rb').read()
    assert d[:8] == b'\x89PNG\r\n\x1a\n', p
    i, idat = 8, b''
    w = h = bd = ct = None
    while i < len(d):
        ln = struct.unpack('>I', d[i:i + 4])[0]
        typ, data = d[i + 4:i + 8], d[i + 8:i + 8 + ln]
        i += 12 + ln
        if typ == b'IHDR':
            w, h, bd, ct = struct.unpack('>IIBB', data[:10])
        elif typ == b'IDAT':
            idat += data
        elif typ == b'IEND':
            break
    assert bd == 8
    raw = zlib.decompress(idat)
    nch = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ct]
    stride = w * nch
    out, prev, o = bytearray(), bytearray(stride), 0
    for _ in range(h):
        f = raw[o]; o += 1
        line = bytearray(raw[o:o + stride]); o += stride
        for x in range(stride):
            a = line[x - nch] if x >= nch else 0
            b = prev[x]
            c = prev[x - nch] if x >= nch else 0
            if f == 1: line[x] = (line[x] + a) & 255
            elif f == 2: line[x] = (line[x] + b) & 255
            elif f == 3: line[x] = (line[x] + ((a + b) >> 1)) & 255
            elif f == 4:
                pp = a + b - c
                pa, pb, pc = abs(pp - a), abs(pp - b), abs(pp - c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[x] = (line[x] + pr) & 255
        out += line
        prev = line
    return w, h, nch, bytes(out)


def q5(v):
    c = v >> 3
    return v == ((c << 3) | (c >> 2))


for path in sys.argv[1:]:
    w, h, n, px = readpng(path)
    cols = set()
    for y in range(h):
        r = y * w * n
        for x in range(w):
            o = r + x * n
            cols.add((px[o], px[o + 1], px[o + 2]))
    bad = [c for c in cols if not (q5(c[0]) and q5(c[1]) and q5(c[2]))]
    print("%-44s %dx%d  %3d distinct  %3d NOT 5-bit/gun"
          % (path.split('/')[-1], w, h, len(cols), len(bad)))
