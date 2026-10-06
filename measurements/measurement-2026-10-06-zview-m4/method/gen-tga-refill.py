#!/usr/bin/env python3
"""gen-tga-refill.py <outdir> -- a synthetic RLE TGA (TREFILL.TGA, 270x48) that
triggers the line-buffer refill defect of zView's TGA codec
(z-tools-04805b7/zview/plugins/tga/tgaldg.c fill_img_buf :304-327): the
buffer holds 2 * line_size bytes and is refilled only when the read offset
has passed half of it, without checking that the next line's packets are
all in the buffer. A line of 806 bytes (just under half the buffer, so no
refill) followed by a line of 820 bytes (raw packets, more than the 814
bytes left) makes the decoder read past the valid data. Writes
<outdir>/TREFILL.TGA and <outdir>/TREFILL.TGA.rgb (screen-quantised
expected picture) and prints the rows a model of the upstream buffer logic
predicts to underrun."""
import os, random, struct, sys
out = sys.argv[1]; os.makedirs(out, exist_ok=True)
W, H = 270, 48
rnd = random.Random(4711)
noise = lambda n: [(rnd.randrange(256), rnd.randrange(256), rnd.randrange(256)) for _ in range(n)]
rows, packets = [], []
for y in range(H):
    if y % 2 == 0:
        # line B: noise in 10 raw packets of 27 pixels = 810 + 10 = 820 bytes
        segs = [("raw", noise(27)) for _ in range(10)]
    else:
        # line A: 4 two-pixel runs between 4 raw packets (66,66,65,65)
        # = 262*3 + 4 + 4*(1+3) = 806 bytes, just under half the buffer
        segs = []
        for n in (66, 66, 65, 65):
            segs.append(("raw", noise(n)))
            c = noise(1)[0]
            segs.append(("rle", [c, c]))
    row = [p for (_, px) in segs for p in px]
    assert len(row) == W
    rows.append(row); packets.append(segs)
body = bytearray(); sizes = []
for segs in packets:
    s = len(body)
    for kind, px in segs:
        if kind == "raw":
            body.append(len(px) - 1); body += b"".join(bytes((b, g, r)) for (r, g, b) in px)
        else:
            r, g, b = px[0]; body.append(0x80 | (len(px) - 1)); body += bytes((b, g, r))
    sizes.append(len(body) - s)
open(os.path.join(out, "TREFILL.TGA"), "wb").write(
    struct.pack("<BBBHHBHHHHBB", 0, 0, 10, 0, 0, 0, 0, 0, W, H, 24, 0x20) + bytes(body))
q = lambda c: ((c >> 3) << 3) | ((c >> 3) >> 2)
with open(os.path.join(out, "TREFILL.TGA.rgb"), "wb") as f:
    f.write(struct.pack("<HH", W, H))
    for row in rows:
        for (r, g, b) in row:
            f.write(bytes((q(r), q(g), q(b))))
ls = W * 3; blen = 2 * ls; rest = len(body); used = min(blen, rest); rest -= used; off = 0; under = []
for y, need in enumerate(sizes):
    if need > used:
        under.append((y, need, used))
    used -= need; off += need
    if off >= blen >> 1:
        r = min(off, rest); used += r; rest -= r; off = 0
print("line_size", ls, "max line", max(sizes), "model underruns (row, needed, in buffer):", under[:6])
