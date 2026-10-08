#!/usr/bin/env python3
"""strips.py OUT.png Y0 Y1 IMG... -- INPUT01 evidence picture: the band
Y0..Y1 of each screendump, stacked top to bottom with its file name, so the
pointer's x position over a screendump burst is visible at a glance (the
zv-*-move scripts move the pointer along one row). Needs PIL (.venv)."""
import sys, os
from PIL import Image, ImageDraw
out, y0, y1, imgs = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4:]
h = y1 - y0
strips = []
for p in imgs:
    im = Image.open(p).convert('RGB')
    s = im.crop((0, y0, im.width, y1))
    lab = Image.new('RGB', (im.width, 12), (255, 255, 255))
    ImageDraw.Draw(lab).text((2, 0), os.path.basename(p), fill=(0, 0, 0))
    strips += [lab, s]
W = max(s.width for s in strips)
H = sum(s.height for s in strips)
o = Image.new('RGB', (W, H), (128, 128, 128))
y = 0
for s in strips:
    o.paste(s, (0, y)); y += s.height
o.save(out)
print(out, W, H)
