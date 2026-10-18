#!/usr/bin/env python3
"""pixcmp.py <dir> <card> -- compare <card>-head-<mark>.png with <card>-wt-<mark>.png for every mark: IDENTICAL or the number of
differing pixels + bounding box. Writes nothing."""
import sys, glob, os
from PIL import Image, ImageChops
d, card = sys.argv[1:3]
for h in sorted(glob.glob(f"{d}/{card}-head-*.png")):
    mark = os.path.basename(h)[len(card) + 6:-4]; g = h.replace(f"{card}-head-", f"{card}-wt-")
    if not os.path.exists(g): print(f"{mark:28s} MISSING green"); continue
    a, b = Image.open(h).convert("RGB"), Image.open(g).convert("RGB")
    if a.size != b.size: print(f"{mark:28s} SIZE {a.size} {b.size}"); continue
    diff = ImageChops.difference(a, b); bb = diff.getbbox()
    if bb is None: print(f"{mark:28s} IDENTICAL")
    else:
        n = sum(1 for p in diff.convert("L").getdata() if p)
        print(f"{mark:28s} DIFF {n} px, bbox {bb}")
