#!/usr/bin/env python3
"""swcost-analyze.py <serial-dir> <label> <mode>... -- ARCH03-4 switch cost.

Per mode, from <label>-<mode>-rpi4-asidperf-counters-stamped.log (kernel
with the SWCOST-TEMP counters): the cumulative [swcost] line last printed
before "ASIDPERF: A begin" and before "ASIDPERF: D end" bound the probe's
parts A+B+D; reported: installs and restores in between, mean CNTVCT ticks
per restore_boot_ttbr0 and per install_ttbr0 (incl. its memprot::sync),
in ticks and microseconds (CNTFRQ from the line). From the uninstrumented
<label>-<mode>-rpi4-asidperf-stamped.log: host wall-clock seconds of parts
A, B, D (drive.py --stamp). QEMU numbers only -- indicative.
Also the virt run <label>-<mode>-virt-asidstr-stamped.log (counters too).
"""
import re, sys, os
d, label, modes = sys.argv[1], sys.argv[2], sys.argv[3:]
SW = re.compile(rb'\[swcost\] installs=(\d+) restores=(\d+) restore_ticks=(\d+) install_ticks=(\d+) cntfrq=(\d+) asid gen=(\d+) rollovers=(\d+) assigned=(\d+)')
def lines(path):
    with open(path, 'rb') as f:
        for l in f:
            t, _, rest = l.partition(b'\t')
            yield float(t), rest
def counters(path, start_pat, end_pat):
    last = None; a = b = None
    for t, l in lines(path):
        m = SW.search(l)
        if m:
            last = tuple(int(x) for x in m.groups())
        if start_pat in l and a is None:
            a = last
        if end_pat in l and b is None:
            b = last
    return a, b
def walls(path, tag):
    ts = {}
    for t, l in lines(path):
        m = re.search(rb'%s: ([ABD]) (begin|end)' % tag, l)
        if m:
            ts[(m.group(1), m.group(2))] = t
    return {p: ts[(p, b'end')] - ts[(p, b'begin')] for p in (b'A', b'B', b'D') if (p, b'begin') in ts and (p, b'end') in ts}
print(f"{'mode':14} {'run':10} {'installs':>8} {'restores':>8} {'tick/restore':>12} {'us/restore':>10} {'tick/install':>12} {'us/install':>10} {'rollovers':>9} | wall A/B/D s (uninstrumented)")
for m in modes:
    for run, tag, sp, ep in (('rpi4', 'ASIDPERF', b'ASIDPERF: A begin', b'ASIDPERF: D end'),
                             ('virt', 'ASIDSTR', b'ASIDSTR: A begin', b'ASIDSTR: D end')):
        p = os.path.join(d, f'{label}-{m}-{run}-asidperf-counters-stamped.log' if run == 'rpi4' else f'{label}-{m}-virt-asidstr-stamped.log')
        if not os.path.exists(p):
            continue
        a, b = counters(p, sp, ep)
        if b and not a:
            a = (0, 0, 0, 0, b[4], 1, 0, 0)  # no line yet: counters start at 0
        if not a or not b or b[0] == a[0]:
            print(f'{m:14} {run:10} (no counter lines around the probe)'); continue
        ni, nr = b[0] - a[0], b[1] - a[1]
        tr, tin, f = b[2] - a[2], b[3] - a[3], b[4]
        w = ''
        if run == 'rpi4':
            wp = os.path.join(d, f'{label}-{m}-rpi4-asidperf-stamped.log')
            if os.path.exists(wp):
                ww = walls(wp, b'ASIDPERF')
                w = '/'.join(f'{ww.get(k, float("nan")):.2f}' for k in (b'A', b'B', b'D'))
        print(f'{m:14} {run:10} {ni:8} {nr:8} {tr/nr:12.1f} {tr/nr/f*1e6:10.2f} {tin/ni:12.1f} {tin/ni/f*1e6:10.2f} {b[6]-a[6]:9} | {w}')
