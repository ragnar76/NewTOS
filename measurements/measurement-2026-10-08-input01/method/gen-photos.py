#!/usr/bin/env python3
"""gen-photos.py <outdir> -- 8 deterministic photo-like JPEGs X01..X08.JPG
(2048x1536, quality 90, gradient + blobs + noise) for the ZVPROF photo run."""
import sys, os
from PIL import Image, ImageFilter, ImageDraw
import numpy as np
out = sys.argv[1]; os.makedirs(out, exist_ok=True)
for k in range(1, 9):
    rng = np.random.default_rng(1000 + k)
    w, h = 2048, 1536
    y, x = np.mgrid[0:h, 0:w]
    base = np.stack([(x * 255 // w + 40 * k) % 256, (y * 255 // h), ((x + y) * 255 // (w + h))], axis=-1).astype(np.float32)
    noise = rng.normal(0, 14, (h, w, 3)).astype(np.float32)
    im = Image.fromarray(np.clip(base + noise, 0, 255).astype(np.uint8), "RGB")
    d = ImageDraw.Draw(im)
    for _ in range(40):
        cx, cy, r = rng.integers(0, w), rng.integers(0, h), rng.integers(20, 200)
        d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=tuple(int(v) for v in rng.integers(0, 255, 3)))
    im = im.filter(ImageFilter.GaussianBlur(0.8))
    p = os.path.join(out, f"X{k:02d}.JPG"); im.save(p, quality=90)
    print(p, os.path.getsize(p))
