#!/usr/bin/env python3
"""pxd.py a.png b.png -- number of differing pixels and their bounding box."""
import sys
from PIL import Image, ImageChops
a = Image.open(sys.argv[1]).convert("RGB"); b = Image.open(sys.argv[2]).convert("RGB")
if a.size != b.size: print("size differs", a.size, b.size); sys.exit(1)
d = ImageChops.difference(a, b); bb = d.getbbox()
n = sum(1 for p in d.getdata() if p != (0, 0, 0))
print("%s: %d px differ, bbox %s" % (sys.argv[2].split("/")[-1], n, bb))
