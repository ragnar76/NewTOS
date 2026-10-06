#!/usr/bin/env python3
"""gen-samples.py -- zView's sample/test images (TASK/ZVIEW-M4), one per
codec path: run by sdk/zview/build-zview.sh into sdk/zview/build/samples/,
installed by tools/make-sd-image.sh as C:\\ZVIEW\\SAMPLES\\ (QEMU image only),
checked against the screen by measurements/measurement-2026-10-06-zview-m4.

Usage: gen-samples.py OUT
Writes OUT/IMAGES/<name> (the files zView opens) and
OUT/expected/<name>.rgb (the picture zView must put on the screen: width,
height as two little-endian uint16, then width*height RGB triples).

No copyrighted material: every picture is synthesised here. The Atari
formats are written from their layouts AS THE CODEC SOURCES READ THEM
(z-tools-04805b7/zview/plugins/<codec>/*.c), big-endian like the m68k
programs that write them:

  DEGAS  PI1/PC1   34-byte header: word 0 = resolution (bits 1-0) +
                   compression flag (bit 15), 16 ST/STe palette words;
                   PI1 = 32000 bytes of interleaved plane words, PC1 =
                   per line, per plane 40 bytes, PackBits (degasldg.c
                   read_line: n<=127 -> n+1 literals, else 257-n repeats)
  NEOchrome        128-byte header (flags, resolution, 16 palette words,
                   ...) + 32000 bytes, file size exactly 32128 (neoldg.c)
  GodPaint GOD     "G4", width, height, then RGB565 words (godldg.c)
  Eureka RAW       width, height (BE words), 3072-byte palette of BE LONGs
                   read as R<<24|G<<16|B<<8 per entry (rawldg.c:117-124),
                   then width*height index bytes; the codec mirrors every
                   row (rawldg.c:153-163), so the file holds rows reversed
  GEM IMG          8-word header, XIMG id + RGB1000 palette for colour,
                   each plane line as a 0x80 "bit string" record
                   (imgldg.c unpack_line2)

Expected screen colour: zView's 16-bit raster code keeps the top 5 bits of
red and blue and the top 6 of green (zvdi/raster.c dither_16/raster_16,
zvdi/color.c cnvpal_high); NewTOS' device word is the Falcon
rrrrrggg ggxbbbbb with bit 5 ignored (tos-api/src/vdi/eddi.rs), so green
keeps its top 5 bits too, and the backend expands 5 -> 8 bits as
(c << 3) | (c >> 2) (kernel/src/vdi_backend.rs expand5to8). The JPEG
reference pixels come from the measurement's method/jpgref.c (the pinned
IJG 9f, decoded with the codec's settings), not from here.
"""
import os
import struct
import sys

from PIL import Image

OUT = sys.argv[1]
IMG = os.path.join(OUT, "IMAGES")
EXP = os.path.join(OUT, "expected")
os.makedirs(IMG, exist_ok=True)
os.makedirs(EXP, exist_ok=True)


def disp(c):
    q = c >> 3
    return (q << 3) | (q >> 2)


def write_expected(name, w, h, rgb_rows, quantise=True):
    with open(os.path.join(EXP, name + ".rgb"), "wb") as f:
        f.write(struct.pack("<HH", w, h))
        for row in rgb_rows:
            for (r, g, b) in row:
                if quantise:
                    f.write(bytes((disp(r), disp(g), disp(b))))
                else:
                    f.write(bytes((r, g, b)))


def put(name, data):
    with open(os.path.join(IMG, name), "wb") as f:
        f.write(data)


# ---------------------------------------------------------------- truecolour
W, H = 64, 48
BARS = [(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 0),
        (0, 255, 255), (255, 0, 255), (128, 64, 32), (32, 160, 224)]


def tc_pixel(x, y):
    if y < 24:
        return BARS[x // 8]
    if x < 32:
        v = x * 8
        return (v, v, 255 - v) if y < 36 else (v, v, v)
    return (255, 255, 255) if ((x // 4) + (y // 4)) % 2 == 0 else (0, 0, 0)


TC = [[tc_pixel(x, y) for x in range(W)] for y in range(H)]

# BMP 24-bit (BITMAPINFOHEADER, bottom-up, BGR, rows padded to 4 bytes)
def bmp24(rows):
    w, h = len(rows[0]), len(rows)
    pad = (4 - (w * 3) % 4) & 3
    pix = b"".join(b"".join(bytes((b, g, r)) for (r, g, b) in rows[y]) + b"\0" * pad
                   for y in range(h - 1, -1, -1))
    hdr = struct.pack("<2sIHHI", b"BM", 54 + len(pix), 0, 0, 54)
    info = struct.pack("<IiiHHIIiiII", 40, w, h, 1, 24, 0, len(pix), 2835, 2835, 0, 0)
    return hdr + info + pix


put("T24.BMP", bmp24(TC))
write_expected("T24.BMP", W, H, TC)

# BMP 8-bit: palette of the distinct colours, 256 entries written (the codec
# reads 1 << 8 entries, bmpldg.c:181-188), data right after the palette
pal = []
for row in TC:
    for c in row:
        if c not in pal:
            pal.append(c)
assert len(pal) <= 256
pad = (4 - W % 4) & 3
pix8 = b"".join(bytes(pal.index(c) for c in TC[y]) + b"\0" * pad for y in range(H - 1, -1, -1))
paltab = b"".join(bytes((b, g, r, 0)) for (r, g, b) in pal) + b"\0" * (4 * (256 - len(pal)))
put("T8.BMP", struct.pack("<2sIHHI", b"BM", 54 + 1024 + len(pix8), 0, 0, 54 + 1024)
    + struct.pack("<IiiHHIIiiII", 40, W, H, 1, 8, 0, len(pix8), 2835, 2835, 256, 0)
    + paltab + pix8)
write_expected("T8.BMP", W, H, TC)

# TGA: type 2 (uncompressed, bottom-left origin) and type 10 (RLE, top-left)
def tga(rows, rle, top_left):
    w, h = len(rows[0]), len(rows)
    hdr = struct.pack("<BBBHHBHHHHBB", 0, 0, 10 if rle else 2, 0, 0, 0, 0, 0, w, h, 24,
                      0x20 if top_left else 0)
    order = range(h) if top_left else range(h - 1, -1, -1)
    out = bytearray()
    for y in order:
        px = [bytes((b, g, r)) for (r, g, b) in rows[y]]
        if not rle:
            out += b"".join(px)
            continue
        i = 0
        while i < w:              # per line: runs >= 2 as RLE packets, else raw
            j = i
            while j + 1 < w and px[j + 1] == px[i] and j - i < 127:
                j += 1
            if j > i:
                out.append(0x80 | (j - i)); out += px[i]; i = j + 1
            else:
                k = i
                while k + 1 < w and px[k + 1] != px[k] and k - i < 127:
                    k += 1
                out.append(k - i); out += b"".join(px[i:k + 1]); i = k + 1
    return hdr + bytes(out)


put("T24.TGA", tga(TC, False, False))
write_expected("T24.TGA", W, H, TC)
put("TRLE.TGA", tga(TC, True, True))
write_expected("TRLE.TGA", W, H, TC)

# PNG (RGB) and PNG (palette), GIF (palette) via Pillow -- lossless
im = Image.new("RGB", (W, H))
im.putdata([c for row in TC for c in row])
im.save(os.path.join(IMG, "TRGB.PNG"), optimize=False)
write_expected("TRGB.PNG", W, H, TC)
imp = Image.new("P", (W, H))
imp.putpalette([v for c in pal for v in c] + [0] * (3 * (256 - len(pal))))
imp.putdata([pal.index(c) for row in TC for c in row])
imp.save(os.path.join(IMG, "TPAL.PNG"))
write_expected("TPAL.PNG", W, H, TC)
imp.save(os.path.join(IMG, "T.GIF"))
write_expected("T.GIF", W, H, TC)
# JPEG: baseline, quality 95, Pillow's default 4:2:0; expected from jpgref.c
im.save(os.path.join(IMG, "T.JPG"), quality=95)

# GodPaint: RGB565 big-endian; the codec expands r5<<3, g6<<2, b5<<3
god = bytearray(b"G4" + struct.pack(">HH", W, H))
GOD = []
for row in TC:
    erow = []
    for (r, g, b) in row:
        v = ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3)
        god += struct.pack(">H", v)
        erow.append((((v >> 11) & 0x1F) << 3, ((v >> 5) & 0x3F) << 2, (v & 0x1F) << 3))
    GOD.append(erow)
put("T.GOD", bytes(god))
write_expected("T.GOD", W, H, GOD)

# Eureka RAW with palette (indexed path, rawldg.c:113-125)
raw = bytearray(struct.pack(">HH", W, H))
paltab = bytearray()
for i in range(768):
    r, g, b = pal[i] if i < len(pal) else (0, 0, 0)
    paltab += struct.pack(">I", (r << 24) | (g << 16) | (b << 8))
raw += paltab
for row in TC:
    raw += bytes(pal.index(c) for c in reversed(row))      # codec mirrors rows
put("T.RAW", bytes(raw))
write_expected("T.RAW", W, H, TC)

# ---------------------------------------------------------- 16-colour Atari
# Sixteen ST/STe palette words (nibble = STe LSB in bit 3, ST bits 2-0)
STPAL = [0x0FFF, 0x0F00, 0x00F0, 0x000F, 0x0FF0, 0x00FF, 0x0F0F, 0x0000,
         0x0888, 0x0444, 0x0C63, 0x036C, 0x09A5, 0x05A9, 0x0E17, 0x0B2D]


def st_rgb(p):
    # degasldg.c:166-168 / neoldg.c (same formula)
    r = (((p >> 7) & 0xE) + ((p >> 11) & 0x1)) * 0x11
    g = (((p >> 3) & 0xE) + ((p >> 7) & 0x1)) * 0x11
    b = (((p << 1) & 0xE) + ((p >> 3) & 0x1)) * 0x11
    return (r & 0xFF, g & 0xFF, b & 0xFF)


SW, SH = 320, 200


def st_index(x, y):
    if y < 100:
        return x // 20
    return ((x // 8) + (y // 8)) % 16


STIDX = [[st_index(x, y) for x in range(SW)] for y in range(SH)]
STRGB = [[st_rgb(STPAL[i]) for i in row] for row in STIDX]


def plane_words(row, nplanes):
    """interleaved ST low-res words for one line: for each 16-pixel group,
    one word per plane"""
    out = bytearray()
    for g in range(0, len(row), 16):
        for p in range(nplanes):
            w = 0
            for k in range(16):
                w = (w << 1) | ((row[g + k] >> p) & 1)
            out += struct.pack(">H", w)
    return bytes(out)


def plane_lines(row, nplanes):
    """per plane, all of the line's words (DEGAS compressed layout)"""
    out = bytearray()
    for p in range(nplanes):
        for g in range(0, len(row), 16):
            w = 0
            for k in range(16):
                w = (w << 1) | ((row[g + k] >> p) & 1)
            out += struct.pack(">H", w)
    return bytes(out)


def packbits(data):
    out = bytearray(); i = 0
    while i < len(data):
        j = i
        while j + 1 < len(data) and data[j + 1] == data[i] and j - i < 127:
            j += 1
        if j - i >= 1:
            out += bytes((257 - (j - i + 1), data[i])); i = j + 1
        else:
            k = i
            while k + 1 < len(data) and data[k + 1] != data[k] and k - i < 127:
                k += 1
            out.append(k - i); out += data[i:k + 1]; i = k + 1
    return bytes(out)


pal_be = b"".join(struct.pack(">H", p) for p in STPAL)
body = b"".join(plane_words(r, 4) for r in STIDX)
assert len(body) == 32000
put("T.PI1", struct.pack(">H", 0) + pal_be + body)
write_expected("T.PI1", SW, SH, STRGB)
put("T.PC1", struct.pack(">H", 0x8000) + pal_be
    + b"".join(packbits(plane_lines(r, 4)) for r in STIDX))
write_expected("T.PC1", SW, SH, STRGB)
neo = struct.pack(">hh", 0, 0) + pal_be + b"TEST    .NEO" + struct.pack(">7h", 0, 0, 0, 0, 0, SW, SH) + b"\0" * 66
assert len(neo) == 128
put("T.NEO", neo + body)
write_expected("T.NEO", SW, SH, STRGB)

# GEM IMG, XIMG 4 planes, 64x48, palette RGB1000; and mono 1 plane
IW, IH = 64, 48
IMGIDX = [[(x // 4) if y < 24 else (((x // 8) + (y // 8)) % 16) for x in range(IW)] for y in range(IH)]
R1000 = [(1000, 1000, 1000), (1000, 0, 0), (0, 1000, 0), (0, 0, 1000), (1000, 1000, 0),
         (0, 1000, 1000), (1000, 0, 1000), (0, 0, 0), (500, 500, 500), (250, 250, 250),
         (800, 400, 200), (200, 400, 800), (600, 650, 300), (300, 650, 600), (900, 100, 450),
         (700, 150, 850)]


def c1000(v):
    # imgldg.c:150-155: (((i << 8) - i) + 500) / 1000
    return ((v << 8) - v + 500) // 1000


def img_file(idx, nplanes, palette):
    w, h = len(idx[0]), len(idx)
    line_bytes = (w + 7) >> 3
    hdr_words = 8 + (3 + 3 * len(palette) if palette else 0)
    out = bytearray(struct.pack(">8h", 1, hdr_words, nplanes, 2, 85, 85, w, h))
    if palette:
        out += b"XIMG" + struct.pack(">h", 0)
        for (r, g, b) in palette:
            out += struct.pack(">3h", r, g, b)
    for row in idx:
        for p in range(nplanes):
            bits = bytearray()
            for g in range(0, w, 8):
                v = 0
                for k in range(8):
                    v = (v << 1) | ((row[g + k] >> p) & 1)
                bits.append(v)
            out += bytes((0x80, line_bytes)) + bits
    return bytes(out)


put("T4.IMG", img_file(IMGIDX, 4, R1000))
write_expected("T4.IMG", IW, IH, [[tuple(c1000(v) for v in R1000[i]) for i in row] for row in IMGIDX])
MONO = [[1 if ((x // 4) + (y // 4)) % 2 == 0 or (y < 8 and x < 32) else 0 for x in range(IW)] for y in range(IH)]
put("T1.IMG", img_file(MONO, 1, None))
# set bit = black (GEM IMG convention), clear = white; mono is not quantised
write_expected("T1.IMG", IW, IH, [[(0, 0, 0) if v else (255, 255, 255) for v in row] for row in MONO], False)

for n in sorted(os.listdir(IMG)):
    print("%-9s %7d bytes" % (n, os.path.getsize(os.path.join(IMG, n))))
