#!/usr/bin/env python3
"""Count differing pixels between two screendumps, optionally inside a box.

usage: pixdiff.py A.png B.png [X0 Y0 X1 Y1]
Prints the number of differing pixels and the total in the box.
"""
import sys
from PIL import Image
a = Image.open(sys.argv[1]).convert('RGB')
b = Image.open(sys.argv[2]).convert('RGB')
if len(sys.argv) > 3:
    box = tuple(int(v) for v in sys.argv[3:7])
    a = a.crop(box); b = b.crop(box)
pa, pb = a.load(), b.load()
w, h = a.size
n = sum(1 for y in range(h) for x in range(w) if pa[x, y] != pb[x, y])
print(f"{sys.argv[1]} vs {sys.argv[2]}: {n} differing pixels of {w*h}")
