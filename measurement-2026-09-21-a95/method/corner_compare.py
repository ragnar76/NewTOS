#!/usr/bin/env python3
"""corner_compare.py REF.png rx ry rw rh OURS.png ox oy ow oh OUT.png [label]

The A79/A94 corner comparison: three frame-relative crops — TL (0,0)+120x48,
TR (w-72,0)+72x48, BR (w-72,h-72)+72x72 — of the reference window and ours,
rendered side by side at 4x (top row = reference, bottom row = ours), and a
pixel-class diff per crop (classes: black, white, grey = EmuTOS LBLACK, other),
listing every differing pixel with frame-relative coordinates."""
import sys
from PIL import Image
ref = Image.open(sys.argv[1]).convert('RGB'); rx, ry, rw, rh = map(int, sys.argv[2:6])
our = Image.open(sys.argv[6]).convert('RGB'); ox, oy, ow, oh = map(int, sys.argv[7:11])
out = sys.argv[11]
label = sys.argv[12] if len(sys.argv) > 12 else ''
def cls(p):
    r, g, b = p
    if r < 40 and g < 40 and b < 40: return 'K'
    if r > 220 and g > 220 and b > 220: return 'W'
    if 100 < r < 175 and 100 < g < 175 and 100 < b < 175 and abs(r-g) < 20 and abs(g-b) < 20: return 'G'
    return '?'
crops = [('TL', 0, 0, 120, 48), ('TR', -72, 0, 72, 48), ('BR', -72, -72, 72, 72)]
Z = 4
tiles = []
total = 0
for name, cx, cy, cw, ch in crops:
    rx0 = rx + (cx if cx >= 0 else rw + cx); ry0 = ry + (cy if cy >= 0 else rh + cy)
    ox0 = ox + (cx if cx >= 0 else ow + cx); oy0 = oy + (cy if cy >= 0 else oh + cy)
    rc = ref.crop((rx0, ry0, rx0 + cw, ry0 + ch)); oc = our.crop((ox0, oy0, ox0 + cw, oy0 + ch))
    diffs = []
    for y in range(ch):
        for x in range(cw):
            a = cls(rc.getpixel((x, y))); b = cls(oc.getpixel((x, y)))
            if a != b:
                fx = cx + x if cx >= 0 else cx + x   # frame-relative (negative = from right)
                fy = cy + y if cy >= 0 else cy + y
                diffs.append((fx, fy, a, b))
    total += len(diffs)
    print(f"{name}: {len(diffs)} differing pixels (frame-relative dx,dy ref->ours)")
    for d in diffs:
        print(f"   ({d[0]:4d},{d[1]:4d}) {d[2]}->{d[3]}")
    tiles.append((rc.resize((cw*Z, ch*Z), Image.NEAREST), oc.resize((cw*Z, ch*Z), Image.NEAREST)))
W = sum(t[0].width for t in tiles) + 8 * (len(tiles) - 1)
H = max(t[0].height for t in tiles) * 2 + 8
img = Image.new('RGB', (W, H), (255, 0, 255))
x = 0
for r, o in tiles:
    img.paste(r, (x, 0)); img.paste(o, (x, r.height + 8)); x += r.width + 8
img.save(out)
print(f"total {total} differing pixels; composite {out}")
