#!/usr/bin/env python3
"""gen-data.py -- generate the embedded test data of LIBTEST.NCP and its
independent (Python-side) reference values (TASK/LDG-M2).

Writes sdk/examples/libtest/libtest-data.h:
  - every test file as a C byte array (libtest.c writes them to disk and
    decodes them through the libraries' FILE/path APIs, as zView's codecs do);
  - PY_REFS: CRC-32 + length of what a correct decoder MUST produce, computed
    here WITHOUT the libraries under test:
      zlib   CRC-32/Adler-32 of a buffer libtest.c generates with the same LCG,
             and the inflated output of a stream made by Python's zlib;
      PNG    the RGBA8 pixels the files were encoded FROM (our own little PNG
             encoder below, every filter type 0-4 per row, Adam7 too), mapped
             the way png_set_expand/strip_16/gray_to_rgb/add_alpha define it;
      GIF    the colour-index raster and the global colour table, read back
             by Pillow (an independent decoder) resp. parsed from the bytes;
      EXIF   the thumbnail bytes we embedded ourselves.
    JPEG pixels have no Python reference: Pillow decodes with libjpeg-turbo,
    whose chroma upsampling differs from IJG 9's DCT-domain scaling. Those
    values come from the same IJG 9f sources built for the HOST
    (make-ref.sh -> libtest-ref.h). The Pillow decode is compared only for
    information (max abs difference printed here).

Deterministic: no timestamps, fixed seeds. Run: python3 gen-data.py
(Pillow required; tested with Pillow 12.3.0).
"""
import io
import os
import struct
import zlib

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))


def crc(b):
    return zlib.crc32(bytes(b)) & 0xFFFFFFFF


# ---------------------------------------------------------------- zlib -----
def lcg_text(n, seed):
    """Must match libtest.c lcg_text()."""
    alpha = b"NewTOS zlib round trip ABCDEFGH\n"   # 32 symbols
    x = seed
    out = bytearray(n)
    for i in range(n):
        x = (x * 1103515245 + 12345) & 0xFFFFFFFF
        out[i] = alpha[(x >> 16) & 31]
    return bytes(out)


ZBUF_LEN = 70000 + 123
ZBUF_SEED = 0x2A2A1234


def adler32(b):
    return zlib.adler32(b) & 0xFFFFFFFF


# ----------------------------------------------------------------- PNG -----
def png_chunk(t, d):
    return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)


def paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    if pb <= pc:
        return b
    return c


def filter_row(ft, row, prev, bpp):
    out = bytearray([ft])
    for i, x in enumerate(row):
        a = row[i - bpp] if i >= bpp else 0
        b = prev[i] if prev is not None else 0
        c = prev[i - bpp] if (prev is not None and i >= bpp) else 0
        if ft == 0:
            v = x
        elif ft == 1:
            v = x - a
        elif ft == 2:
            v = x - b
        elif ft == 3:
            v = x - ((a + b) >> 1)
        else:
            v = x - paeth(a, b, c)
        out.append(v & 0xFF)
    return out


def pack_row(pix, depth, chans):
    """pix: list of sample tuples -> packed bytes (big-endian 16-bit)."""
    if depth == 16:
        return b"".join(struct.pack(">H", s) for p in pix for s in p)
    if depth == 8:
        return bytes(s for p in pix for s in p)
    out = bytearray()
    acc, nbits = 0, 0
    for p in pix:
        for s in p:
            acc = (acc << depth) | s
            nbits += depth
            if nbits == 8:
                out.append(acc)
                acc, nbits = 0, 0
    if nbits:
        out.append(acc << (8 - nbits))
    return bytes(out)


ADAM7 = [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4),
         (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)]


def make_png(w, h, ctype, depth, img, interlace=False, palette=None, trns=None, text=None):
    chans = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ctype]
    bpp = max(1, chans * depth // 8)
    raw = bytearray()
    rowno = 0
    passes = ADAM7 if interlace else [(0, 0, 1, 1)]
    for (x0, y0, dx, dy) in passes:
        prev = None
        for y in range(y0, h, dy):
            pix = [img[y][x] for x in range(x0, w, dx)]
            if not pix:
                continue
            row = pack_row(pix, depth, chans)
            raw += filter_row(rowno % 5, row, prev, bpp)   # every filter type
            prev = row
            rowno += 1
    out = b"\x89PNG\r\n\x1a\n"
    out += png_chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, depth, ctype, 0, 0, 1 if interlace else 0))
    if palette:
        out += png_chunk(b"PLTE", bytes(c for rgb in palette for c in rgb))
    if trns:
        out += png_chunk(b"tRNS", bytes(trns))
    if text:
        out += png_chunk(b"tEXt", text[0] + b"\0" + text[1])
    out += png_chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    out += png_chunk(b"IEND", b"")
    return out


def expect_rgba(w, h, ctype, depth, img, palette=None, trns=None):
    """What libpng delivers with png_set_expand + strip_16 + gray_to_rgb +
    add_alpha(0xff, after) -- the transforms libtest.c (and zView's
    pngldg.c, minus add_alpha) request."""
    out = bytearray()
    for y in range(h):
        for x in range(w):
            p = img[y][x]
            if ctype == 3:
                r, g, b = palette[p[0]]
                a = trns[p[0]] if (trns and p[0] < len(trns)) else 255
            else:
                def s8(v):
                    if depth == 16:
                        return v >> 8                        # strip_16 = high byte
                    if depth < 8:
                        return v * 255 // ((1 << depth) - 1)  # bit replication
                    return v
                if ctype == 0:
                    r = g = b = s8(p[0]); a = 255
                elif ctype == 4:
                    r = g = b = s8(p[0]); a = s8(p[1])
                elif ctype == 2:
                    r, g, b = (s8(v) for v in p); a = 255
                else:
                    r, g, b, a = (s8(v) for v in p)
            out += bytes((r, g, b, a))
    return bytes(out)


def pattern(w, h, f):
    return [[f(x, y) for x in range(w)] for y in range(h)]


# ---------------------------------------------------------------- JPEG -----
def rgb_image(w, h, seed):
    im = Image.new("RGB", (w, h))
    px = im.load()
    for y in range(h):
        for x in range(w):
            px[x, y] = ((x * 255 // max(1, w - 1) + seed) & 255,
                        (y * 255 // max(1, h - 1)) & 255,
                        ((x ^ y) * 13 + seed * 3) & 255)
    return im


def tiff_entry(tag, typ, count, value_or_off):
    return struct.pack("<HHI", tag, typ, count) + value_or_off


def build_exif(thumb):
    """'Exif\\0\\0' + little-endian TIFF: IFD0 (Make, Model, Orientation,
    XResolution, YResolution, ResolutionUnit, DateTime) -> IFD1 (Compression,
    JPEGInterchangeFormat/-Length = the thumbnail)."""
    make, model, dt = b"NewTOS\0", b"LIBTEST.NCP\0", b"2026:10:06 12:34:56\0"
    n0, n1 = 7, 3
    ifd0 = 8
    ifd0_size = 2 + 12 * n0 + 4
    data0 = ifd0 + ifd0_size                     # out-of-line values of IFD0
    off_make = data0
    off_model = off_make + len(make)
    off_dt = off_model + len(model)
    off_xres = off_dt + len(dt)
    off_yres = off_xres + 8
    ifd1 = off_yres + 8
    ifd1_size = 2 + 12 * n1 + 4
    off_thumb = ifd1 + ifd1_size
    t = b"II*\0" + struct.pack("<I", ifd0)
    t += struct.pack("<H", n0)
    t += tiff_entry(0x010F, 2, len(make), struct.pack("<I", off_make))
    t += tiff_entry(0x0110, 2, len(model), struct.pack("<I", off_model))
    t += tiff_entry(0x0112, 3, 1, struct.pack("<HH", 1, 0))
    t += tiff_entry(0x011A, 5, 1, struct.pack("<I", off_xres))
    t += tiff_entry(0x011B, 5, 1, struct.pack("<I", off_yres))
    t += tiff_entry(0x0128, 3, 1, struct.pack("<HH", 2, 0))
    t += tiff_entry(0x0132, 2, len(dt), struct.pack("<I", off_dt))
    t += struct.pack("<I", ifd1)
    t += make + model + dt + struct.pack("<II", 72, 1) + struct.pack("<II", 300, 4)
    assert len(t) == ifd1
    t += struct.pack("<H", n1)
    t += tiff_entry(0x0103, 3, 1, struct.pack("<HH", 6, 0))
    t += tiff_entry(0x0201, 4, 1, struct.pack("<I", off_thumb))
    t += tiff_entry(0x0202, 4, 1, struct.pack("<I", len(thumb)))
    t += struct.pack("<I", 0)
    assert len(t) == off_thumb
    t += thumb
    return b"Exif\0\0" + t


# ----------------------------------------------------------------- GIF -----
def gif_gct(b):
    flags = b[10]
    if not flags & 0x80:
        return b""
    n = 2 << (flags & 7)
    return b[13:13 + 3 * n]


def main():
    files = []     # (name, bytes)
    refs = []      # (name, crc, len)

    # zlib
    zb = lcg_text(ZBUF_LEN, ZBUF_SEED)
    refs.append(("Z.crc32", zlib.crc32(zb) & 0xFFFFFFFF, len(zb)))
    refs.append(("Z.adler32", adler32(zb), len(zb)))
    zsrc = (b"The quick brown fox jumps over the lazy dog. " * 300) + bytes(range(256)) * 8
    files.append(("ZPY", zlib.compress(zsrc, 9)))
    refs.append(("Z.inflate-py", crc(zsrc), len(zsrc)))

    # PNG
    p1 = pattern(37, 23, lambda x, y: ((x * 7) & 255, (y * 11) & 255, (x * y) & 255))
    p2 = pattern(29, 19, lambda x, y: ((x * 9) & 255, (y * 13) & 255, ((x + y) * 5) & 255, (x * 17 + y * 3) & 255))
    pal = [((i * 21) & 255, (255 - i * 19) & 255, (i * 47) & 255) for i in range(12)]
    trns = [0, 64, 128, 192, 255]
    p3 = pattern(31, 17, lambda x, y: (((x // 3) + y) % 12,))
    p4 = pattern(13, 11, lambda x, y: (((x * 5003) + (y * 977)) & 0xFFFF,))
    p5 = pattern(21, 9, lambda x, y: (((x ^ y) >> 1) & 1,))
    p6 = pattern(15, 10, lambda x, y: (((x * 16) + y) & 255, (y * 25) & 255))
    cases = [
        ("P1", 37, 23, 2, 8, p1, False, None, None, (b"Comment", b"NewTOS libtest")),
        ("P2", 29, 19, 6, 8, p2, True, None, None, None),
        ("P3", 31, 17, 3, 4, p3, True, pal, trns, None),
        ("P4", 13, 11, 0, 16, p4, False, None, None, None),
        ("P5", 21, 9, 0, 1, p5, False, None, None, None),
        ("P6", 15, 10, 4, 8, p6, False, None, None, None),
    ]
    for (n, w, h, ct, d, img, il, pl, tr, tx) in cases:
        data = make_png(w, h, ct, d, img, il, pl, tr, tx)
        files.append((n, data))
        exp = expect_rgba(w, h, ct, d, img, pl, tr)
        refs.append((n + ".rgba", crc(exp), len(exp)))
        # Pillow cross-check of the encoder (independent decoder): must agree
        # wherever Pillow's conversion is defined the same way (not 16-bit).
        if d != 16:
            pim = Image.open(io.BytesIO(data)).convert("RGBA").tobytes()
            assert pim == exp, "Pillow disagrees with the expected RGBA of " + n

    # JPEG (pixel references from the host build of IJG 9f, see header)
    def jpg(im, **kw):
        b = io.BytesIO()
        im.save(b, "JPEG", **kw)
        return b.getvalue()
    files.append(("J1", jpg(rgb_image(45, 29, 7), quality=85, subsampling=2)))
    files.append(("J2", jpg(rgb_image(33, 21, 99), quality=92, subsampling=0, progressive=True)))
    files.append(("J3", jpg(rgb_image(27, 19, 50).convert("L"), quality=75)))
    thumb = jpg(rgb_image(8, 8, 3), quality=80)
    exif = build_exif(thumb)
    files.append(("J4", jpg(rgb_image(24, 16, 21), quality=90, exif=exif,
                            comment=b"NewTOS libtest comment")))
    refs.append(("E.thumb", crc(thumb), len(thumb)))

    # GIF
    def gif(im, **kw):
        b = io.BytesIO()
        im.save(b, "GIF", **kw)
        return b.getvalue()
    gpal = []
    for i in range(32):
        gpal += [(i * 8) & 255, (255 - i * 7) & 255, (i * 37) & 255]
    g1 = Image.new("P", (41, 27)); g1.putpalette(gpal)
    g1.putdata([((x * 3 + y * 5) % 32) for y in range(27) for x in range(41)])
    g2 = Image.new("P", (23, 31)); g2.putpalette(gpal[:48])
    g2.putdata([((x + 2 * y) % 16) for y in range(31) for x in range(23)])
    g3a = Image.new("P", (16, 12)); g3a.putpalette(gpal[:24])
    g3a.putdata([((x // 2 + y) % 8) for y in range(12) for x in range(16)])
    g3b = Image.new("P", (16, 12)); g3b.putpalette(gpal[:24])
    g3b.putdata([((x + y // 2) % 8) for y in range(12) for x in range(16)])
    files.append(("G1", gif(g1, interlace=False, optimize=False)))
    files.append(("G2", gif(g2, interlace=True, optimize=False, transparency=3, comment=b"NewTOS gif")))
    files.append(("G3", gif(g3a, save_all=True, append_images=[g3b], duration=100, loop=0,
                            interlace=False, optimize=False)))
    for n in ("G1", "G2", "G3"):
        data = dict(files)[n]
        im = Image.open(io.BytesIO(data))
        im.seek(0)
        assert im.mode == "P", n
        idx = im.tobytes()
        refs.append((n + ".index", crc(idx), len(idx)))
        gct = gif_gct(data)
        refs.append((n + ".gct", crc(gct), len(gct)))

    # Pillow vs (later) IJG: informational only.
    for n in ("J1", "J2", "J3", "J4"):
        im = Image.open(io.BytesIO(dict(files)[n]))
        print("info: Pillow (libjpeg-turbo) decode of %s: %s %dx%d" % (n, im.mode, im.size[0], im.size[1]))

    # ---- write the header
    out = []
    out.append("/* libtest-data.h -- GENERATED by gen-data.py (TASK/LDG-M2). Do not edit.")
    out.append(" * Test files embedded in LIBTEST.NCP and their Python-side references")
    out.append(" * (see gen-data.py for what each reference is computed from). */")
    out.append("#define ZBUF_LEN %du" % ZBUF_LEN)
    out.append("#define ZBUF_SEED 0x%08Xu" % ZBUF_SEED)
    for (n, data) in files:
        out.append("static const unsigned char d_%s[%d] = {" % (n, len(data)))
        for i in range(0, len(data), 16):
            out.append("  " + ",".join("0x%02x" % c for c in data[i:i + 16]) + ",")
        out.append("};")
    out.append("static const struct blob { const char *name; const unsigned char *p; unsigned long len; } blobs[] = {")
    for (n, data) in files:
        out.append('  { "%s", d_%s, sizeof d_%s },' % (n, n, n))
    out.append("  { 0, 0, 0 }")
    out.append("};")
    out.append("static const struct ref py_refs[] = {")
    for (n, c, l) in refs:
        out.append('  { "%s", 0x%08xu, %du },' % (n, c, l))
    out.append("  { 0, 0, 0 }")
    out.append("};")
    with open(os.path.join(HERE, "libtest-data.h"), "w") as f:
        f.write("\n".join(out) + "\n")
    print("wrote libtest-data.h: %d files, %d python references, %d bytes of data"
          % (len(files), len(refs), sum(len(d) for _, d in files)))


if __name__ == "__main__":
    main()
