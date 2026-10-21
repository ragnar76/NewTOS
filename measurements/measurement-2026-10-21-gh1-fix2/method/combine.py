#!/usr/bin/env python3
"""combine.py OUT SECTION... -- concatenate injector scripts. SECTION = <script>:<offset>[:<marks>] ; marks = name=tick,name=tick (ticks as in the section, before the offset).
Output OUT (script) + OUT.marks (name tick, ordered). A script is raw 'TICK 0xNN ...' lines (comments ok)."""
import sys
out = []; marks = []
for sec in sys.argv[2:]:
    p = sec.split(':'); f, off = p[0], int(p[1])
    for l in open(f):
        c = l.split('#')[0].split()
        if c: out.append((int(c[0]) + off, ' '.join(c[1:])))
    if len(p) > 2 and p[2] == '@':
        for l in open(f + '.marks'):
            n, t = l.split(); marks.append((n, int(t) + off))
    elif len(p) > 2 and p[2]:
        for m in p[2].split(','):
            n, t = m.split('='); marks.append((n, int(t) + off))
out.sort(key=lambda x: x[0])
open(sys.argv[1], 'w').write(''.join('%d %s\n' % o for o in out))
open(sys.argv[1] + '.marks', 'w').write(''.join('%s %d\n' % m for m in marks))
print(sys.argv[1], len(out), 'lines, last tick', out[-1][0], len(marks), 'marks')
