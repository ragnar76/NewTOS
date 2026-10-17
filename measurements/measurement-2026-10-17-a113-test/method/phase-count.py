#!/usr/bin/env python3
"""phase-count.py <serial-log> <script> <tag> <trigger> <pattern> -- number of serial lines matching <pattern> between the injected bytes
of consecutive marks of <script> (marks file), and before the first / after the last."""
import sys, re
log = open(sys.argv[1], errors='replace').read().split('\n'); scr, tag, trig, pat = sys.argv[2:6]
steps = []
for l in open(scr):
    l = l.split('#')[0].split()
    if l: steps.append((int(l[0]), len(l) - 1))
N = lambda t: sum(n for tk, n in steps if tk <= t)
marks = [l.split() for l in open(scr + '.marks')]
feeds = [i for i, l in enumerate(log) if re.search(r'\[%s\] %s feed' % (tag, re.escape(trig)), l)]
idx = [0] + [feeds[N(int(t)) - 1] for _, t in marks if N(int(t)) - 1 < len(feeds)] + [len(log)]
names = ['start'] + [m[0] for m in marks][:len(idx) - 2] + ['end']
tot = 0
for a, b, nm in zip(idx, idx[1:], names[1:]):
    c = sum(1 for l in log[a:b] if re.search(pat, l)); tot += c
    print("  %-28s %5d" % ('-> ' + nm, c))
print("  total", tot)
