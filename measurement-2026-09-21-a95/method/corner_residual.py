#!/usr/bin/env python3
"""corner_residual.py REF.png rx ry rw rh OURS.png ox oy ow oh

The three corner crops of corner_compare.py, minus what is not the frame:
  * the work area (rows 42..h-24, cols 1..w-24) and the info line's interior
    (rows 24..40, cols 1..w-2): application content, differs by app/dir;
  * the slider track interiors (vertical rows 66..h-48 x cols w-22..w-2,
    horizontal cols 24..w-48 x rows h-22..h-2): the elevator length is the
    app's WF_?SLSIZE, EmuDESK fills the track, TeraDesk does not;
  * the pointer sprite on the reference (col w-1, rows h-50..h-36);
  * IP_2PATT / IP_1PATT phase: the VDI aligns fill patterns to absolute
    screen coordinates (vdi_fill.c), so a stipple pixel is compared against
    its own screen position's expectation in each image. Name-band interior
    rows 2..20, cols 25..w-26 (IP_2PATT, ink on odd screen rows at odd
    screen columns; the topped word 0x11a1).
Prints every remaining differing pixel with frame-relative coordinates."""
import sys
from PIL import Image
ref = Image.open(sys.argv[1]).convert('RGB'); rx, ry, rw, rh = map(int, sys.argv[2:6])
our = Image.open(sys.argv[6]).convert('RGB'); ox, oy, ow, oh = map(int, sys.argv[7:11])
# optional: only the title band (rows < MAXROW) of the top crops, for a window
# whose lower part lies under another window (the untopped comparison)
maxrow = int(sys.argv[11]) if len(sys.argv) > 11 else None
def cls(p):
    r, g, b = p
    if r < 40 and g < 40 and b < 40: return 'K'
    if r > 220 and g > 220 and b > 220: return 'W'
    if 100 < r < 175 and 100 < g < 175 and 100 < b < 175 and abs(r-g) < 20 and abs(g-b) < 20: return 'G'
    return '?'
def ip2(x, y): return (y % 4) % 2 == 1 and (x % 16) % 2 == 1
def ip1(x, y):
    row = {1: 0x4444, 3: 0x1111}.get(y % 4, 0)
    return row & (0x8000 >> (x % 16)) != 0
crops = [('TL', 0, 0, 120, 48), ('TR', -72, 0, 72, 48), ('BR', -72, -72, 72, 72)]
if maxrow is not None:
    crops = [('TL', 0, 0, 120, maxrow), ('TR', -72, 0, 72, maxrow)]
def excluded(dx, dy, w, h):
    # dx, dy >= 0 frame-relative
    if 42 <= dy <= h-24 and 1 <= dx <= w-24: return 'work area'
    if 24 <= dy <= 40 and 1 <= dx <= w-2: return 'info text'
    if dx == w-1 and h-50 <= dy <= h-36: return 'pointer'
    return None
def pattern_at(dx, dy, w, h):
    """Which screen-aligned VDI pattern the frame shows at (dx, dy): the
    name box interior (rows 1..21 inside the 3D-grown box, IP_2PATT), the
    W_VSLIDE interior (rows 66..h-47 of the bar column, IP_1PATT) or the
    W_HSLIDE interior (cols 24..w-47 of the band, IP_1PATT)."""
    if 1 <= dy <= 21 and 24 <= dx <= w-25: return ip2, 'name'
    if 66 <= dy <= h-47 and w-22 <= dx <= w-2: return ip1, 'v-track'
    if h-22 <= dy <= h-2 and 24 <= dx <= w-47: return ip1, 'h-track'
    return None, None
total = 0; excl = {}
for name, cx, cy, cw, ch in crops:
    rdx0 = cx if cx >= 0 else rw + cx; rdy0 = cy if cy >= 0 else rh + cy
    odx0 = cx if cx >= 0 else ow + cx; ody0 = cy if cy >= 0 else oh + cy
    res = []
    for y in range(ch):
        for x in range(cw):
            rdx, rdy = rdx0 + x, rdy0 + y; odx, ody = odx0 + x, ody0 + y
            rp = cls(ref.getpixel((rx + rdx, ry + rdy))); op = cls(our.getpixel((ox + odx, oy + ody)))
            if rp == op: continue
            e = excluded(rdx, rdy, rw, rh)
            if e is None:
                pat, region = pattern_at(rdx, rdy, rw, rh)
                if pat is not None:
                    want_r = 'K' if pat(rx + rdx, ry + rdy) else 'W'
                    want_o = 'K' if pat(ox + odx, oy + ody) else 'W'
                    if rp == want_r and op == want_o:
                        e = f'{region} pattern phase'
                    elif region in ('v-track', 'h-track'):
                        e = f'{region} elevator length'
            if e:
                excl[e] = excl.get(e, 0) + 1
            else:
                fx = rdx if cx >= 0 else rdx - rw; fy = rdy if cy >= 0 else rdy - rh
                res.append((fx, fy, rp, op))
    total += len(res)
    print(f"{name}: {len(res)} residual differing pixels")
    for d in res: print(f"   ({d[0]:4d},{d[1]:4d}) ref {d[2]} -> ours {d[3]}")
print("excluded (not frame / phase):", excl)
print(f"RESIDUAL total {total}")
