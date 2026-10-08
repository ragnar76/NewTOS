#!/usr/bin/env python3
"""lag.py <serial.log> [FROM TO] -- INPUT01 evidence: the injector's step lag.

Each "[m3] step i at +A sched S irq=I" line (apply-injector-input01.py) says
the step scheduled for tick S ran at tick A, inside the IRQ handler (I=1) or
inside a trap's input pump (I=0). Within FROM..TO (scheduled ticks, default
816..2816 = the continuous-movement window of zv-start-move.actions) it prints
the lag distribution (A-S, in ticks of 5 ms), the longest lags with their
"[i01] trap" lines in between (apply-trapprof-input01.py), and the longest
stretch of hz_200 without any step at all (= the pointer stood still although
the script kept moving it).
"""
import re, sys
log = open(sys.argv[1], 'rb').read().decode('latin-1').splitlines()
lo, hi = (int(sys.argv[2]), int(sys.argv[3])) if len(sys.argv) > 3 else (886, 2886)
step = re.compile(r'\[m3\] (?:MARK \S+ )?step (\d+) at \+(\d+) sched (\d+) irq=(\d)')
trap = re.compile(r'\[i01\] trap (\d+) us from el(\d) ec (\S+) svc (\S+) fn (\S+) x0 (\S+) hz (\d+)')
armed = re.compile(r'injector armed at hz_200 (\d+)')
t0 = None; steps = []; traps = []
for ln in log:
    m = armed.search(ln)
    if m: t0 = int(m.group(1))
    m = step.search(ln)
    if m: steps.append((int(m.group(1)), int(m.group(2)), int(m.group(3)), int(m.group(4))))
    m = trap.search(ln)
    if m and t0 is not None:
        traps.append((int(m.group(7)) - t0, int(m.group(1)), m.group(2), m.group(4), m.group(5)))
w = [s for s in steps if lo <= s[2] <= hi]
if not w:
    sys.exit("no steps in window")
lags = sorted(a - s for _, a, s, _ in w)
n = len(lags)
print(f"steps in window {lo}..{hi}: {n}, ran in IRQ: {sum(1 for x in w if x[3])}, in trap pump: {sum(1 for x in w if not x[3])}")
print(f"lag ticks: min {lags[0]} median {lags[n//2]} p95 {lags[int(n*0.95)]} max {lags[-1]}  (1 tick = 5 ms)")
ats = [a for _, a, _, _ in w]
gaps = sorted(((b - a, a, b) for a, b in zip(ats, ats[1:])), reverse=True)[:5]
print("longest stretches without a step (ticks, from, to):", [(g, a, b) for g, a, b in gaps])
big = sorted(w, key=lambda x: x[2] - x[1])[:5]
for i, a, s, irq in big:
    print(f"  step {i}: sched {s} ran {a} lag {a - s} ticks ({(a - s) * 5} ms) irq={irq}")
tw = [t for t in traps if lo - 10 <= t[0] <= hi + 400]
if tw:
    tw.sort(key=lambda t: -t[1])
    print(f"traps >= 5 ms starting in the window: {len(tw)}, sum {sum(t[1] for t in tw)/1000:.1f} ms, longest:")
    for at, us, el, svc, fn in tw[:8]:
        print(f"  starts +{at}: {us/1000:.1f} ms from el{el} svc {svc} fn {fn}")
# composition of the longest stretch: the >= 5 ms traps that ran inside it.
# The "hz" a trap line carries is the trap's START tick for a trap that never
# opens IRQs: _hz_200 is advanced only by the timer interrupt, which is masked
# for the whole trap (the ticks it covered are credited on the interrupt right
# after it). A trap that reaches an input pump (an AES wait) opens IRQs there,
# so its hz is the tick of its last pump window -- its masked stretch before
# that window then shows as "the rest" of a stretch.
g, a, b = gaps[0]
inside = [t for t in traps if a <= t[0] < b]
tsum = sum(t[1] for t in inside) / 1000
byfn = {}
for at, us, el, svc, fn in inside:
    k = f"el{el} svc {svc} fn {fn if svc == '0x1' else '-'}"
    c, s, m = byfn.get(k, (0, 0, 0))
    byfn[k] = (c + 1, s + us, max(m, us))
print(f"longest stretch {a}..{b} = {g * 5} ms: traps >= 5 ms inside: {len(inside)}, {tsum:.0f} ms "
      f"({100 * tsum / (g * 5):.0f} %); the rest ({g * 5 - tsum:.0f} ms) = EL0 time + traps < 5 ms")
for k, (c, s, m) in sorted(byfn.items(), key=lambda kv: -kv[1][1]):
    print(f"  {k}: {c} traps, {s / 1000:.0f} ms, longest {m / 1000:.1f} ms")
