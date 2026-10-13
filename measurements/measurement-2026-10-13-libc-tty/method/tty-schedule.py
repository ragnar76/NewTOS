#!/usr/bin/env python3
"""tty-schedule.py <scratch-tree>

LIBC-TTY: copy of measurement-2026-10-13-fs11/method/fs11-schedule.py with only
the part-C keys. On top of inj-keys02-apply-instrumentation.py (applied first,
scratch tree only): replace the KRP injector's whole schedule with the key
sequence the probe (tests/ttyprobe.c, T4) reads. The injector arms at the
first input pump with an EL0 program running -- the probe's first blocking
read (getchar -> Fread(0, ...)) -- and feeds IKBD make/break codes (PC set 1,
break = make|0x80, 5 ticks apart) every 20 ticks of hz_200 from +100:

  'a' 'b' Return       getchar() + fgets()
  'c' 'o' 'p' Return   fgets()
"""
import re, sys, pathlib
t = pathlib.Path(sys.argv[1])
he = t / "kernel/src/host_env.rs"
s = he.read_text()
m = re.search(r"    const SCRIPT: &\[\(u32, u8\)\] = &\[\n(.*?)\n    \];\n", s, re.S)
assert m, "krp SCRIPT anchor"
SC = {'a': 0x1E, 'b': 0x30, 'c': 0x2E, 'o': 0x18, 'p': 0x19, '\r': 0x1C}
keys = "ab\r" + "cop\r"
ent = []
at = 100
for ch in keys:
    ent.append((at, SC[ch])); ent.append((at + 5, SC[ch] | 0x80)); at += 20
body = "".join(f"        ({a}, {b:#04x}),\n" for a, b in ent)
s = s[:m.start()] + "    const SCRIPT: &[(u32, u8)] = &[\n" + body + "    ];\n" + s[m.end():]
he.write_text(s)
print("tty schedule:", len(ent), "entries, last at", ent[-1][0])
