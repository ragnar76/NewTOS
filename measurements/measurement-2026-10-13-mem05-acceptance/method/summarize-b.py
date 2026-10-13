#!/usr/bin/env python3
"""summarize-b.py <serial-dir> <label> -- MEM05 part B: per mode, the MDSTEP/MDTIME/MDPROBE
series from the --stamp logs (host seconds since QEMU start; QEMU timings indicative only):
blocks -> MDs taken by malloc (OS view, MDSTEP), mintlib hunks (program view), time since the
first step line; the MDPROBE start/end probe times and the series time to 3 000 000."""
import re, sys, os
d, L = sys.argv[1:3]
def rows(path):
    out = []
    for line in open(path, 'rb').read().decode('latin-1').splitlines():
        m = re.match(r'\s*([0-9.]+)\t(.*)', line)
        if m:
            out.append((float(m.group(1)), m.group(2)))
    return out
for mode in ('on', 'off', 'strict', 'on-asid'):
    for run in ('mdstep', 'mdtime', 'mdprobe'):
        p = os.path.join(d, f'{L}-{mode}-rpi4-{run}-stamped.log')
        if not os.path.exists(p):
            continue
        r = rows(p)
        t_start = next((t for t, s in r if 'sh_ldapp: starting' in s), None)
        t_main = next((t for t, s in r if 'more blocks allowed at start' in s), None)
        steps = []
        for i, (t, s) in enumerate(r):
            m = re.search(r'\[(MDSTEP|MDPROBE)\] (?:step )?blocks=(\d+) ', s)
            if m:
                mds = re.search(r'MDs taken by malloc=(\d+)', s)
                hk = None
                if i + 1 < len(r):
                    h = re.search(r'mintlib hunks=(\d+)', r[i + 1][1])
                    hk = h.group(1) if h else None
                steps.append((int(m.group(2)), t, mds.group(1) if mds else '-', hk or '-'))
        end = next(((t, s) for t, s in r if '[MDPROBE] end' in s), None)
        done = next((t for t, s in r if re.search(r'(MDSTEP|MDPROBE): DONE', s)), None)
        print(f'{mode:8} {run:8} start->main {t_main - t_start:6.2f} s' if t_main and t_start else f'{mode} {run} ?')
        if steps:
            t0 = steps[0][1]
            print('   blocks     MDs  hunks  t(s since 100)')
            for b, t, mds, hk in steps:
                print(f'   {b:>9} {mds:>5} {hk:>6} {t - t0:9.3f}')
        if end:
            m = re.search(r'allowed=(\d+) taken by malloc=(\d+)', end[1])
            last = steps[-1][1] if steps else 0
            print(f'   end probe: more allowed={m.group(1)} taken by malloc={m.group(2)}; end-probe time {end[0] - last:.2f} s')
        if done and steps:
            print(f'   main start -> DONE {done - t_main:.2f} s; first step -> last step {steps[-1][1] - steps[0][1]:.3f} s')
