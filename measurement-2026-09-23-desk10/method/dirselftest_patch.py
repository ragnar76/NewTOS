#!/usr/bin/env python3
"""DESK10-TEMP (scratch copy only): call the unwired `desk::dir::selftest()`
once, right before the desktop's event loop, so the mutating file operations
(Dcreate / Fcreate+Fwrite / copy / Frename / recursive delete / count) run
headless in QEMU and print a PASS/FAIL line each. The product tree is not
touched; `dir::selftest` itself is unchanged."""
import sys
p = sys.argv[1] + '/kernel/src/desk/mod.rs'
s = open(p).read()
a = "    main::event_loop();"
assert s.count(a) == 1, a
s = s.replace(a, "    dir::selftest(); // DESK10-TEMP: file-op backend proof\n" + a, 1)
open(p, 'w').write(s)
print("patched", p)
