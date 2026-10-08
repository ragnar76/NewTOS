#!/usr/bin/env python3
"""gen-landscape.py <out.jpg> -- synthetic, colourful landscape (sunset sky,
sun, layered mountains, lake with reflection, stars-free). Deterministic.
1280x960 JPEG q90, no photo content."""
import sys, numpy as np
from PIL import Image, ImageFilter
W, H = 1280, 960
rng = np.random.default_rng(7)
y, x = np.mgrid[0:H, 0:W].astype(np.float32)
hz = int(H * 0.62)
# sky gradient: deep blue -> magenta -> orange -> yellow at the horizon
t = np.clip(y / hz, 0, 1)[..., None]
stops = [(0.0, (20, 30, 110)), (0.45, (150, 50, 150)), (0.75, (250, 110, 60)), (1.0, (255, 215, 110))]
sky = np.zeros((H, W, 3), np.float32)
for (a, ca), (b, cb) in zip(stops, stops[1:]):
    m = ((t >= a) & (t <= b))
    f = (t - a) / (b - a)
    sky = np.where(m, np.array(ca) * (1 - f) + np.array(cb) * f, sky)
# sun
sx, sy, sr = W * 0.64, hz - 40, 90
d = np.hypot(x - sx, y - sy)[..., None]
glow = np.clip(1 - d / 420, 0, 1) ** 2
sky = sky + glow * np.array([255, 170, 60]) * 0.55
sky = np.where(d < sr, np.array([255, 245, 200]), sky)
img = np.clip(sky, 0, 255)
# mountain layers
def ridge(seed, base, amp, col, haze):
    r = np.random.default_rng(seed)
    n = 8
    ph = r.uniform(0, 6.28, n); fr = np.array([1, 2, 3, 5, 8, 13, 21, 34]) * 2 * np.pi / W
    am = amp / np.array([1, 1.6, 2.4, 3.5, 5, 8, 12, 18])
    prof = base + sum(am[i] * np.sin(fr[i] * x[0] + ph[i]) for i in range(n))
    return prof
layers = [(1, hz - 150, 70, (120, 60, 120)), (2, hz - 95, 60, (80, 40, 100)), (3, hz - 40, 45, (35, 25, 70))]
for seed, base, amp, col in layers:
    prof = ridge(seed, base, amp, col, 0)[None, :]
    m = (y > prof) & (y < hz + 2)
    shade = 1 - 0.25 * np.clip((y - prof) / 120, 0, 1)[..., None]
    img = np.where(m[..., None], np.array(col) * shade, img)
# lake: mirror of the upper part with ripples
lake = np.zeros((H - hz, W, 3), np.float32)
src = img[:hz][::-1][: H - hz]
ys = np.arange(H - hz)
for i in ys:
    shift = int(6 * np.sin(i / 5.0) * (i / (H - hz)) * 3)
    lake[i] = np.roll(src[min(i, src.shape[0]-1)], shift, axis=0)
tint = np.array([0.55, 0.7, 0.95])
lake = lake * tint + np.array([0, 18, 50]) * np.linspace(0.2, 1, H - hz)[:, None, None]
img[hz:] = lake
# glitter on the lake under the sun
for i in range(400):
    gy = hz + rng.integers(4, H - hz); gx = int(sx + rng.normal(0, 70 * (1 + (gy - hz) / 100)))
    if 0 <= gx < W - 30: img[gy, gx:gx + rng.integers(6, 30)] = (255, 235, 170)
out = Image.fromarray(np.clip(img, 0, 255).astype(np.uint8), "RGB").filter(ImageFilter.GaussianBlur(0.7))
out.save(sys.argv[1], quality=90)
print(sys.argv[1])
