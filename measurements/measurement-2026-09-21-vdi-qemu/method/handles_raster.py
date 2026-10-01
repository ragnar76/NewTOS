#!/usr/bin/env python3
"""V15 (raster half): contrl[6] / contrl_before[6] per record. Raster record format:
32-byte header, 3696-byte records; contrl[15] at +8, contrl_before[15] at +38 (tosdump/core/tosdump.h:248-262)."""
import struct, sys
from collections import Counter
HDR, REC = 32, 3696
def pairs(path):
    b = open(path, 'rb').read()
    n = (len(b) - HDR) // REC
    out = []
    for i in range(n):
        r = b[HDR + i*REC: HDR + (i+1)*REC]
        case = struct.unpack('>H', r[0:2])[0]
        op = struct.unpack('>h', r[2:4])[0]
        contrl = struct.unpack('>15h', r[8:8+30])
        before = struct.unpack('>15h', r[38:38+30])
        out.append((case, op, contrl[6], before[6]))
    return out
names = sys.argv[1::2]; paths = sys.argv[2::2]
data = {n: pairs(p) for n, p in zip(names, paths)}
print("records:", {n: len(d) for n, d in data.items()})
for n, d in data.items():
    print(f"  {n}: pairs (before, after) = {dict(Counter((b, a) for (_, _, a, b) in d))}")
ref = data[names[0]]
for n in names[1:]:
    diffs = [(x, y) for x, y in zip(ref, data[n]) if (x[2], x[3]) != (y[2], y[3])]
    print(f"  {n} vs {names[0]}: {len(diffs)} records with a different handle pair")
