#!/usr/bin/env python3
"""c2p-loop-breakdown.py <hatari.log[.gz]> <symbol> -- per-instruction cycles of one
function from the 'profile addresses' dump, grouped by how often each
instruction ran (= which loop) and by instruction kind. Base for the
68030-assembler outline (RUNINFO 4)."""
import collections, gzip, re, sys
p, sym = sys.argv[1], sys.argv[2]
log = (gzip.open(p, "rt", errors="replace") if p.endswith(".gz") else open(p, errors="replace")).read().splitlines()
rx = re.compile(r"^([0-9a-f]{8}) (?:[0-9a-f]{4} ?)+\s+(\S+)\s+(.*?)\s+[0-9.]+% \((\d+), (\d+), (\d+), (\d+)\)")
cur = None; rows = []
for l in log:
    m = re.match(r"^(_\S+):$", l)
    if m:
        cur = m.group(1); continue
    if cur == sym:
        m = rx.match(l)
        if m:
            a, op, arg, n, c, im, dh = m.groups()
            rows.append((int(a, 16), op, arg, int(n), int(c), int(im), int(dh)))
tot = sum(r[4] for r in rows)
loops = collections.defaultdict(list)
for r in rows:
    loops[r[3]].append(r)
print(f"{sym}: {tot} cycles in {len(rows)} instructions")
for n, rs in sorted(loops.items(), key=lambda x: -sum(r[4] for r in x[1]))[:4]:
    c = sum(r[4] for r in rs); im = sum(r[5] for r in rs)
    lo = min(r[0] for r in rs); hi = max(r[0] for r in rs)
    print(f"\n-- executed {n} times: {len(rs)} instructions {lo:x}-{hi:x}, {c} cycles ({100 * c / tot:.1f}%), "
          f"{c / n:.1f} cycles and {im / n:.2f} i-cache misses per pass")
    k = collections.Counter()
    for a, op, arg, _, cy, _, _ in rs:
        if re.search(r"\(\$00,a\d,d\d\.l\)", arg) and op in ("move.l", "or.l"):
            kind = "table load (move.l/or.l (An,Dn.l))"
        elif op == "move.b":
            kind = "pixel byte load"
        elif op.startswith("move.w"):
            kind = "plane word store"
        else:
            kind = op
        k[kind] += cy
    for kind, cy in k.most_common():
        print(f"   {cy / n:7.1f} cyc/pass  {100 * cy / c:5.1f}%  {kind}")
