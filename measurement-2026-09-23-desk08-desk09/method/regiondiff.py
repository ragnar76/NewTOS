#!/usr/bin/env python3
"""regiondiff.py A.png B.png x y w h -> differing pixels inside that rectangle."""
import sys
from PIL import Image
a = Image.open(sys.argv[1]).convert('RGB'); b = Image.open(sys.argv[2]).convert('RGB')
x, y, w, h = (int(v) for v in sys.argv[3:7])
n = 0; xs = []; ys = []
pa = a.load(); pb = b.load()
for j in range(y, y + h):
    for i in range(x, x + w):
        if pa[i, j] != pb[i, j]:
            n += 1; xs.append(i); ys.append(j)
print(f"{n} differing pixels in ({x},{y},{w},{h})" + (f", bbox ({min(xs)},{min(ys)})-({max(xs)},{max(ys)})" if n else ""))
