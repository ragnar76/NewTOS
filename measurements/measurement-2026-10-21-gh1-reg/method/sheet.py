#!/usr/bin/env python3
"""sheet.py OUT.png DIR PREFIX name... -- contact sheet (labelled, 2 columns, half size) of DIR/PREFIX-name.png"""
import sys
from PIL import Image, ImageDraw
out, d, pre = sys.argv[1:4]; names = sys.argv[4:]
W, H = 480, 360; cols = 2; rows = (len(names) + 1) // 2
m = Image.new("RGB", (cols * W, rows * H), (60, 0, 60)); dr = ImageDraw.Draw(m)
for i, n in enumerate(names):
    try: im = Image.open("%s/%s-%s.png" % (d, pre, n)).convert("RGB").resize((W, H))
    except Exception as e: im = Image.new("RGB", (W, H), (120, 0, 0))
    x, y = (i % cols) * W, (i // cols) * H; m.paste(im, (x, y)); dr.rectangle([x, y, x + 150, y + 12], fill=(0, 0, 0)); dr.text((x + 2, y), n, fill=(255, 255, 0))
m.save(out)
