#!/usr/bin/env python3
"""Compare the FSELECTR *geometry* of our screendump against the rscview
reference, in root-relative coordinates.

Both images render the same resource at the same 8x16 cell metrics, so the
root box is 320x356 in both and every box edge must land on the same
root-relative pixel. Only runs of >= MINRUN black pixels are compared, which
keeps text glyphs (<= 7 px per cell) and dithers out of the comparison: what
is left are the box borders -- exactly what fs_start moves.

usage: compare-geometry.py OURS.png REFERENCE.png
"""
import sys
from PIL import Image

MINRUN = 10


def black_mask(path):
    im = Image.open(path).convert("RGB")
    px = im.load()
    w, h = im.size
    return [[px[x, y] == (0, 0, 0) for x in range(w)] for y in range(h)], w, h


def root_origin(mask, w, h):
    """The dialog root box: the topmost row carrying a >=300 px black run."""
    for y in range(h):
        x = 0
        while x < w:
            if mask[y][x]:
                s = x
                while x < w and mask[y][x]:
                    x += 1
                if x - s >= 300:
                    return s, y, x - s
            else:
                x += 1
    raise SystemExit("no root box found in image")


def runs(row, x0, x1, minrun):
    out = []
    x = x0
    while x < x1:
        if row[x]:
            s = x
            while x < x1 and row[x]:
                x += 1
            if x - s >= minrun:
                out.append((s - x0, x - 1 - x0))
        else:
            x += 1
    return out


def main(ours, ref):
    ma, wa, ha = black_mask(ours)
    mb, wb, hb = black_mask(ref)
    ax, ay, aw = root_origin(ma, wa, ha)
    bx, by, bw = root_origin(mb, wb, hb)
    print(f"root box: ours at ({ax},{ay}) w={aw}   reference at ({bx},{by}) w={bw}")
    if aw != bw:
        print("ROOT WIDTH DIFFERS -- comparison aborted")
        return 1
    # root height: last row with a >=300 px run
    def rooth(m, x0, w0, h0):
        last = 0
        for y in range(h0):
            if runs(m[y], x0, x0 + w0, 300):
                last = y
        return last
    aH = rooth(ma, ax, aw, ha) - ay + 1
    bH = rooth(mb, bx, bw, hb) - by + 1
    print(f"root height: ours {aH}  reference {bH}  ({'EQUAL' if aH == bH else 'DIFFER'})")
    same = diff = 0
    for k in range(min(aH, bH)):
        ra = runs(ma[ay + k], ax, ax + aw, MINRUN)
        rb = runs(mb[by + k], bx, bx + bw, MINRUN)
        if ra == rb:
            same += 1
        else:
            diff += 1
            print(f"  y={k:3d}  ours={ra}")
            print(f"         ref ={rb}")
    print(f"\n{same} root-relative rows identical, {diff} different "
          f"(runs of >= {MINRUN} black px)")
    return 0 if diff == 0 else 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
