#!/usr/bin/env python3
"""trace-counts.py <qemu-trace.log> [FSBENCH.LOG] -- count SD card commands and
blocks in a QEMU trace (-trace sdcard_normal_command -trace sdcard_read_block
-trace sdcard_write_block). With the FSBENCH.LOG of the same run, the payload
the bench asked for (L1 Lrwabs + L2 Fread + L3 fread, bytes) and the device
sectors read per KiB of payload."""
import re, sys, collections
cmds = collections.Counter(); rd = wr = 0
for line in open(sys.argv[1], errors='replace'):
    if line.startswith('sdcard_read_block'):
        rd += 1
    elif line.startswith('sdcard_write_block'):
        wr += 1
    elif line.startswith('sdcard_normal_command'):
        m = re.search(r'\b(CMD\d+)\b', line)
        if m:
            cmds[m.group(1)] += 1
for c in ('CMD17', 'CMD18', 'CMD24', 'CMD25', 'CMD12'):
    print(f'{c:6} {cmds[c]}')
print(f'read commands (CMD17+CMD18)   {cmds["CMD17"] + cmds["CMD18"]}')
print(f'blocks read  (sdcard_read_block)  {rd}')
print(f'blocks written (sdcard_write_block) {wr}')
if len(sys.argv) > 2:
    l1 = l2 = l3 = 0
    sect = None
    for line in open(sys.argv[2], errors='replace'):
        f = line.split()
        if sect == 'L1' and len(f) >= 8 and f[0] in ('10K', '100K', '1M', '10M'):
            l1 += int(f[1])
        if line.startswith('L1 '): sect = 'L1'
        elif line.startswith('L2 ') or line.startswith('L3 '): sect = line[:2]
        if line.startswith('# ') and len(f) >= 9 and f[1] in ('cold', 'warm') and f[2].endswith('.DAT'):
            l2 += int(f[8])
        if sect == 'L3' and len(f) >= 9 and f[0].endswith('.DAT') and f[2] in ('cold', 'warm'):
            l3 += int(f[1])
    tot = l1 + l2 + l3
    print(f'payload L1 {l1} B, L2 {l2} B, L3 {l3} B, total {tot} B = {tot / 1024:.0f} KiB')
    print(f'payload blocks (512 B)                 {tot // 512}')
    print(f'device blocks read per payload block   {rd / (tot / 512):.4f}  (1.0000 = no overhead)')
    print(f'device blocks read per KiB of payload  {rd / (tot / 1024):.4f}  (2.0000 = no overhead)')
    print(f'read commands per KiB of payload       {(cmds["CMD17"] + cmds["CMD18"]) / (tot / 1024):.4f}')
