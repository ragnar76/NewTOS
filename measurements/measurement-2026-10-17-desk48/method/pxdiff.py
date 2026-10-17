#!/usr/bin/env python3
"""pxdiff.py A.png B.png -- count differing pixels (RGB) and their bounding box. Pure stdlib PNG decoder (8-bit RGB/RGBA)."""
import zlib, struct, sys
def load(p):
    d = open(p, 'rb').read(); i = 8; idat = b''
    while i < len(d):
        n, t = struct.unpack('>I4s', d[i:i+8]); c = d[i+8:i+8+n]
        if t == b'IHDR': w, h, bd, ct = struct.unpack('>IIBB', c[:10])
        if t == b'IDAT': idat += c
        i += 12 + n
    raw = zlib.decompress(idat); bpp = 3 if ct == 2 else 4; st = w * bpp; out = []; prev = bytearray(st); p = 0
    for y in range(h):
        f = raw[p]; line = bytearray(raw[p+1:p+1+st]); p += 1 + st
        for x in range(st):
            a = line[x-bpp] if x >= bpp else 0; b = prev[x]; c = prev[x-bpp] if x >= bpp else 0
            if f == 1: line[x] = (line[x] + a) & 255
            elif f == 2: line[x] = (line[x] + b) & 255
            elif f == 3: line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                pa = abs(b - c); pb = abs(a - c); pc = abs(a + b - 2 * c)
                pr = a if pa <= pb and pa <= pc else (b if pb <= pc else c); line[x] = (line[x] + pr) & 255
        out.append(bytes(line)); prev = line
    return w, h, bpp, out
w, h, bpp, A = load(sys.argv[1]); _, _, bpq, B = load(sys.argv[2])
diff = [(x, y) for y in range(h) for x in range(w) if A[y][x*bpp:x*bpp+3] != B[y][x*bpq:x*bpq+3]]
if diff:
    xs = [d[0] for d in diff]; ys = [d[1] for d in diff]
    print("%s vs %s: %d px differ, bbox (%d,%d)-(%d,%d)" % (sys.argv[1].split('/')[-1], sys.argv[2].split('/')[-1], len(diff), min(xs), min(ys), max(xs), max(ys)))
else:
    print("%s vs %s: identical" % (sys.argv[1].split('/')[-1], sys.argv[2].split('/')[-1]))
