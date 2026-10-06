#!/usr/bin/env python3
"""symbolize-b2.py <serial.log> -- map every [b2] Malloc/Mxalloc line's ELR and
frame-pointer return addresses to symbols of the module that was running
(POPUP.NLD / EX_LIB.NLD of ../../measurement-2026-10-06-ldg-m3/bin, unchanged;
runtime VA = basepage + link VA, the NLDs are linked at 0x1000 above their
basepage, sdk/newtos.ld). Uses llvm-nm of the pinned Rust toolchain."""
import re, subprocess, sys, pathlib
HERE = pathlib.Path(__file__).resolve().parent
LIBS = HERE / "../../measurement-2026-10-06-ldg-m3/bin/LDGDEMO/LIBS"
sysroot = subprocess.run(["rustc", "--print", "sysroot"], capture_output=True, text=True).stdout.strip()
host = [l.split()[1] for l in subprocess.run(["rustc", "-vV"], capture_output=True, text=True).stdout.splitlines() if l.startswith("host:")][0]
NM = f"{sysroot}/lib/rustlib/{host}/bin/llvm-nm"
def syms(f):
    r = []
    for l in subprocess.run([NM, "-n", str(f)], capture_output=True, text=True).stdout.splitlines():
        p = l.split()
        if len(p) == 3 and p[1] in "TtWw":
            r.append((int(p[0], 16), p[2]))
    return r
def sym(r, a):
    best = None
    for v, n in r:
        if v <= a: best = (v, n)
    return f"{best[1]}+{a-best[0]:#x}" if best else hex(a)
mods = {}
pexec4 = []
for line in open(sys.argv[1], errors="replace"):
    m = re.search(r"\[b2\] Pexec mode=0x4 .* x2=(0x[0-9a-f]+)", line)
    if m: pexec4.append(int(m.group(1), 16))
order = ["EX_LIB.NLD", "POPUP.NLD"]   # ldg_open order of client.c
for i, b in enumerate(pexec4[:2]):
    mods[b] = (order[i], syms(LIBS / order[i]))
for line in open(sys.argv[1], errors="replace"):
    m = re.search(r"\[b2\] fn=(0x4[48]) .*-> (0x[0-9a-f]+) run=(0x[0-9a-f]+) .*elr=(0x[0-9a-f]+) .*ra=\[([^\]]*)\]", line)
    if not m: continue
    run = int(m.group(3), 16); ras = [int(x, 16) for x in m.group(5).split(",") if int(x.strip(), 16)]
    # symbolize against every module whose image could contain the address
    def name(a):
        cand = [b for b in mods if b <= a < b + 0x20000]
        if not cand: return hex(a)
        b = max(cand)   # the nearest basepage below the address
        n, r = mods[b]
        return f"{n}:{sym(r, a-b)}"
    print(f"{m.group(1)} -> {m.group(2)} run={m.group(3)} elr={name(int(m.group(4),16))}")
    print("    chain: " + " <- ".join(name(a) for a in ras))
