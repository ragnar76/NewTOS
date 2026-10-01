#!/usr/bin/env python3
"""zoom.py IN.png OUT.png X Y W H [SCALE]  -- nearest-neighbour crop+zoom.

Used only to make a screendump detail readable in the report; the full-size
dump beside it is the evidence, this is a magnifier over the same pixels.
"""
import sys
from PIL import Image
inp, outp = sys.argv[1], sys.argv[2]
x, y, w, h = (int(v) for v in sys.argv[3:7])
s = int(sys.argv[7]) if len(sys.argv) > 7 else 3
im = Image.open(inp).convert('RGB').crop((x, y, x + w, y + h))
im.resize((w * s, h * s), Image.NEAREST).save(outp)
print("wrote", outp)
