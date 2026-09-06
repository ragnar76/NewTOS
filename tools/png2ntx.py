#!/usr/bin/env python3
"""png2ntx — turn artwork into ``BALL.NTX``, the bouncing-ball demo's texture.

The demo (``sdk/apps/ball/``) wraps an EQUIRECTANGULAR texture around its
sphere: the horizontal axis is 360 degrees of longitude, the vertical axis 180
degrees of latitude, the top row is the north pole. Feed this tool a PNG drawn
that way and it writes the ``NTX1`` file the demo reads from ``C:\\BALL.NTX``.

Why a converter at all
----------------------
Everything expensive happens HERE, once, on a fast machine: resampling and,
above all, quantisation. The demo is meant to run on an Atari TT with a NOVA
card in 640x480x256 as well as on a Raspberry Pi, and the TT must not spend a
second of its start-up reducing colours the host could have reduced. So the
file carries 8-bit indices AND the palette they belong to, and both platforms
load it exactly as it is.

The palette split — this is a contract, not a preference
--------------------------------------------------------
The demo has 256 screen colours for the whole picture and the texture is only
one tenant::

      0 ..  23   backdrop
     24 ..  95   floor and its shadow
     96 .. 127   RESERVED for milestone 2b (grid room, floor reflection, HUD)
    128 .. 255   the ball

The texture may not spend anything outside its own slice, and the format makes
that structural rather than a rule to remember: a texel is not a screen colour,
it is a HUE — one of at most four surface colours the ball is made of. The demo
gives each hue a ramp of shades inside 128..255 and lights a pixel with a table
lookup, ``shade_lut[level][hue]``. Four hues is therefore the hard ceiling, and
it is why this tool quantises to four colours and not to sixteen.

The NTX1 file
-------------
All multi-byte fields are BIG-endian, so the same file is valid for the
AArch64 build and for a future big-endian m68k one::

    offset  size   field
    0       4      magic, "NTX1"
    4       2      width in texels   -- a power of two, <= 256
    6       2      height in texels  -- a power of two, <= 128
    8       1      hue count, 1 .. 4
    9       1      flags, 0
    10      2      reserved, 0
    12      3*n    hue colours, r, g, b each
    12+3n   w*h    one byte per texel: the hue index, < hue count

    constraint: 2 * width * height <= 65536

Usage::

    tools/png2ntx.py ball.png BALL.NTX
    tools/png2ntx.py ball.png BALL.NTX --dither --preview out.ppm
    tools/png2ntx.py --info BALL.NTX

Only the standard library is used, so there is nothing to install: the PNG is
decoded here with ``zlib``.
"""
import argparse
import struct
import sys
import zlib

MAGIC = b"NTX1"

# The demo's own four hues. They are the generator's colours in
# sdk/apps/ball/ball-tex.c, so artwork converted with the defaults sits in the
# same palette as the generated texture it replaces.
DEFAULT_HUES = [
    (236, 238, 240),   # white -- the ball's ground
    (206, 38, 32),     # red   -- the equatorial band
    (22, 22, 26),      # black -- lettering and pinstripes
    (58, 150, 58),     # green -- the berry's leaves
]

MAX_W, MAX_H, MAX_HUES = 256, 128, 4


# --------------------------------------------------------------------------
# PNG decoding: 8-bit, non-interlaced, all five colour types
# --------------------------------------------------------------------------
def _paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def read_png(path):
    """Return (width, height, rows) with rows as lists of (r, g, b)."""
    data = open(path, "rb").read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path}: not a PNG")
    pos, idat, plte, trns = 8, bytearray(), None, None
    w = h = depth = ctype = interlace = None
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        kind = data[pos + 4:pos + 8]
        body = data[pos + 8:pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            w, h, depth, ctype, _, _, interlace = struct.unpack(">IIBBBBB", body)
        elif kind == b"PLTE":
            plte = body
        elif kind == b"tRNS":
            trns = body
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
    if depth != 8:
        raise SystemExit(f"{path}: only 8 bits per channel supported "
                         f"(this one has {depth})")
    if interlace:
        raise SystemExit(f"{path}: interlaced PNGs are not supported")
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}.get(ctype)
    if channels is None:
        raise SystemExit(f"{path}: unknown colour type {ctype}")
    if ctype == 3 and plte is None:
        raise SystemExit(f"{path}: palette image without a PLTE chunk")

    raw = zlib.decompress(bytes(idat))
    stride = w * channels
    out, prev, p = [], bytearray(stride), 0
    for _ in range(h):
        f = raw[p]
        line = bytearray(raw[p + 1:p + 1 + stride])
        p += 1 + stride
        if f == 1:
            for i in range(channels, stride):
                line[i] = (line[i] + line[i - channels]) & 0xFF
        elif f == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 0xFF
        elif f == 3:
            for i in range(stride):
                a = line[i - channels] if i >= channels else 0
                line[i] = (line[i] + ((a + prev[i]) >> 1)) & 0xFF
        elif f == 4:
            for i in range(stride):
                a = line[i - channels] if i >= channels else 0
                c = prev[i - channels] if i >= channels else 0
                line[i] = (line[i] + _paeth(a, prev[i], c)) & 0xFF
        elif f != 0:
            raise SystemExit(f"{path}: bad row filter {f}")
        prev = line

        row = []
        if ctype == 0:
            row = [(v, v, v) for v in line]
        elif ctype == 2:
            row = [tuple(line[i:i + 3]) for i in range(0, stride, 3)]
        elif ctype == 3:
            for v in line:
                row.append(tuple(plte[v * 3:v * 3 + 3]))
        elif ctype == 4:
            row = [(line[i], line[i], line[i]) for i in range(0, stride, 2)]
        else:
            row = [tuple(line[i:i + 3]) for i in range(0, stride, 4)]
        out.append(row)
    _ = trns
    return w, h, out


# --------------------------------------------------------------------------
# Resample and quantise
# --------------------------------------------------------------------------
def box_resample(src, sw, sh, dw, dh):
    """Average each destination texel over the source pixels it covers."""
    dst = []
    for dy in range(dh):
        y0, y1 = dy * sh // dh, max(dy * sh // dh + 1, (dy + 1) * sh // dh)
        row = []
        for dx in range(dw):
            x0, x1 = dx * sw // dw, max(dx * sw // dw + 1, (dx + 1) * sw // dw)
            r = g = b = n = 0
            for y in range(y0, y1):
                s = src[y]
                for x in range(x0, x1):
                    px = s[x]
                    r += px[0]
                    g += px[1]
                    b += px[2]
                    n += 1
            row.append((r / n, g / n, b / n))
        dst.append(row)
    return dst


def nearest(hues, c):
    """Index of the closest hue. Luma-weighted, because a sphere's shading
    already carries the brightness and what must survive is the HUE."""
    best, bi = None, 0
    for i, h in enumerate(hues):
        dr, dg, db = c[0] - h[0], c[1] - h[1], c[2] - h[2]
        d = 2.0 * dr * dr + 4.0 * dg * dg + 3.0 * db * db
        if best is None or d < best:
            best, bi = d, i
    return bi


def quantise(img, w, h, hues, dither):
    """One byte per texel. Floyd-Steinberg optionally, which helps photographic
    artwork and hurts flat logo artwork -- hence the flag."""
    px = bytearray(w * h)
    buf = [list(r) for r in img]
    for y in range(h):
        for x in range(w):
            c = buf[y][x]
            i = nearest(hues, c)
            px[y * w + x] = i
            if not dither:
                continue
            e = (c[0] - hues[i][0], c[1] - hues[i][1], c[2] - hues[i][2])
            for dx, dy, f in ((1, 0, 7 / 16), (-1, 1, 3 / 16),
                              (0, 1, 5 / 16), (1, 1, 1 / 16)):
                nx, ny = x + dx, y + dy
                if 0 <= nx < w and 0 <= ny < h:
                    o = buf[ny][nx]
                    buf[ny][nx] = (o[0] + e[0] * f, o[1] + e[1] * f,
                                   o[2] + e[2] * f)
    return px


# --------------------------------------------------------------------------
# The file
# --------------------------------------------------------------------------
def write_ntx(path, w, h, hues, px):
    with open(path, "wb") as f:
        f.write(MAGIC)
        f.write(struct.pack(">HHBBH", w, h, len(hues), 0, 0))
        for r, g, b in hues:
            f.write(bytes((r, g, b)))
        f.write(bytes(px))


def read_ntx(path):
    d = open(path, "rb").read()
    if d[:4] != MAGIC:
        raise SystemExit(f"{path}: not an NTX1 file")
    w, h, n, flags, rsvd = struct.unpack(">HHBBH", d[4:12])
    hues = [tuple(d[12 + 3 * i:15 + 3 * i]) for i in range(n)]
    px = d[12 + 3 * n:]
    return w, h, hues, flags, rsvd, px


def write_preview(path, w, h, hues, px):
    with open(path, "wb") as f:
        f.write(b"P6\n%d %d\n255\n" % (w, h))
        f.write(b"".join(bytes(hues[b]) for b in px))


def parse_hues(text):
    out = []
    for tok in text.split(","):
        tok = tok.strip().lstrip("#")
        if len(tok) != 6:
            raise SystemExit(f"--colors: '{tok}' is not RRGGBB")
        out.append(tuple(int(tok[i:i + 2], 16) for i in (0, 2, 4)))
    if not 1 <= len(out) <= MAX_HUES:
        raise SystemExit(f"--colors: 1..{MAX_HUES} colours, got {len(out)}")
    return out


def main(argv):
    ap = argparse.ArgumentParser(
        description="Convert an equirectangular PNG into a BALL.NTX texture.")
    ap.add_argument("input", nargs="?", help="source PNG (or NTX with --info)")
    ap.add_argument("output", nargs="?", help="destination .NTX")
    ap.add_argument("--size", default="256x128",
                    help="texel size, default 256x128; both powers of two, "
                         "2*w*h <= 65536")
    ap.add_argument("--colors", help="hue palette as RRGGBB,RRGGBB,... "
                                     "(1..4); default is the demo's own")
    ap.add_argument("--dither", action="store_true",
                    help="Floyd-Steinberg; good for photographs, bad for flat "
                         "logo artwork")
    ap.add_argument("--preview", help="also write a PPM of the result")
    ap.add_argument("--info", action="store_true", help="describe an NTX file")
    a = ap.parse_args(argv)

    if a.info:
        if not a.input:
            ap.error("--info needs a file")
        w, h, hues, flags, rsvd, px = read_ntx(a.input)
        print(f"{a.input}: {w}x{h}, {len(hues)} hues, flags={flags}, "
              f"reserved={rsvd}, {len(px)} texel bytes "
              f"({'ok' if len(px) == w * h else 'WRONG, expected %d' % (w * h)})")
        for i, c in enumerate(hues):
            print(f"  hue {i}: #{c[0]:02X}{c[1]:02X}{c[2]:02X}  "
                  f"{px.count(i)} texels")
        return 0

    if not a.input or not a.output:
        ap.error("need an input PNG and an output .NTX")

    try:
        dw, dh = (int(v) for v in a.size.lower().split("x"))
    except ValueError:
        ap.error("--size wants WxH, e.g. 256x128")
    for name, v, cap in (("width", dw, MAX_W), ("height", dh, MAX_H)):
        if v <= 0 or v & (v - 1) or v > cap:
            ap.error(f"{name} {v} must be a power of two and at most {cap}")
    if 2 * dw * dh > 65536:
        ap.error(f"2 * {dw} * {dh} exceeds the 65536-byte texel offset limit")

    hues = parse_hues(a.colors) if a.colors else DEFAULT_HUES
    sw, sh, src = read_png(a.input)
    img = box_resample(src, sw, sh, dw, dh)
    px = quantise(img, dw, dh, hues, a.dither)
    write_ntx(a.output, dw, dh, hues, px)
    if a.preview:
        write_preview(a.preview, dw, dh, hues, px)

    print(f"{a.input} {sw}x{sh} -> {a.output} {dw}x{dh}, {len(hues)} hues"
          f"{', dithered' if a.dither else ''}")
    for i, c in enumerate(hues):
        share = 100.0 * px.count(i) / (dw * dh)
        print(f"  hue {i}: #{c[0]:02X}{c[1]:02X}{c[2]:02X}  {share:5.1f} %")
    print("copy it to the SD card as C:\\BALL.NTX")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
