#!/usr/bin/env python3
"""pixdiff.py A.png B.png [--boxes] -- DESK43 U9-2: pixel comparison of two screendumps.

Prints: the sha256 equality, the number of differing pixels, their bounding box, and with
--boxes the connected groups of differing pixels (8-neighbourhood, merged when closer than
4 px) as x,y,w,h with the pixel count and the colours seen in A and in B -- the unit the
RUNINFO lists each difference by.
"""
import hashlib
import sys

from PIL import Image


def main():
    a, b = sys.argv[1], sys.argv[2]
    boxes = '--boxes' in sys.argv
    ha = hashlib.sha256(open(a, 'rb').read()).hexdigest()
    hb = hashlib.sha256(open(b, 'rb').read()).hexdigest()
    ia, ib = Image.open(a).convert('RGB'), Image.open(b).convert('RGB')
    if ia.size != ib.size:
        print(f'SIZE {ia.size} vs {ib.size}')
        return
    w, h = ia.size
    pa, pb = ia.load(), ib.load()
    diff = [(x, y) for y in range(h) for x in range(w) if pa[x, y] != pb[x, y]]
    tag = 'BYTE-IDENTICAL' if ha == hb else ('PIXEL-IDENTICAL' if not diff else 'DIFF')
    if not diff:
        print(f'{tag} 0 px  ({ha[:12]} / {hb[:12]})')
        return
    xs = [p[0] for p in diff]
    ys = [p[1] for p in diff]
    print(f'{tag} {len(diff)} px bbox x={min(xs)}..{max(xs)} y={min(ys)}..{max(ys)}  ({ha[:12]} / {hb[:12]})')
    if not boxes:
        return
    # group: grid cells of 4 px, union-find over neighbouring occupied cells
    cells = {}
    for x, y in diff:
        cells.setdefault((x // 4, y // 4), []).append((x, y))
    parent = {c: c for c in cells}

    def find(c):
        while parent[c] != c:
            parent[c] = parent[parent[c]]
            c = parent[c]
        return c
    for (cx, cy) in cells:
        for dx in (-1, 0, 1):
            for dy in (-1, 0, 1):
                n = (cx + dx, cy + dy)
                if n in cells:
                    parent[find(n)] = find((cx, cy))
    groups = {}
    for c, pts in cells.items():
        groups.setdefault(find(c), []).extend(pts)
    for pts in sorted(groups.values(), key=lambda p: (min(q[1] for q in p), min(q[0] for q in p))):
        gx = [p[0] for p in pts]
        gy = [p[1] for p in pts]
        ca = sorted({pa[p] for p in pts})[:4]
        cb = sorted({pb[p] for p in pts})[:4]
        print(f'  box x={min(gx)} y={min(gy)} w={max(gx)-min(gx)+1} h={max(gy)-min(gy)+1} px={len(pts)} A{ca} B{cb}')


if __name__ == '__main__':
    main()
