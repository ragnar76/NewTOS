#!/usr/bin/env python3
"""fs11-schedule.py <scratch-tree>

On top of inj-keys02-apply-instrumentation.py (applied first, scratch tree
only): replace the KRP injector's whole schedule with the key sequence the
FS11 probe (tests/fs11dev.c, part B) reads. The injector arms at the first
input pump with an EL0 program running -- the probe's first blocking
Fread(-1, 1) -- and feeds IKBD make/break codes (PC set 1, break = make|0x80,
5 ticks apart) every 20 ticks of hz_200 from +100:

  'a'                  Fread(-1, 1)
  'h' 'i' Return       Fread(-1, 80)
  'o' BS 'k' Return    Fread(0, 80)
  'a' 'b' Return       getchar() + fgets()
  'c' 'o' 'p' Return   fgets()
"""
import re, sys, pathlib
t = pathlib.Path(sys.argv[1])
he = t / "kernel/src/host_env.rs"
s = he.read_text()
m = re.search(r"    const SCRIPT: &\[\(u32, u8\)\] = &\[\n(.*?)\n    \];\n", s, re.S)
assert m, "krp SCRIPT anchor"
SC = {'a': 0x1E, 'b': 0x30, 'c': 0x2E, 'h': 0x23, 'i': 0x17, 'k': 0x25,
      'o': 0x18, 'p': 0x19, '\r': 0x1C, '\b': 0x0E}
keys = "a" + "hi\r" + "o\bk\r" + "ab\r" + "cop\r"
ent = []
at = 100
for ch in keys:
    ent.append((at, SC[ch])); ent.append((at + 5, SC[ch] | 0x80)); at += 20
body = "".join(f"        ({a}, {b:#04x}),\n" for a, b in ent)
s = s[:m.start()] + "    const SCRIPT: &[(u32, u8)] = &[\n" + body + "    ];\n" + s[m.end():]
he.write_text(s)
print("fs11 schedule:", len(ent), "entries, last at", ent[-1][0])
