#!/usr/bin/env python3
"""phase-counts.py <merged log> -- FS09: SD card commands per FSBENCH phase.

The merged log (run-fsbench-trace.sh) holds the serial output and the QEMU SD
trace in the order the vCPU produced them. FSBENCH prints a progress line
before every timed region ("L1 cold Rwabs ...", "L2 warm K010_1.DAT", "L3 cold
..."), so each trace line belongs to the phase of the last progress line seen.
'boot' = before the first one (kernel boot, AUTO start, FSBENCH's own load and
Getbpb); 'end' = after "summary:" (log written to C:, pages shown, desktop).
Counted per phase: CMD17, CMD18 (read commands), blocks read, CMD24/CMD25,
blocks written."""
import re, sys, collections
PROG = re.compile(rb'\b(L[123]) (cold|warm)\b')
order = ['boot']
c = collections.defaultdict(collections.Counter)
phase = 'boot'
for raw in open(sys.argv[1], 'rb'):
    if raw.startswith(b'sdcard_'):
        if raw.startswith(b'sdcard_read_block'):
            c[phase]['rblk'] += 1
        elif raw.startswith(b'sdcard_write_block'):
            c[phase]['wblk'] += 1
        else:
            m = re.search(rb'\b(CMD\d+)\b', raw)
            if m:
                c[phase][m.group(1).decode()] += 1
        continue
    if b'summary:' in raw and phase != 'end':
        phase = 'end'; order.append(phase); continue
    m = PROG.search(raw)
    if m and phase != 'end':
        p = f'{m.group(1).decode()} {m.group(2).decode()}'
        if p != phase:
            phase = p
            if p not in order:
                order.append(p)
print(f'{"phase":10} {"CMD17":>8} {"CMD18":>8} {"rd cmds":>8} {"blk read":>9} {"CMD24":>6} {"CMD25":>6} {"blk wr":>7}')
tot = collections.Counter()
for p in order:
    k = c[p]; tot.update(k)
    print(f'{p:10} {k["CMD17"]:8} {k["CMD18"]:8} {k["CMD17"] + k["CMD18"]:8} {k["rblk"]:9} {k["CMD24"]:6} {k["CMD25"]:6} {k["wblk"]:7}')
for pas in ('cold', 'warm'):
    k = collections.Counter()
    for p in order:
        if p.endswith(pas):
            k.update(c[p])
    print(f'{"L1-3 " + pas:10} {k["CMD17"]:8} {k["CMD18"]:8} {k["CMD17"] + k["CMD18"]:8} {k["rblk"]:9}')
print(f'{"total":10} {tot["CMD17"]:8} {tot["CMD18"]:8} {tot["CMD17"] + tot["CMD18"]:8} {tot["rblk"]:9} {tot["CMD24"]:6} {tot["CMD25"]:6} {tot["wblk"]:7}')
