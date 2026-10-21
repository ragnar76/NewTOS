#!/usr/bin/env python3
"""cmp3.py MODE -- pixel diff of every ../screendumps/fx2-MODE-*.png against the GH1-reg run's head-MODE-* (HEAD f1c225fc) and wt-MODE-* (GH1 tree before fix 2)."""
import sys, glob, os
from PIL import Image
here = os.path.dirname(os.path.abspath(__file__))
mine = os.path.join(here, "..", "screendumps"); reg = os.path.join(here, "..", "..", "measurement-2026-10-21-gh1-reg", "screendumps")
def diff(a, b):
    if not os.path.exists(b): return None
    A = Image.open(a).convert("RGB"); B = Image.open(b).convert("RGB")
    if A.size != B.size: return -1
    pa, pb = A.tobytes(), B.tobytes(); n = 0
    for i in range(0, len(pa), 3):
        if pa[i:i+3] != pb[i:i+3]: n += 1
    return n
mode = sys.argv[1]; s = {"head": 0, "wt": 0}; tot = 0
for f in sorted(glob.glob(os.path.join(mine, "fx2-%s-*.png" % mode))):
    name = os.path.basename(f)[len("fx2-%s-" % mode):]; tot += 1
    h = diff(f, os.path.join(reg, "head-%s-%s" % (mode, name))); w = diff(f, os.path.join(reg, "wt-%s-%s" % (mode, name)))
    s["head"] += h == 0; s["wt"] += w == 0
    print("%-28s vsHEAD %8s  vsGH1wt %8s" % (name, h, w))
print("mode %s: %d shots, identical to HEAD %d, identical to GH1-wt %d" % (mode, tot, s["head"], s["wt"]))
