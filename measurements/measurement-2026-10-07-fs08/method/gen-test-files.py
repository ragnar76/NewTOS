#!/usr/bin/env python3
"""gen-test-files.py <outdir> -- FS08 test files: FS08TEST.BIN (4 MiB,
deterministic pseudo-random) and BIG.JPG (1932x2576 synthetic photo-like
JPEG, quality 92). Deterministic (fixed seeds)."""
import sys, os, random
from PIL import Image, ImageFilter
import numpy as np
out = sys.argv[1]; os.makedirs(out, exist_ok=True)
r = random.Random(808)
with open(os.path.join(out, "FS08TEST.BIN"), "wb") as f:
    f.write(bytes(r.getrandbits(8) for _ in range(4 * 1024 * 1024)))
rng = np.random.default_rng(808)
w, h = 1932, 2576
y, x = np.mgrid[0:h, 0:w]
base = np.stack([(x * 255 // w), (y * 255 // h), ((x + y) * 255 // (w + h))], axis=-1).astype(np.float32)
noise = rng.normal(0, 28, (h, w, 3)).astype(np.float32)
img = np.clip(base + noise, 0, 255).astype(np.uint8)
im = Image.fromarray(img, "RGB").filter(ImageFilter.GaussianBlur(0.6))
im.save(os.path.join(out, "BIG.JPG"), quality=92)
for n in ("FS08TEST.BIN", "BIG.JPG"):
    print(n, os.path.getsize(os.path.join(out, n)))
