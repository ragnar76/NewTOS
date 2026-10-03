#!/usr/bin/env python3
"""contact-sheet.py <out.png> <row-label>=<dir-with-pngs> ...
Rows = runs, columns = screenshots in name order; each cell scaled to 320x240
(Hatari's TT-low grab is 640x480: the TT pixel shown 2 wide, 1 tall, i.e.
the picture as on the glass). Label in the top-left corner of each row."""
import sys, glob, os
from PIL import Image, ImageDraw
out = sys.argv[1]; rows = [a.split('=', 1) for a in sys.argv[2:]]
cols = max(len(glob.glob(os.path.join(d, '*.png'))) for _, d in rows)
W, H = 320, 240
sheet = Image.new('RGB', (W * cols, H * len(rows)), (255, 255, 255))
dr = ImageDraw.Draw(sheet)
for r, (label, d) in enumerate(rows):
    for c, f in enumerate(sorted(glob.glob(os.path.join(d, '*.png')))):
        sheet.paste(Image.open(f).convert('RGB').resize((W, H)), (c * W, r * H))
    dr.rectangle([0, r * H, 8 * len(label) + 6, r * H + 14], fill=(255, 255, 255))
    dr.text((3, r * H + 2), label, fill=(0, 0, 0))
sheet.save(out)
print(out, sheet.size)
