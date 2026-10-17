#!/usr/bin/env python3
"""sheet.py out.png in.png... -- contact sheet for visual review: 3 columns, half scale, file name above each dump."""
import sys
from PIL import Image, ImageDraw
ins = sys.argv[2:]; cols = 3; w, h = 320, 240
rows = (len(ins) + cols - 1) // cols
sh = Image.new("RGB", (cols * w, rows * (h + 12)), (60, 60, 60)); d = ImageDraw.Draw(sh)
for i, p in enumerate(ins):
    im = Image.open(p).convert("RGB").resize((w, h), Image.NEAREST)
    x, y = (i % cols) * w, (i // cols) * (h + 12)
    sh.paste(im, (x, y + 12)); d.text((x + 2, y), p.split('/')[-1][:52], fill=(255, 255, 0))
sh.save(sys.argv[1])
