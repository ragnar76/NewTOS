#!/usr/bin/env python3
"""zvprof-sum.py PROFILE.TXT [first_seg [last_seg]] -- sum the ZVPROF segments
(measurement-2026-10-07-zview-perf) of a C:\\PROFILE.TXT: per-segment one-line
overview, then the summed op table (sorted by EL1 work), backend primitives,
and all LONG/GAP/PEXEC/CLICK events of the selected segments."""
import re, sys
from collections import defaultdict

txt = open(sys.argv[1], encoding="latin-1").read().replace("\r", "")
lo = int(sys.argv[2]) if len(sys.argv) > 2 else 1
hi = int(sys.argv[3]) if len(sys.argv) > 3 else 10**9
segs = re.split(r"(?m)^==== ZVPROF segment ", txt)[1:]
ops = defaultdict(lambda: [0, 0.0, 0.0, 0.0, 0.0, 0, 0])
prims = defaultdict(lambda: [0, 0, 0.0])
tot = defaultdict(float)
events = []
print("seg  t_start..t_end(ms)    span   EL0   EL1work  wait  memprot  gaps>100/>500  longest_gap  clicks(avg)  reason")
for s in segs:
    m = re.match(r"(\d+) : t=(\d+)\.\.(\d+) ms since boot, span (\d+) ms, closed by (.*) ====", s)
    if not m:
        continue
    n = int(m.group(1))
    if n < lo or n > hi:
        continue
    t0, t1, span, reason = int(m.group(2)), int(m.group(3)), int(m.group(4)), m.group(5)
    tm = re.search(r"time: EL0 (\d+) ms .*EL1 trap work (\d+) ms .*EL1 waiting in traps (\d+) ms \| rest \(outside traps\) (\d+) ms", s)
    mp = re.search(r"memprot sync \(with work\) n=(\d+) t=(\d+) ms max=(\d+) us, of which refresh n=(\d+) t=(\d+) ms over (\d+) pages", s)
    gp = re.search(r"gaps without pump \(pointer frozen\) >100ms (\d+) >250ms (\d+) >500ms (\d+) >1s (\d+) >2s (\d+) \| longest (\d+) ms \| sum of gaps>100ms (\d+) ms", s)
    ck = re.search(r"clicks \(press -> evnt_multi[^)]*\) n=(\d+) avg (\d+) ms max (\d+) ms", s)
    el0, work, wait, rest = (int(x) for x in tm.groups())
    for k, v in (("span", span), ("el0", el0), ("work", work), ("wait", wait), ("rest", rest),
                 ("mp", int(mp.group(2))), ("rf_pages", int(mp.group(6))), ("gap_sum", int(gp.group(7)))):
        tot[k] += v
    print(f"{n:>3}  {t0:>7}..{t1:<7} {span:>7} {el0:>5} {work:>8} {wait:>6} {int(mp.group(2)):>7}  "
          f"{gp.group(1):>4}/{gp.group(3):<4} {gp.group(6):>8} ms  {ck.group(1)}({ck.group(2)})  {reason}")
    for line in s.split("\n"):
        mo = re.match(r"\s+(gemdos|vdi|aes|bios|xbios|other)\s+(0x[0-9a-f]+)\s+(\S*)\s+(\d+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+(\d+)\s+(\d+)$", line)
        if mo:
            key = (mo.group(1), mo.group(2), mo.group(3))
            o = ops[key]
            o[0] += int(mo.group(4)); o[1] += float(mo.group(5)); o[2] += float(mo.group(6))
            o[3] = max(o[3], float(mo.group(7))); o[4] += float(mo.group(8)); o[5] += int(mo.group(9)); o[6] += int(mo.group(10))
            continue
        mo = re.match(r"\s+(\S.*?)\s+n=(\d+)\s+px/bytes=(\d+)\s+([\d.]+) ms$", line)
        if mo:
            p = prims[mo.group(1)]
            p[0] += int(mo.group(2)); p[1] += int(mo.group(3)); p[2] += float(mo.group(4))
            continue
        if re.match(r" t=\d+ (LONG|GAP|PEXEC|CLICK|EL0|PTERM)", line):
            events.append(f"[seg {n}]{line}")
print(f"\nsum: span {tot['span']:.0f} ms, EL0 {tot['el0']:.0f} ms, EL1 work {tot['work']:.0f} ms, EL1 waiting {tot['wait']:.0f} ms, "
      f"rest {tot['rest']:.0f} ms, memprot {tot['mp']:.0f} ms over {tot['rf_pages']:.0f} pages, pointer-frozen gaps>100ms {tot['gap_sum']:.0f} ms")
print("\nops summed, sorted by EL1 work:")
print("  class  op     name               n        work_ms   wait_ms  maxwork_ms  memprot_ms  pixels")
for k, o in sorted(ops.items(), key=lambda kv: -kv[1][1]):
    if o[1] < 0.05 and o[4] < 0.05:
        continue
    print(f"  {k[0]:<6} {k[1]:<6} {k[2]:<18} {o[0]:<8} {o[1]:>8.1f} {o[2]:>9.1f} {o[3]:>10.1f} {o[4]:>10.1f}  {o[5]}")
print("\nbackend primitives summed:")
for k, p in sorted(prims.items(), key=lambda kv: -kv[1][2]):
    print(f"  {k:<22} n={p[0]:<9} px/bytes={p[1]:<12} {p[2]:.1f} ms")
print("\nevents:")
for e in events:
    print(e)
