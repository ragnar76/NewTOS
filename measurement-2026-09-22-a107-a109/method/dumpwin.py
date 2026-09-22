#!/usr/bin/env python3
"""dumpwin.py png x y w h [x0 y0 x1 y1] -> ascii dump of the window frame-relative region.
Chars: '#' black, '.' white, 'g' grey (LBLACK 140,138,140), 'w' LWHITE, '?' other."""
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert('RGB')
x, y, w, h = map(int, sys.argv[2:6])
if len(sys.argv) > 6:
    x0, y0, x1, y1 = map(int, sys.argv[6:10])
else:
    x0, y0, x1, y1 = 0, 0, w-1, h-1
def cls(p):
    r,g,b = p
    if r < 40 and g < 40 and b < 40: return '#'
    if r > 220 and g > 220 and b > 220: return '.'
    if abs(r-140) < 12 and abs(g-138) < 12 and abs(b-140) < 12: return 'g'
    if abs(r-180) < 20 and abs(g-180) < 20 and abs(b-180) < 20: return 'w'
    return '?'
def rel(v, n):
    return v if v >= 0 else n + v
xs = list(range(rel(x0,w), rel(x1,w)+1))
ys = list(range(rel(y0,h), rel(y1,h)+1))
hdr = ''.join(f'{(c%100)//10}' for c in xs)
print('      ' + hdr)
print('      ' + ''.join(f'{c%10}' for c in xs))
for dy in ys:
    row = ''.join(cls(im.getpixel((x+dx, y+dy))) for dx in xs)
    print(f'{dy:4d}  {row}   ({dy-h})')
