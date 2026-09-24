#!/usr/bin/env python3
"""DESK22/DESK29-TEMP (scratch copy only): the one hook these runs need on top
of the standard injector scaffolding.

kernel/src/host_env.rs -- step the H02 injector inside
`console_read_key_blocking`'s spin as well. The injector normally only runs
from `pump_input`, which is the AES event loop's idle pass; while the desktop
sits in `console_pause_for_key` ("Press any key to return to the desktop")
the AES is not running at all, so without this hook no scripted keystroke
could ever end the pause. Copied verbatim from
measurement-2026-09-23-desk06/method/desk06_patch.py (piece 1). Nothing about
the product path changes -- it is the QEMU stand-in for a keyboard.

Applied to the scratch copy AFTER a79_patch_desk10.py / apply_injector.py /
a78_patch2.py / key_step_patch.py / probe_patch.py, never to the shared tree.
"""
import sys

scr = sys.argv[1]


def rep(path, a, b):
    s = open(path).read()
    if b in s:                      # idempotent: runone.sh re-applies this
        print("already patched", path)
        return
    assert s.count(a) == 1, (path, a[:70])
    open(path, 'w').write(s.replace(a, b, 1))
    print("patched", path)


rep(scr + '/kernel/src/host_env.rs',
    "            crate::drivers::usb::poll();\n\n            if let Some(w) = crate::console::rx_pop_key() {",
    "            crate::drivers::usb::poll();\n"
    "            h02_inject::step(); // DESK22-TEMP: keys for the console pause\n\n"
    "            if let Some(w) = crate::console::rx_pop_key() {")
