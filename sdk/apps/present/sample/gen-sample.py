#!/usr/bin/env python3
"""gen-sample.py OUTDIR -- write TEST.PNG, the PRESENT01 test picture.

Deterministic, standard library only (own PNG encoder over zlib): 800x600
RGBA8, a colour gradient, an orange disc, a white grid of 40 px cells (one
pixel lines: scaling artefacts show), the left half of the top band fully
transparent (alpha 0 -> must show the slide background) and a half-
transparent bar at the bottom (alpha 128 -> must be blended).
"""
import os, struct, sys, zlib

W, H = 800, 600

def chunk(t, d):
    c = struct.pack('>I', len(d)) + t + d
    return c + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)

def pixel(x, y):
    r = 40 + x * 180 // W
    g = 60 + y * 150 // H
    b = 200 - x * 120 // W
    a = 255
    dx, dy = x - 560, y - 300
    if dx * dx + dy * dy < 150 * 150:
        r, g, b = 240, 128, 48
    if x % 40 == 0 or y % 40 == 0:
        r, g, b = 255, 255, 255
    if y < 60 and x < W // 2:
        a = 0
    if y >= 520:
        a = 128
    return bytes((r, g, b, a))

raw = bytearray()
for y in range(H):
    raw.append(0)
    for x in range(W):
        raw += pixel(x, y)
png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', W, H, 8, 6, 0, 0, 0)) \
    + chunk(b'IDAT', zlib.compress(bytes(raw), 9)) + chunk(b'IEND', b'')
out = os.path.join(sys.argv[1] if len(sys.argv) > 1 else '.', 'TEST.PNG')
open(out, 'wb').write(png)
print(out, len(png), 'bytes')
