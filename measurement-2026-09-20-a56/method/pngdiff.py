#!/usr/bin/env python3
"""pngdiff.py A B -> count of differing pixels + bbox"""
import sys
from PIL import Image
a = Image.open(sys.argv[1]).convert('RGB'); b = Image.open(sys.argv[2]).convert('RGB')
if a.size != b.size:
    print("size differs", a.size, b.size); sys.exit(2)
pa, pb = a.load(), b.load()
n = 0; x0=y0=10**9; x1=y1=-1
for y in range(a.size[1]):
    for x in range(a.size[0]):
        if pa[x,y] != pb[x,y]:
            n += 1
            x0=min(x0,x); y0=min(y0,y); x1=max(x1,x); y1=max(y1,y)
print("%d pixels differ" % n + ("" if n==0 else " bbox (%d,%d)-(%d,%d)" % (x0,y0,x1,y1)))
