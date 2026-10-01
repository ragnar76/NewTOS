#!/usr/bin/env python3
"""ACC02/ACC03/ACC04 (scratch copy only), applied after a78_patch2.py:
arm the injector on the DESKTOP only.

a78_patch2.py picks the script from whichever context armed the injector
first -- an EL0 program (SCRIPT_TERA) or the desktop (SCRIPT_DESK). That
heuristic breaks here for a reason that is the whole point of this batch: a
desk accessory is an EL0 process that runs BEFORE the desktop (EmuTOS
`geminit.c:736-737`), so it arms the injector and every later step is taken
from the empty SCRIPT_TERA. The accessory's own first run is not something a
script should drive anyway -- it happens before there is a screen to click on
-- so the gate becomes simply "the desktop is up".
"""
import sys
p = sys.argv[1] + '/kernel/src/host_env.rs'; s = open(p).read()
def rep(a, b):
    global s
    assert s.count(a) == 1, a[:70]
    s = s.replace(a, b, 1)
rep("        let el0 = !tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null();\n"
    "        if !el0 && !DESK_UP.load(Ordering::Acquire) {\n            return; // A46: while an EL0 program runs, or once our desktop is up\n        }\n"
    "        if ARMED_BY_EL0.load(Ordering::Relaxed) == 0 { ARMED_BY_EL0.store(if el0 { 1 } else { 2 }, Ordering::Relaxed); }\n"
    "        let script: &[(u32, Step)] = if ARMED_BY_EL0.load(Ordering::Relaxed) == 1 { SCRIPT_TERA } else { SCRIPT_DESK };\n",
    "        if !DESK_UP.load(Ordering::Acquire) {\n            return; // ACC-TEMP: only once our desktop is up (accessories run before it)\n        }\n"
    "        if ARMED_BY_EL0.load(Ordering::Relaxed) == 0 { ARMED_BY_EL0.store(2, Ordering::Relaxed); }\n"
    "        let script: &[(u32, Step)] = SCRIPT_DESK;\n")
open(p, 'w').write(s)
print("patched", p)
