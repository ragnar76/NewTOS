#!/usr/bin/env python3
"""compare-dumps.py <old-dir> <new-dir> <name>...
Per screendump: size, bounding box of the non-black pixels (the picture's
place on the screen) and whether old and new are pixel-identical. The attract
demo is not frame-locked to the wall clock, so identical content is not
expected; the bounding box is the geometry check."""
import sys, os
from PIL import Image, ImageChops
old, new = sys.argv[1], sys.argv[2]
for n in sys.argv[3:]:
    a = Image.open(os.path.join(old, n)).convert('RGB')
    b = Image.open(os.path.join(new, n)).convert('RGB')
    same = a.size == b.size and ImageChops.difference(a, b).getbbox() is None
    print(f"{n:28s} old {a.size} bbox {a.getbbox()}  new {b.size} bbox {b.getbbox()}  "
          f"{'IDENTICAL' if same else 'content differs'}")
