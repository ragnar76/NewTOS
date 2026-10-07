#!/usr/bin/env python3
"""mark-counts.py <TAG-stamped.log> <qemu trace> -- FS09: SD card commands per
injector segment. The stamped serial log (drive-m3.py --stamp) carries the host
epoch time of every serial line; the QEMU trace (-msg timestamp=on) the host
time of every SD event as an ISO-8601 UTC prefix
("2026-10-07T14:06:09.856118Z sdcard_normal_command ...", QEMU 11). A segment runs from
one "[m3] MARK <NAME>" line to the next; it is named after the mark that ENDS
it ("..A_IDLE" = everything up to the first mark: boot, desktop start)."""
import re, sys, bisect, collections, datetime
marks = []
for raw in open(sys.argv[1], 'rb'):
    t, _, line = raw.partition(b'\t')
    m = re.search(rb'\[m3\] MARK (\S+)', line)
    if m:
        marks.append((float(t), m.group(1).decode()))
times = [t for t, _ in marks]
seg = collections.defaultdict(collections.Counter)
EV = re.compile(rb'^(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d+)Z (sdcard_\w+)(.*)')

def epoch(iso):
    return datetime.datetime.fromisoformat(iso.decode()).replace(tzinfo=datetime.timezone.utc).timestamp()
for raw in open(sys.argv[2], 'rb'):
    m = EV.match(raw)
    if not m:
        continue
    i = bisect.bisect_left(times, epoch(m.group(1)))
    name = marks[i][1] if i < len(marks) else '(after last mark)'
    ev = m.group(2)
    if ev == b'sdcard_read_block':
        seg[name]['rblk'] += 1
    elif ev == b'sdcard_write_block':
        seg[name]['wblk'] += 1
    else:
        c = re.search(rb'\b(CMD\d+)\b', m.group(3))
        if c:
            seg[name][c.group(1).decode()] += 1
names = [n for _, n in marks] + ['(after last mark)']
print(f'{"segment ending at":22} {"CMD17":>6} {"CMD18":>6} {"rd cmds":>7} {"blk rd":>7} {"CMD24":>6} {"CMD25":>6} {"blk wr":>6}')
for n in names:
    k = seg.get(n)
    if k is None:
        continue
    print(f'{n:22} {k["CMD17"]:6} {k["CMD18"]:6} {k["CMD17"] + k["CMD18"]:7} {k["rblk"]:7} {k["CMD24"]:6} {k["CMD25"]:6} {k["wblk"]:6}')
