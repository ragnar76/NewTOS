#!/usr/bin/env python3
"""sym.py <elf> <base> addr... -- map runtime addresses of a NewTOS program
(basepage <base>; ELF linked at basepage-relative addresses) to symbol+off,
via llvm-nm of the rust toolchain. An address below 0x100000 is taken as
already ELF-relative."""
import subprocess, sys, bisect, os
elf, base = sys.argv[1], int(sys.argv[2], 16)
nm = os.path.join(subprocess.check_output(['rustc', '--print', 'sysroot'], text=True).strip(),
                  'lib/rustlib/aarch64-apple-darwin/bin/llvm-nm')
syms = []
for l in subprocess.check_output([nm, '-n', elf], text=True).splitlines():
    p = l.split()
    if len(p) == 3 and p[1] in 'TtWw':
        syms.append((int(p[0], 16), p[2]))
syms.sort()
keys = [s[0] for s in syms]
for a in sys.argv[3:]:
    v = int(a, 16)
    r = v - base if v >= 0x100000 else v
    i = bisect.bisect_right(keys, r) - 1
    print(f"{a:>12} -> elf {r:#07x} {syms[i][1]}+{r - syms[i][0]:#x}" if i >= 0 else f"{a} -> ?")
