#!/usr/bin/env python3
"""cmp-dialog.py REF NEW [x0 y0 x1 y1] -- compare the ADBKGND dialog area of a Hatari reference screenshot (644x516,
offset searched) with a NewTOS QEMU screendump (640x480), pixel classes rounded to the nearest of
{black, white, grey 189, dark grey, other} after a per-channel quantisation (Hatari's Falcon palette grey is
(189,186,189), QEMU's (189,189,189)). Prints the best offset, the number of differing pixels per class pair, and writes
<NEW>-diff.png (red = differ) of the compared rectangle."""
import sys
from PIL import Image
ref = Image.open(sys.argv[1]).convert('RGB'); new = Image.open(sys.argv[2]).convert('RGB')
x0, y0, x1, y1 = (int(v) for v in sys.argv[3:7]) if len(sys.argv) > 6 else (164, 116, 476, 382)
def q(p):
    return tuple((c + 8) // 16 for c in p)
best = None
for dx in range(-4, 6):
    for dy in range(-4, 40):
        d = 0
        for y in range(y0, y1, 3):
            for x in range(x0, x1, 3):
                if q(ref.getpixel((x + dx, y + dy))) != q(new.getpixel((x, y))):
                    d += 1
        if best is None or d < best[0]:
            best = (d, dx, dy)
_, dx, dy = best
print("offset ref = new + (%d,%d)" % (dx, dy))
diff = Image.new('RGB', (x1 - x0, y1 - y0)); n = 0; pairs = {}
for y in range(y0, y1):
    for x in range(x0, x1):
        a = ref.getpixel((x + dx, y + dy)); b = new.getpixel((x, y))
        if q(a) != q(b):
            n += 1; pairs[(a, b)] = pairs.get((a, b), 0) + 1
            diff.putpixel((x - x0, y - y0), (255, 0, 0))
        else:
            diff.putpixel((x - x0, y - y0), tuple(c // 3 for c in b))
print("rect new (%d,%d)-(%d,%d): %d of %d pixels differ" % (x0, y0, x1, y1, n, (x1 - x0) * (y1 - y0)))
for k, v in sorted(pairs.items(), key=lambda kv: -kv[1])[:8]:
    print("  ref %s -> new %s: %d" % (k[0], k[1], v))
diff.save(sys.argv[2].rsplit('.', 1)[0] + '-diff.png')
