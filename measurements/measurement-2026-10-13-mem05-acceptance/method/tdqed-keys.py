#!/usr/bin/env python3
"""tdqed-keys.py <scratch-tree>

On top of measurement-2026-10-04-keys02/method/apply-instrumentation.py
(applied first): replace the injector schedule by one F1 press 5 s after
the first EL0 program (TeraDesk) runs. TERADESK-QED.INF assigns F1 to
C:\\APPS\\QED\\QED.NAP (TeraDesk `fkey`), so TeraDesk Pexecs QED after its own
start-up Mshrink (ARCH03 E9: a child launch from a GEM app).
"""
import sys, pathlib
he = pathlib.Path(sys.argv[1]) / "kernel/src/host_env.rs"
s = he.read_text()
old = """        (400, 0x48), (500, 0x4D), (600, 0xCD), (600, 0xC8),
        (900, 0x1E), (920, 0x9E),
        (2000, 0x38), (2010, 0x2E), (2020, 0xAE), (2030, 0xB8),
"""
assert s.count(old) == 1, "schedule anchor"
s = s.replace(old, "        (1000, 0x3B), (1010, 0xBB),\n")
he.write_text(s)
print("tdqed schedule: F1 at +1000")
