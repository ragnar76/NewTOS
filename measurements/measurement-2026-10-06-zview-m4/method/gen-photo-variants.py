#!/usr/bin/env python3
"""gen-photo-variants.py <photo> <outdir> -- measurement-2026-10-06-zview-m4.

Derives zView test files from a real photograph (the user-supplied photo of
this run is PRIVATE: it, everything this script writes and every screendump
showing it stay in the session scratch directory and never enter the repo;
RUNINFO.TXT records only sha256 sums). Writes <outdir>/IMAGES/P*.* and
<outdir>/expected/<name>.rgb (screen-quantised, the same rules as
sdk/zview/samples/gen-samples.py; the JPEG reference comes from jpgref.c).

  270x360 (3:4, fits the zView window on a 640x480 screen):
    P.JPG (quality 90), P.PNG, P.BMP (24-bit), P.TGA (24-bit RLE),
    P.GIF (adaptive 256-colour palette), P.GOD (RGB565),
    P.RAW (Eureka, 256-colour palette), P8.IMG (XIMG, 8 bit planes,
    256-colour RGB1000 palette -- the pp_8_to_24 path of xp32)
  320x200, 16 colours (the photo scaled to 150x200, centred on black):
    P.PI1 (DEGAS), P.NEO (NEOchrome) with an STe palette
"""
import os
import struct
import sys

from PIL import Image, ImageOps

SRC, OUT = sys.argv[1], sys.argv[2]
IMG = os.path.join(OUT, "IMAGES")
EXP = os.path.join(OUT, "expected")
os.makedirs(IMG, exist_ok=True)
os.makedirs(EXP, exist_ok=True)


def disp(c):
    q = c >> 3
    return (q << 3) | (q >> 2)


def expect(name, w, h, rows):
    with open(os.path.join(EXP, name + ".rgb"), "wb") as f:
        f.write(struct.pack("<HH", w, h))
        for row in rows:
            for (r, g, b) in row:
                f.write(bytes((disp(r), disp(g), disp(b))))


def put(name, data):
    with open(os.path.join(IMG, name), "wb") as f:
        f.write(data)


photo = ImageOps.exif_transpose(Image.open(SRC)).convert("RGB")
W, H = 270, 360
im = photo.resize((W, H), Image.LANCZOS)
px = list(im.getdata())
RGB = [px[y * W:(y + 1) * W] for y in range(H)]

im.save(os.path.join(IMG, "P.JPG"), quality=90)
im.save(os.path.join(IMG, "P.PNG"))
expect("P.PNG", W, H, RGB)

# BMP 24
pad = (4 - (W * 3) % 4) & 3
pix = b"".join(b"".join(bytes((b, g, r)) for (r, g, b) in RGB[y]) + b"\0" * pad for y in range(H - 1, -1, -1))
put("P.BMP", struct.pack("<2sIHHI", b"BM", 54 + len(pix), 0, 0, 54)
    + struct.pack("<IiiHHIIiiII", 40, W, H, 1, 24, 0, len(pix), 2835, 2835, 0, 0) + pix)
expect("P.BMP", W, H, RGB)

# TGA 24 RLE, top-left origin, packets per line
out = bytearray(struct.pack("<BBBHHBHHHHBB", 0, 0, 10, 0, 0, 0, 0, 0, W, H, 24, 0x20))
for y in range(H):
    p = [bytes((b, g, r)) for (r, g, b) in RGB[y]]
    i = 0
    while i < W:
        j = i
        while j + 1 < W and p[j + 1] == p[i] and j - i < 127:
            j += 1
        if j > i:
            out.append(0x80 | (j - i)); out += p[i]; i = j + 1
        else:
            k = i
            while k + 1 < W and p[k + 1] != p[k] and k - i < 127:
                k += 1
            out.append(k - i); out += b"".join(p[i:k + 1]); i = k + 1
put("P.TGA", bytes(out))
expect("P.TGA", W, H, RGB)

# 256-colour palette version (GIF, RAW, IMG)
pim = im.quantize(colors=256, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
pal = pim.getpalette()[:768]
pal += [0] * (768 - len(pal))
idx = list(pim.getdata())
IDX = [idx[y * W:(y + 1) * W] for y in range(H)]
PRGB = [[tuple(pal[3 * i:3 * i + 3]) for i in row] for row in IDX]
pim.save(os.path.join(IMG, "P.GIF"))
expect("P.GIF", W, H, PRGB)

raw = bytearray(struct.pack(">HH", W, H))
for i in range(768):
    r, g, b = pal[3 * i:3 * i + 3] if i < 256 else (0, 0, 0)
    raw += struct.pack(">I", (r << 24) | (g << 16) | (b << 8))
for row in IDX:
    raw += bytes(reversed(row))
put("P.RAW", bytes(raw))
expect("P.RAW", W, H, PRGB)

# XIMG 8 planes: RGB1000 palette (v*1000+127)//255, codec converts back
R1000 = [tuple((pal[3 * i + c] * 1000 + 127) // 255 for c in range(3)) for i in range(256)]
c1000 = lambda v: ((v << 8) - v + 500) // 1000
lb = (W + 7) >> 3
ximg = bytearray(struct.pack(">8h", 1, 8 + 3 + 3 * 256, 8, 2, 85, 85, W, H))
ximg += b"XIMG" + struct.pack(">h", 0)
for t in R1000:
    ximg += struct.pack(">3h", *t)
for row in IDX:
    for p in range(8):
        bits = bytearray()
        for g in range(0, lb * 8, 8):
            v = 0
            for k in range(8):
                x = g + k
                v = (v << 1) | (((row[x] >> p) & 1) if x < W else 0)
            bits.append(v)
        ximg += bytes((0x80, lb)) + bits
put("P8.IMG", bytes(ximg))
expect("P8.IMG", W, H, [[tuple(c1000(v) for v in R1000[i]) for i in row] for row in IDX])

# GodPaint RGB565
god = bytearray(b"G4" + struct.pack(">HH", W, H))
GOD = []
for row in RGB:
    e = []
    for (r, g, b) in row:
        v = ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3)
        god += struct.pack(">H", v)
        e.append((((v >> 11) & 0x1F) << 3, ((v >> 5) & 0x3F) << 2, (v & 0x1F) << 3))
    GOD.append(e)
put("P.GOD", bytes(god))
expect("P.GOD", W, H, GOD)

# 320x200 16 colours, STe palette (4 bits per gun, LSB in nibble bit 3)
st = Image.new("RGB", (320, 200))
st.paste(photo.resize((150, 200), Image.LANCZOS), (85, 0))
q16 = st.quantize(colors=16, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
p16 = (q16.getpalette()[:48] + [0] * 48)[:48]


def ste_nib(v):
    v4 = v >> 4
    return ((v4 & 1) << 3) | (v4 >> 1)


STPAL = [(ste_nib(p16[3 * i]) << 8) | (ste_nib(p16[3 * i + 1]) << 4) | ste_nib(p16[3 * i + 2]) for i in range(16)]


def st_rgb(p):
    r = (((p >> 7) & 0xE) + ((p >> 11) & 0x1)) * 0x11
    g = (((p >> 3) & 0xE) + ((p >> 7) & 0x1)) * 0x11
    b = (((p << 1) & 0xE) + ((p >> 3) & 0x1)) * 0x11
    return (r & 0xFF, g & 0xFF, b & 0xFF)


si = list(q16.getdata())
SIDX = [si[y * 320:(y + 1) * 320] for y in range(200)]
body = bytearray()
for row in SIDX:
    for g in range(0, 320, 16):
        for p in range(4):
            w = 0
            for k in range(16):
                w = (w << 1) | ((row[g + k] >> p) & 1)
            body += struct.pack(">H", w)
pal_be = b"".join(struct.pack(">H", p) for p in STPAL)
put("P.PI1", struct.pack(">H", 0) + pal_be + bytes(body))
put("P.NEO", struct.pack(">hh", 0, 0) + pal_be + b"PHOTO   .NEO" + struct.pack(">7h", 0, 0, 0, 0, 0, 320, 200) + b"\0" * 66 + bytes(body))
ST = [[st_rgb(STPAL[i]) for i in row] for row in SIDX]
expect("P.PI1", 320, 200, ST)
expect("P.NEO", 320, 200, ST)

for n in sorted(os.listdir(IMG)):
    print("%-8s %7d bytes" % (n, os.path.getsize(os.path.join(IMG, n))))
