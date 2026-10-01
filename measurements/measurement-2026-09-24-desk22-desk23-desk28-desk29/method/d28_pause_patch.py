#!/usr/bin/env python3
"""DESK28-TEMP (scratch copy only): hold the \\AUTO\\ text screen for 4 s.

The AUTO phase and the desktop build are a few hundred milliseconds apart, and
QEMU flushes its `-serial file:` log in chunks, so by the time the QMP driver
sees `[auto] N AUTO program(s) run` in the log the desktop has long since been
painted over the text. This inserts one `delay_ms(4000)` between
`launch::run_auto_folder()` and the AES session loop, so the text screen the
AUTO programs wrote stands long enough to be photographed. Nothing else is
changed; the product path is untouched and the delay exists only in the
scratch copy used for the DESK28 pictures.
"""
import sys

scr = sys.argv[1]
p = scr + '/kernel/src/lib.rs'
s = open(p).read()
a = "    launch::run_auto_folder();\n"
b = ("    launch::run_auto_folder();\n"
     "    crate::arch::aarch64::delay_ms(4000); // DESK28-TEMP: hold the AUTO text screen\n")
if b in s:
    print("already patched", p)
else:
    assert s.count(a) == 1, a
    open(p, 'w').write(s.replace(a, b, 1))
    print("patched", p)
