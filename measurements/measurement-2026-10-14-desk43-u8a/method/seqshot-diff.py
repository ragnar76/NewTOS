#!/usr/bin/env python3
"""seqshot-diff.py A.png B.png -- DESK43 U7: where two screendumps differ
(bounding box, pixel count) and whether they are equal outside that box."""
import sys
from PIL import Image, ImageChops
a = Image.open(sys.argv[1]).convert('RGB'); b = Image.open(sys.argv[2]).convert('RGB')
d = ImageChops.difference(a, b); bb = d.getbbox()
n = sum(1 for p in d.get_flattened_data() if p != (0, 0, 0)) if hasattr(d, 'get_flattened_data') else sum(1 for p in d.getdata() if p != (0, 0, 0))
print(f'{sys.argv[1]} vs {sys.argv[2]}: size {a.size}/{b.size}, differing pixels {n}, bbox {bb}')
