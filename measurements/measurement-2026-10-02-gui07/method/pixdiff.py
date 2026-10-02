#!/usr/bin/env python3
"""pixdiff.py <before-dir> <after-dir> [glob]

GUI07: compare the screendumps of the two arms pixel for pixel.
For every PNG present in both directories under the same name: identical or
the number of differing pixels and their bounding box. Then, because a run's
content at a fixed wall-clock offset depends on how fast the kernel is (Doom
and the ball are time-driven), every AFTER dump is also matched against ALL
BEFORE dumps of the same run prefix: "same picture as before/<name>" means a
pixel-identical picture exists in the other arm.
"""
import glob, os, sys, hashlib
from PIL import Image

b, a = sys.argv[1], sys.argv[2]
pat = sys.argv[3] if len(sys.argv) > 3 else '*.png'

def load(p):
    im = Image.open(p).convert('RGB')
    return im.size, im.tobytes()

def digest(p):
    size, data = load(p)
    return hashlib.sha256(bytes(str(size), 'ascii') + data).hexdigest()

names = sorted(os.path.basename(p) for p in glob.glob(os.path.join(a, pat)))
before_by_hash = {}
for p in glob.glob(os.path.join(b, pat)):
    before_by_hash.setdefault(digest(p), []).append(os.path.basename(p))

for n in names:
    pa, pb = os.path.join(a, n), os.path.join(b, n)
    line = n + ': '
    if os.path.exists(pb):
        (sa, da), (sb, db) = load(pa), load(pb)
        if sa != sb:
            line += 'SIZE DIFFERS %s vs %s' % (sb, sa)
        elif da == db:
            line += 'IDENTICAL to before'
        else:
            w, h = sa
            diff = [(i // 3) for i in range(0, len(da), 3) if da[i:i+3] != db[i:i+3]]
            xs = [p % w for p in diff]; ys = [p // w for p in diff]
            line += 'differs in %d px, bbox x %d..%d y %d..%d' % (len(diff), min(xs), max(xs), min(ys), max(ys))
    else:
        line += 'no before dump of that name'
    same = before_by_hash.get(digest(pa), [])
    if same:
        line += ' | same picture as before/' + ','.join(sorted(same))
    print(line)
