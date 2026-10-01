#!/usr/bin/env python3
"""ARCH01: force the EL0-entry race open (scratch copy only).

Inserts a calibrated busy-wait between `msr elr_el1, {entry}` and the
`eret` in `enter_el0_with_resume_capture`, so that the 200 Hz system
tick (or any other IRQ) is certain to land inside the window the ticket
describes, instead of hitting it once in seven runs.

Usage: force_race_patch.py <tree>
"""
import sys

p = sys.argv[1] + '/kernel/src/arch/aarch64/user_entry.rs'
s = open(p).read()

ANCHOR = '''            "msr   elr_el1, {entry}",
'''
DELAY = '''            "msr   elr_el1, {entry}",

            // ARCH01-TEMP (scratch copy only): hold the window between the
            // `ELR_EL1` write and the `eret` open for ~16.7M loop iterations,
            // far longer than the 5 ms system-tick period, so the IRQ that the
            // ticket observed once in seven runs lands here on every launch.
            "mov   {tmp}, #0x1000000",
            "9:",
            "subs  {tmp}, {tmp}, #1",
            "b.ne  9b",

'''
assert s.count(ANCHOR) == 1, "anchor not unique"
s = s.replace(ANCHOR, DELAY, 1)
open(p, 'w').write(s)
print("patched", p)
