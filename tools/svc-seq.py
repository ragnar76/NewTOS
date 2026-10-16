#!/usr/bin/env python3
"""svc-seq.py — turn a C:\\SVCSEQ.LOG into a diffable call sequence.

The log is written by the kernel's `svc` tracer in sequence mode (kernel cargo
feature `svc-seq-trace`, `kernel/src/dbg_qedtrace.rs` module doc "Sequence
mode"; DESK43 U5a, prerequisite of guard G2 in
docs/analysis/desk43-u5-u10-review-2026-10-14.md §4). Every completed call is
one line with a caller column (`K` = kernel space, an `svc` from EL1 -- until
DESK43 U10 that was the built-in desktop; `U<slot>` = an EL0 process, since U9
the desktop `NEWDESK` among them), followed by indented detail lines for some
calls; `>> evnt_multi entered (#n)` marks each event-loop entry.

Output: one line per call, `<caller> <CLASS> <op> <name> [-> result]`, plus
the `>> evnt_multi` marks, detail lines dropped unless --details.

  tools/svc-seq.py SVCSEQ.LOG                    # every caller
  tools/svc-seq.py --caller K SVCSEQ.LOG         # only EL1 callers
  tools/svc-seq.py --caller U3 --no-vdi LOG      # one EL0 process, no VDI
  tools/svc-seq.py --no-caller --no-results LOG  # the shape only: for a diff
                                                 # between two link targets of
                                                 # the same desktop (K vs U<n>)
  tools/svc-seq.py --details LOG                 # keep decoded detail lines

The equality proof of U9 was (logs of a kernel with the built-in desktop,
which U10 removed)
  diff <(svc-seq.py --caller K --no-caller A.LOG) <(svc-seq.py --caller U5 --no-caller B.LOG)
"""
import argparse
import re
import sys

CALL = re.compile(r'^(K|U\S+)\s+(GEMDOS|BIOS|XBIOS|AES|VDI|GEM)\s+(.*)$')
WAIT = re.compile(r'^(K|U\S+)\s+>> evnt_multi entered')


def parse_call(cls, rest):
    """(op, name, result) from the part after the class column."""
    if cls == 'GEMDOS':
        m = re.match(r'#1\s+fn=(0x[0-9a-f]+)\s+(\S*)\s*-> (\S+)', rest)
        return (m.group(1), m.group(2) or '?', m.group(3)) if m else (rest, '', '')
    if cls in ('BIOS', 'XBIOS'):
        m = re.match(r'#\d+\s+fn=(0x[0-9a-f]+)\s*-> (\S+)', rest)
        return (m.group(1), '', m.group(2)) if m else (rest, '', '')
    if cls == 'AES':
        m = re.match(r'op=(-?\d+)\s+(\S+)\s+-> (\S+)', rest)
        return (m.group(1), m.group(2), m.group(3)) if m else (rest, '', '')
    if cls == 'VDI':
        m = re.match(r'op=(-?\d+)\s+(\S+)', rest)
        return (m.group(1), m.group(2), '') if m else (rest, '', '')
    return (rest, '', '')


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('log')
    ap.add_argument('--caller', help='keep only this caller column (K, U3, ...)')
    ap.add_argument('--no-caller', action='store_true', help='drop the caller column')
    ap.add_argument('--no-results', action='store_true', help='drop "-> result"')
    ap.add_argument('--no-vdi', action='store_true', help='drop VDI calls')
    ap.add_argument('--details', action='store_true', help='keep indented detail lines')
    a = ap.parse_args()

    with open(a.log, 'rb') as f:
        text = f.read().decode('latin-1')
    keep_details = False
    calls = 0
    for line in text.splitlines():
        m = WAIT.match(line)
        if m:
            keep_details = False
            if a.caller and m.group(1) != a.caller:
                continue
            print(('' if a.no_caller else m.group(1) + ' ') + '>> evnt_multi')
            continue
        m = CALL.match(line)
        if not m:
            if a.details and keep_details and line.startswith(' '):
                print('    ' + line.strip())
            continue
        who, cls, rest = m.groups()
        keep_details = False
        if a.caller and who != a.caller:
            continue
        if a.no_vdi and cls == 'VDI':
            continue
        op, name, res = parse_call(cls, rest)
        out = [] if a.no_caller else [who]
        out += [cls, op, name]
        if res and not a.no_results:
            out += ['->', res]
        print(' '.join(x for x in out if x != ''))
        calls += 1
        keep_details = a.details
    if calls == 0:
        print('svc-seq: no call lines found in %s (was the kernel built with '
              '--features svc-seq-trace?)' % a.log, file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
