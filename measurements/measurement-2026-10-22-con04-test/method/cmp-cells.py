#!/usr/bin/env python3
"""cmp-cells.py <reference.png> <newtos.png> -- compare two 640x480 console screens cell by cell (80x30 cells of 8x16).
Colours are compared at 5 bits per gun (Hatari renders the Falcon's 6-bit green, NewTOS stores RGB555-in-565), the
reference is cropped to 640x480 (Hatari adds a right border). Prints the differing cells as (column,row) per screen row."""
import sys
from PIL import Image

def load(p):
    im = Image.open(p).convert("RGB").crop((0, 0, 640, 480))
    return im.load()

a, b = load(sys.argv[1]), load(sys.argv[2])
q = lambda px: (px[0] >> 3, px[1] >> 3, px[2] >> 3)
diff = {}
for row in range(30):
    for col in range(80):
        if any(q(a[col * 8 + x, row * 16 + y]) != q(b[col * 8 + x, row * 16 + y]) for y in range(16) for x in range(8)):
            diff.setdefault(row, []).append(col)
total = sum(len(v) for v in diff.values())
print(f"differing cells: {total}")
for row, cols in sorted(diff.items()):
    print(f"  row {row:2}: columns {cols}")
