#!/usr/bin/env python3
"""drawframe-breakdown.py <hatari.log>: split _DG_DrawFrame (c2p + scale + cpyfm inlined) by
loop (instructions grouped by execution count) and list the hottest TOS-ROM 256-byte pages."""
import re, sys, collections
import gzip
log = (gzip.open(sys.argv[1], "rt", errors="replace") if sys.argv[1].endswith(".gz") else open(sys.argv[1], errors="replace")).read().splitlines()
i1 = [i for i, l in enumerate(log) if l.startswith("> profile addresses 0x101a000")][0]
i2 = [i for i, l in enumerate(log) if l.startswith("> profile addresses 0xe00000")][0]
rx = re.compile(r"^([0-9a-f]{8}) (?:[0-9a-f]{4} ?)+\s+(\S.*?)\s+[0-9.]+% \((\d+), (\d+), (\d+), (\d+)\)\s*$")
cur = None; rows = []
for l in log[i1:i2]:
    m = re.match(r"^([^\s\[#>].*):$", l)
    if m and m.group(1).startswith("_"): cur = m.group(1)
    m = re.match(r"^([0-9a-f]{8}) .*% \((\d+), (\d+), (\d+), (\d+)\)\s*$", l)
    if m and cur == "_DG_DrawFrame":
        rows.append((int(m.group(1), 16), int(m.group(2)), int(m.group(3)), l))
tot = sum(r[2] for r in rows)
byc = collections.defaultdict(lambda: [0, 0, 0, []])
for a, n, c, l in rows:
    g = byc[n]; g[0] += 1; g[1] += c; g[2] += n; g[3].append(a)
print(f"_DG_DrawFrame: {tot} cycles, {len(rows)} instructions executed at least once")
print("exec-count  #insn  addr-range             cycles   share-of-DrawFrame")
for n, (k, c, ni, addrs) in sorted(byc.items(), key=lambda x: -x[1][1])[:8]:
    print(f"{n:10d} {k:6d}  {min(addrs):x}-{max(addrs):x} {c:12d}  {100*c/tot:5.1f}%")
pages = collections.defaultdict(lambda: [0, 0])
for l in log[i2 + 1:]:
    m = re.match(r"^([0-9a-f]{8}) .*% \((\d+), (\d+), (\d+), (\d+)\)\s*$", l)
    if m:
        p = int(m.group(1), 16) & ~0xff
        pages[p][0] += int(m.group(2)); pages[p][1] += int(m.group(3))
rt = sum(v[1] for v in pages.values())
print(f"TOS ROM: {rt} cycles; hottest 256-byte pages")
for p, (n, c) in sorted(pages.items(), key=lambda x: -x[1][1])[:8]:
    print(f"  {p:08x}  {n:11d} instr {c:12d} cyc  {100*c/rt:5.1f}% of ROM")
