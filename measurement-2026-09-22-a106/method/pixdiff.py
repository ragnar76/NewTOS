#!/usr/bin/env python3
"""pixdiff.py A.png B.png -> prints the number of differing pixels and their bbox."""
import sys
from PIL import Image, ImageChops
a = Image.open(sys.argv[1]).convert('RGB'); b = Image.open(sys.argv[2]).convert('RGB')
if a.size != b.size:
    print("size differs", a.size, b.size); sys.exit(1)
d = ImageChops.difference(a, b)
bbox = d.getbbox()
n = sum(1 for p in d.getdata() if p != (0, 0, 0))
print(f"{n} differing pixels, bbox {bbox}")
sys.exit(0 if n == 0 else 2)
