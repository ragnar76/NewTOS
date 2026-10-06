#!/usr/bin/env python3
"""check-shot.py <screendump.png> <expected.rgb> [--raw] [--mask x0,y0,x1,y1]

Finds the expected picture in a QEMU screendump and compares it pixel by
pixel (measurement-2026-10-06-zview-m4).

expected.rgb: <w uint16 LE><h uint16 LE><w*h RGB> as written by
sdk/zview/samples/gen-samples.py (already screen-quantised) or, with --raw,
by method/jpgref.c (decoder output; quantised here the same way:
5 bits per channel kept, expanded (c << 3) | (c >> 2) -- zvdi/raster.c,
tos-api/src/vdi/eddi.rs, kernel/src/vdi_backend.rs expand5to8).

Location: the first expected row is searched as an exact byte string in
every screen row; each hit is verified on the whole picture. --mask excludes
a screen rectangle (x0,y0 inclusive, x1,y1 exclusive) from the comparison --
used for the mouse sprite, which the kernel draws into the framebuffer at
the screen centre and which QMP's screendump therefore contains.

Output: one line "PASS/FAIL <name> at (x,y) WxH: compared N, mismatched M,
masked K" plus up to 10 mismatch samples. Exit 0 on PASS.
"""
import struct
import sys

from PIL import Image


def q(c):
    v = c >> 3
    return (v << 3) | (v >> 2)


args = sys.argv[1:]
raw = "--raw" in args
mask = None
if "--mask" in args:
    mask = tuple(int(v) for v in args[args.index("--mask") + 1].split(","))
png, exp = args[0], args[1]
d = open(exp, "rb").read()
w, h = struct.unpack("<HH", d[:4])
px = d[4:]
if raw:
    px = bytes(q(c) for c in px)
rows = [px[y * w * 3:(y + 1) * w * 3] for y in range(h)]

im = Image.open(png).convert("RGB")
SW, SH = im.size
scr = im.tobytes()
srow = [scr[y * SW * 3:(y + 1) * SW * 3] for y in range(SH)]


def masked(x, y):
    return mask is not None and mask[0] <= x < mask[2] and mask[1] <= y < mask[3]


def compare(ox, oy):
    n = bad = msk = 0
    samples = []
    for y in range(h):
        sr = srow[oy + y]
        er = rows[y]
        for x in range(w):
            if masked(ox + x, oy + y):
                msk += 1
                continue
            n += 1
            s = sr[(ox + x) * 3:(ox + x) * 3 + 3]
            e = er[x * 3:x * 3 + 3]
            if s != e:
                bad += 1
                if len(samples) < 10:
                    samples.append((x, y, tuple(s), tuple(e)))
    return n, bad, msk, samples


best = None
for ry in (0, h - 1):
    needle = rows[ry]
    for sy in range(SH):
        oy = sy - ry
        if oy < 0 or oy + h > SH:
            continue
        start = 0
        while True:
            i = srow[sy].find(needle, start)
            if i < 0:
                break
            if i % 3 == 0:
                ox = i // 3
                if ox + w <= SW:
                    r = compare(ox, oy)
                    if best is None or r[1] < best[1][1]:
                        best = ((ox, oy), r)
            start = i + 1
    if best is not None:
        break

name = exp.split("/")[-1].replace(".raw.rgb", "").replace(".rgb", "")
if best is None:
    print("FAIL %s: picture not found (neither its first nor its last row appears on the screen)" % name)
    sys.exit(1)
(ox, oy), (n, bad, msk, samples) = best
verdict = "PASS" if bad == 0 else "FAIL"
print("%s %s at (%d,%d) %dx%d: compared %d, mismatched %d, masked %d"
      % (verdict, name, ox, oy, w, h, n, bad, msk))
for (x, y, s, e) in samples:
    print("    (%d,%d) screen %s expected %s" % (x, y, s, e))
sys.exit(0 if bad == 0 else 1)
