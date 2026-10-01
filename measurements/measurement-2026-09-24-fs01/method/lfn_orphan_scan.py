#!/usr/bin/env python3
"""Count the ORPHAN long-name slots of a raw MBR/FAT16 card image.

usage:  lfn_orphan_scan.py IMG [-v]

Read-only. Walks the root directory and every sub-directory reachable from it
and reports the slots with `attrib == 0x0F` that are still LIVE (first byte
neither 0xE5 nor 0x00) while the 8.3 entry they belong to is gone.

This is the strict form of `measurement-2026-09-23-desk24-desk25-desk26/
method/lfn_orphan_probe.py`, which counted a long-name slot as an orphan by
its 8.3 entry alone: on a card straight out of `tools/make-sd-image.sh` that
also counts the ALREADY-ERASED slots macOS leaves when it deletes its own
`._*` companions, which are free slots and bother nobody. Only a LIVE slot
whose 8.3 entry is erased is an orphan.

Exit status is the number of orphans (capped at 250), so a shell can branch.
"""
import struct
import sys

img = sys.argv[1]
verbose = '-v' in sys.argv[2:]
d = open(img, 'rb').read()

start = struct.unpack_from('<I', d, 0x1BE + 8)[0] * 512
bps = struct.unpack_from('<H', d, start + 11)[0]
spc = d[start + 13]
rsv = struct.unpack_from('<H', d, start + 14)[0]
nfat = d[start + 16]
rootent = struct.unpack_from('<H', d, start + 17)[0]
spf = struct.unpack_from('<H', d, start + 22)[0]
root = start + rsv * bps + nfat * spf * bps
rootsec = rootent * 32 // bps
data = root + rootsec * bps
fat = start + rsv * bps


def fat_entry(cl):
    return struct.unpack_from('<H', d, fat + cl * 2)[0]


def dir_sectors(cluster):
    """Partition offsets of the sectors of a directory."""
    if cluster == 0:
        return [root + i * bps for i in range(rootsec)]
    out = []
    seen = set()
    while 2 <= cluster < 0xFFF0 and cluster not in seen:
        seen.add(cluster)
        base = data + (cluster - 2) * spc * bps
        out += [base + i * bps for i in range(spc)]
        cluster = fat_entry(cluster)
    return out


def slots(cluster):
    out = []
    for sec in dir_sectors(cluster):
        for i in range(bps // 32):
            e = d[sec + i * 32:sec + i * 32 + 32]
            if e[0] == 0x00:
                return out
            out.append(e)
    return out


total = 0
erased_lfn = 0
live_lfn = 0
pending = [(0, 'C:\\')]
seen_dirs = set()
while pending:
    cluster, name = pending.pop(0)
    if cluster in seen_dirs:
        continue
    seen_dirs.add(cluster)
    s = slots(cluster)
    for i, e in enumerate(s):
        attr = e[11]
        if attr == 0x0F:
            if e[0] in (0xE5, 0x00):
                erased_lfn += 1
                continue
            live_lfn += 1
            j = i + 1
            while j < len(s) and s[j][11] == 0x0F:
                j += 1
            if j >= len(s) or s[j][0] == 0xE5:
                total += 1
                print(f"ORPHAN {name} slot {i:3d} ord={e[0]:#04x} chksum={e[13]:#04x}")
            continue
        if e[0] == 0xE5:
            continue
        if attr & 0x08:
            continue
        short = e[0:11].decode('latin1')
        if verbose:
            print(f"  live   {name} slot {i:3d} {short!r} attr={attr:#04x}")
        if attr & 0x10 and short[0] != '.':
            cl = struct.unpack_from('<H', e, 0x1A)[0]
            if cl >= 2:
                pending.append((cl, name + short.strip() + '\\'))

print(f"{img}: {total} orphan long-name slot(s) "
      f"({live_lfn} live, {erased_lfn} already erased)")
sys.exit(min(total, 250))
