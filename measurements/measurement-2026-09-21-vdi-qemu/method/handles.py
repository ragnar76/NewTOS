#!/usr/bin/env python3
"""V15: contrl[6] / contrl_before[6] per record, per dump. Record format 2:
32-byte header, 1592-byte records; contrl at +8, contrl_before at +1568 (12 words each, big-endian)."""
import struct, sys
HDR, REC = 32, 1592
def pairs(path):
    b = open(path, 'rb').read()
    n = (len(b) - HDR) // REC
    out = []
    for i in range(n):
        r = b[HDR + i*REC: HDR + (i+1)*REC]
        case = struct.unpack('>H', r[0:2])[0]
        op = struct.unpack('>h', r[2:4])[0]
        contrl = struct.unpack('>12h', r[8:8+24])
        before = struct.unpack('>12h', r[1568:1568+24])
        out.append((case, op, contrl[6], before[6], contrl[5]))
    return out
names = sys.argv[1::2]; paths = sys.argv[2::2]
data = {n: pairs(p) for n, p in zip(names, paths)}
ref = data[names[0]]
print("records:", {n: len(d) for n, d in data.items()})
print("\nper-dump: distinct (contrl_before[6], contrl[6]) pairs and how many records carry each")
for n, d in data.items():
    from collections import Counter
    c = Counter((b, a) for (_, _, a, b, _) in d)
    print(f"  {n}: {dict(c)}")
print("\nrecords where contrl[6] != contrl_before[6] (the call rewrote the handle word):")
for n, d in data.items():
    for (case, op, a, b, sub) in d:
        if a != b:
            print(f"  {n}: case {case} opcode {op} sub {sub}: before={b} after={a}")
print("\ncross-dump: records whose (before, after) pair differs from the first dump:")
for n in names[1:]:
    for x, y in zip(ref, data[n]):
        if (x[2], x[3]) != (y[2], y[3]):
            print(f"  {n}: case {x[0]} opcode {x[1]}: {names[0]}=({x[3]},{x[2]}) {n}=({y[3]},{y[2]})")
print("done")
