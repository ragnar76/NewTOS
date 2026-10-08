#!/usr/bin/env python3
"""stretches.py -- INPUT01 summary over the six evidence logs (base = ofam-2026
+ injector + trap profiler, new = INPUT01 + the same): within each script's
continuous-movement window, the number of stretches without any input-pump
step of >= 100 ms (20 ticks) and >= 50 ms (10 ticks), and the longest one."""
import re, os, sys
here = os.path.dirname(os.path.abspath(__file__))
st = re.compile(r'\[m3\] (?:MARK \S+ )?step (\d+) at \+(\d+) sched (\d+) irq=(\d)')
def stretches(path, lo, hi):
    ats = []
    for ln in open(path, 'rb').read().decode('latin-1').splitlines():
        m = st.search(ln)
        if m and lo <= int(m.group(3)) <= hi:
            ats.append(int(m.group(2)))
    g = [b - a for a, b in zip(ats, ats[1:])]
    return sum(1 for x in g if x >= 20), sum(1 for x in g if x >= 10), max(g)
for t, lo, hi, what in [('zvmove', 886, 2886, 'zView start from EmuDESK (10 codecs, Pexec 3/4 + Mshrink)'),
                        ('photomove', 8886, 10886, 'zView browser thumbnails of 8 photos 2048x1536'),
                        ('jpgmove', 206, 4206, 'zView opens BIG.JPG 2.2 MB')]:
    print(what)
    for m in ('base', 'new'):
        a, b, c = stretches(os.path.join(here, '..', 'serial', f'{m}-{t}-serial.log'), lo, hi)
        print(f'  {m:4s}: stretches >= 100 ms: {a:2d}, >= 50 ms: {b:2d}, longest {c * 5} ms')
