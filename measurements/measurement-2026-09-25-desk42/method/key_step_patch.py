#!/usr/bin/env python3
"""A87-TEMP (scratch copy only), after apply_injector.py + a78_patch2.py:
add `Step::Key(u8)` to the H02 injector — one raw IKBD key byte fed into
`crate::ikbd::feed` (make = scancode, break = scancode | 0x80), so a script
can press HELP (0x62) while the pointer rests in the bar row."""
import sys
p = sys.argv[1] + '/kernel/src/host_env.rs'; s = open(p).read()
def rep(a, b):
    global s
    assert s.count(a) == 1, a[:60]
    s = s.replace(a, b, 1)
rep("    pub enum Step { Mv(i8, i8), Dm(i8, i8), Dn, Up, Arm, Mark(&'static str), Pause }",
    "    pub enum Step { Mv(i8, i8), Dm(i8, i8), Dn, Up, Arm, Mark(&'static str), Pause, Key(u8) }")
rep("                Step::Mark(_) | Step::Pause => {}",
    "                Step::Key(b) => { crate::ikbd::feed(b); } // A87-TEMP: one raw IKBD key byte\n                Step::Mark(_) | Step::Pause => {}")
open(p, 'w').write(s)
print("patched", p)
