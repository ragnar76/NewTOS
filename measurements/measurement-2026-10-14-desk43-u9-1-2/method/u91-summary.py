#!/usr/bin/env python3
"""u91-summary.py <serial-dir> -- DESK43 U9-1/U9-2: the stack and latency lines of every instrumented run
(serial/k1*-serial.log = base tree, i.e. today's EL1 desktop; serial/k2*-serial.log = U9-2 tree, EL1 and the
EL0 NEWDESK), one block per run, then the aggregates the RUNINFO quotes.

Stack: the run's maximum `depth` per kind (el1 = built-in desktop on the kernel stack, el0 = the shell child's
crt0 stack). HELLOGEM paints its own stack with its own word (hello-gem/src/lib.rs stack_paint), so the el0
depth of a HELLOGEM child reads 65536 -- an artefact, excluded (marked).
Latency (aggregates without the self-test runs k*s and the panic proof k2p): `A->U` (sh_ldapp entry -> desktop up), `A->B` (-> drop to EL0, the load), `B->U`, `X->U` (the previous
shell child's Pterm -> desktop up); n = sh_ldapp count (n=1: after boot).
"""
import os
import re
import statistics
import sys

d = sys.argv[1]
LAT = re.compile(r'\[u91\] lat n=(\d+) (\S+) A->U=(-?\d+) us \(A->B=(-?\d+) B->U=(-?\d+)\) X->U=(-?\d+)')
STK = re.compile(r'\[u91\] stk (el[01]) (\w+) (?:run|slot)=(\d+) depth=(\d+) B')
A = re.compile(r"\[u91\] A n=(\d+) nextapp=(\d+) cmd='([^']*)'")


def ms(v):
    return f'{int(v) / 1000:.1f}' if int(v) >= 0 else '-'


agg = {}
for f in sorted(os.listdir(d)):
    if not re.match(r'k[12]', f) or not f.endswith('-serial.log'):
        continue
    run = f[:-len('-serial.log')]
    text = open(os.path.join(d, f), 'rb').read().decode('latin-1').replace('\r', '')
    cmds = {int(m.group(1)): m.group(3) for m in A.finditer(text)}
    print(f'== {run}')
    child_cmd = None
    mx = {}
    for line in text.splitlines():
        m = A.search(line)
        if m:
            child_cmd = m.group(3)
        m = STK.search(line)
        if m:
            kind, ev, _, dep = m.group(1), m.group(2), m.group(3), int(m.group(4))
            if kind == 'el0' and 'HELLOGEM' in (child_cmd or ''):
                continue
            mx[kind] = max(mx.get(kind, 0), dep)
        m = LAT.search(line)
        if m:
            n, kind, au, ab, bu, xu = m.groups()
            cmd = cmds.get(int(n), '?')
            print(f'   lat n={n:>2} {kind:<11} {cmd:<16} A->U {ms(au):>7} ms  A->B {ms(ab):>6}  B->U {ms(bu):>6}  X->U {ms(xu):>7}')
            # the self-test runs (a dialog waits for the injector's Return before "up") and the panic proof
            # are not latency samples
            if not re.match(r'k[12][sp]', run):
                key = (run[:2], kind, 'boot' if n == '1' else 'after-program', cmd)
                agg.setdefault(key, []).append((int(au), int(ab), int(bu), int(xu)))
    for kind, v in sorted(mx.items()):
        print(f'   stack {kind} max depth {v} B ({v / 1024:.1f} KiB)')
    agg.setdefault(('stack', run), mx)

print('\n== aggregates (QEMU raspi4b, host macOS; not representative for the Pi -- prep §5.2)')
for key, vals in sorted((k, v) for k, v in agg.items() if k[0] != 'stack'):
    if 'HELLOGEM' in key[3]:
        continue
    def st(i):
        xs = [v[i] for v in vals if v[i] >= 0]
        if not xs:
            return '-'
        return f'median {statistics.median(xs) / 1000:.1f} min {min(xs) / 1000:.1f} max {max(xs) / 1000:.1f} ms'
    print(f'{key[0]} {key[1]:<11} {key[2]:<13} {key[3]:<16} n={len(vals):>2}  A->U {st(0)} | A->B {st(1)} | B->U {st(2)} | X->U {st(3)}')
