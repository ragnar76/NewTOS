#!/usr/bin/env python3
"""segtrace.py <serial-log> <tag> <name:N> ... -- print the [d48] AES trace lines of a run, segmented by the injector
feed count: segment <name>:N holds the [d48] lines logged once feed N_prev+1 .. N have been fed (the AES calls the
desktop made in reaction to the action ending at feed N, until the next action starts). Lines before the first feed = 'boot'."""
import re, sys
log, tag = sys.argv[1], sys.argv[2]
marks = [(m.split(':')[0], int(m.split(':')[1])) for m in sys.argv[3:]]
feeds = 0; seg = 'boot'; out = {}; order = ['boot']
bounds = [(n, name) for name, n in marks]
for line in open(log, errors='replace'):
    if re.search(r'\[%s\] desktop feed' % re.escape(tag), line):
        feeds += 1
        # segment = first mark whose N >= feeds
        seg = next((name for n, name in bounds if feeds <= n), 'tail')
        if seg not in order: order.append(seg)
        continue
    if '[d48]' in line:
        out.setdefault(seg, []).append(line.strip())
for s in order:
    L = out.get(s, [])
    # form_dial(FMD_FINISH, 0,0,0,0, 0,…): little rect 0 and big rect x 0 = redraw_desktop()'s whole-screen call
    # (int_in[5] is the big rect's x; a dialog's end_dialog has its own x there).
    fd = sum(1 for l in L if 'op=51 in=3,0,0,0,0,0' in l); dl = sum(1 for l in L if 'op=51 in=3,' in l) - fd
    mb = sum(1 for l in L if 'op=30 ' in l)
    print('== %s: %d AES lines, whole-screen form_dial(FMD_FINISH)=%d, dialog FMD_FINISH=%d, menu_bar=%d' % (s, len(L), fd, dl, mb))
    for l in L: print('   ', l)
