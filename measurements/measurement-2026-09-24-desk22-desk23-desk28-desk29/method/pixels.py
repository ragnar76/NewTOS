#!/usr/bin/env python3
"""DESK23: read the four pixels of the window frame's drop-shadow notch out of
the screendumps of this measurement.

usage: pixels.py SCREENDUMP_DIR

Window 1 is (48,43,544,400) -- `wind_get(1, WF_CXYWH)` in serial/d29-serial.log.
With a border thickness of 1 the drop shadow's two bands are, per EmuTOS
`ob_draw`'s SHADOWED block (aes/gemoblib.c:893-900) after `gr_inside(&t,th)`:

    bottom   x = 49 .. 591,  y = 443 .. 444
    right    x = 592 .. 593, y = 44  .. 444

and the frame's own `gr_box` covers x = 48..591, y = 43..442. So four pixels
belong to neither -- (592,43), (593,43), (48,443), (48,444) -- and no draw
path writes them.
"""
import sys
from PIL import Image
import os

d = sys.argv[1]
NOTCH = [(592, 43), (593, 43), (48, 443), (48, 444)]
NAME = {(0, 255, 0): "green", (255, 255, 255): "white",
        (0, 0, 0): "black", (189, 189, 189): "grey189"}
for f in sorted(os.listdir(d)):
    if not f.endswith('.png'):
        continue
    im = Image.open(os.path.join(d, f)).convert('RGB')
    cells = " ".join("%s=%-7s" % (p, NAME.get(im.getpixel(p), im.getpixel(p)))
                     for p in NOTCH)
    print("%-26s %s" % (f, cells))
