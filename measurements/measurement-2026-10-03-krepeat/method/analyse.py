#!/usr/bin/env python3
"""analyse.py <serial.log> -- per hold: key words queued, repeat timing, words after the break."""
import re, sys
lines = open(sys.argv[1], errors='replace').read().replace('\r', '\n').splitlines()
def grab(pat):
    return [(int(m.group(2)), int(m.group(1), 16)) for l in lines for m in [re.search(pat, l)] if m]
feeds = grab(r'\[krp\] feed (0x[0-9a-f]+) at hz_200 (\d+)')
pushes = grab(r'\[krp\] push_rx (0x[0-9a-f]+) at hz_200 (\d+)')
pops = grab(r'\[krp\] rx_pop_key (0x[0-9a-f]+) at hz_200 (\d+)')
holds, mk = [], None
for hz, b in feeds:
    if b == 0x48: mk = hz
    if b == 0xC8: holds.append((mk, hz))
for i, (a, b) in enumerate(holds):
    nxt = holds[i + 1][0] if i + 1 < len(holds) else 1 << 32
    ps = [h for h, w in pushes if w == 0x4800 and a <= h <= b]
    after = [h for h, w in pushes if w == 0x4800 and b < h < nxt]
    gaps = sorted(set(y - x for x, y in zip(ps[1:], ps[2:])))
    first = (ps[1] - a) if len(ps) > 1 else None
    print(f"hold make@{a} break@{b} ({(b-a)*5} ms): words={len(ps)} = 1 make + {len(ps)-1} repeats; "
          f"first repeat {first} ticks after make; repeat gaps {gaps} ticks; "
          f"last word {(b-ps[-1])*5} ms before break; up-arrow words between break and next make / end of log: {len(after)}")
print("Return words:", len([1 for h, w in pushes if w == 0x1c0d]), "(4 makes injected)")
print("words pushed", len(pushes), "popped", len(pops))
for l in lines:
    if 'K06 put' in l and 'key 173' in l:
        print("  doom:", l.strip())
