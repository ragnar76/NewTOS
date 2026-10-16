#!/usr/bin/env python3
# (PROC06: copied from measurement-2026-10-16-t16-desk60/method; scratch /Users/ingouhlemann/.claude/jobs/a6cc309c/tmp/proc06, kernel id p06; changes listed in RUNINFO.TXT.)
# (T16+DESK60: copied unchanged from measurement-2026-10-16-desk58/method.)
"""fx.py <script> <tick>... -- for each tick: the cumulative number of injected bytes through that script step
(= the N of the injector's N-th '[tag] <trigger> feed' serial line, one line per byte)."""
import sys
steps = []
for l in open(sys.argv[1]):
    l = l.split('#')[0].split()
    if l: steps.append((int(l[0]), len(l) - 1))
out = []
for t in map(int, sys.argv[2:]):
    out.append(str(sum(n for tk, n in steps if tk <= t)))
print(" ".join(out))
