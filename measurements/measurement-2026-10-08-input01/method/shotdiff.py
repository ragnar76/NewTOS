#!/usr/bin/env python3
"""shotdiff.py <dirA> <prefixA> <dirB> <prefixB> -- INPUT01: compare the
screendumps of two runs like shotcmp.py (every <prefixA><rest>.png in dirA
against <prefixB><rest>.png in dirB), and for a differing pair describe WHERE
they differ: the differing pixels are grouped into connected regions
(8-neighbourhood over 4x4 cells); each region is printed with its bounding
box. A region no larger than 20x20 px is a candidate for the 16x16 mouse
cursor (arrow) -- flagged CURSOR?; anything bigger is flagged OTHER and must
be looked at. Needs PIL (.venv)."""
import sys, os, hashlib
from PIL import Image, ImageChops
da, pa, db, pb = sys.argv[1:5]
def h(p): return hashlib.sha256(open(p, 'rb').read()).hexdigest()
same = cur = other = miss = 0
for f in sorted(os.listdir(da)):
    if not (f.startswith(pa) and f.endswith('.png')):
        continue
    rest = f[len(pa):]
    if rest.startswith(('strict-', 'fsck-')):
        continue  # another run whose tag extends this prefix (desk-strict-*, doom-fsck-*)
    g = os.path.join(db, pb + rest)
    if not os.path.exists(g):
        print('MISSING', pb + rest); miss += 1; continue
    if h(os.path.join(da, f)) == h(g):
        same += 1; continue
    A = Image.open(os.path.join(da, f)).convert('RGB'); B = Image.open(g).convert('RGB')
    if A.size != B.size:
        print('SIZE', rest, A.size, B.size); other += 1; continue
    d = ImageChops.difference(A, B).convert('L').point(lambda v: 255 if v else 0)
    W, H = d.size; px = d.load(); C = 4
    cells = set()
    for y in range(H):
        for x in range(W):
            if px[x, y]:
                cells.add((x // C, y // C))
    regions = []
    seen = set()
    for c in cells:
        if c in seen: continue
        stack = [c]; seen.add(c); xs = []; ys = []
        while stack:
            cx, cy = stack.pop(); xs.append(cx); ys.append(cy)
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    n = (cx + dx, cy + dy)
                    if n in cells and n not in seen:
                        seen.add(n); stack.append(n)
        regions.append((min(xs) * C, min(ys) * C, (max(xs) + 1) * C, (max(ys) + 1) * C))
    kinds = []
    for x0, y0, x1, y1 in sorted(regions):
        k = 'CURSOR?' if (x1 - x0) <= 20 and (y1 - y0) <= 20 else 'OTHER'
        kinds.append(k)
        print(f'DIFF {rest}: {k} ({x0},{y0})-({x1},{y1})')
    if all(k == 'CURSOR?' for k in kinds) and len(kinds) <= 2:
        cur += 1
    else:
        other += 1
print(f'{pa} vs {pb}: same={same} cursor-only={cur} other={other} missing={miss}')
