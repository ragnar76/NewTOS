#!/usr/bin/env python3
"""T10 workaround (scratch copy only), applied after a78_patch2.py.

a78_patch2.py's dual arming decides which script to run from what happens to be
the RUNNING process at the very first injector tick: an EL0 process means
"TeraDesk", anything else means "our desktop".  Since the accessories landed
(ACC02-ACC04) the boot starts EL0 accessories BEFORE the desktop appears, so
the first tick always sees an EL0 process, the empty SCRIPT_TERA is chosen, and
the run completes with screendumps on which nothing ever happened.  That is
TASK/T10.md, found in PORT01.

This measurement drives our own desktop only, so — exactly as
measurement-2026-09-25-port01/method/port01_patch.py did — the dual arming is
replaced by the one case needed here: step only once "[desk] desktop up" has
set DESK_UP, and always run SCRIPT_DESK.  The clock therefore starts AT the
desktop, which is what the action lists' timings assume.

T10 itself is NOT fixed by this file: the shared harness (a78_patch2.py) is
left untouched, because the other half of its decision — the TeraDesk runs —
cannot be re-measured from this ticket.  T10 stays open.
"""
import sys

p = sys.argv[1] + '/kernel/src/host_env.rs'
s = open(p).read()

OLD = ("        let el0 = !tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null();\n"
       "        if !el0 && !DESK_UP.load(Ordering::Acquire) {\n"
       "            return; // A46: while an EL0 program runs, or once our desktop is up\n"
       "        }\n"
       "        if ARMED_BY_EL0.load(Ordering::Relaxed) == 0 { ARMED_BY_EL0.store(if el0 { 1 } else { 2 }, Ordering::Relaxed); }\n"
       "        let script: &[(u32, Step)] = if ARMED_BY_EL0.load(Ordering::Relaxed) == 1 { SCRIPT_TERA } else { SCRIPT_DESK };\n")
NEW = ("        if !DESK_UP.load(Ordering::Acquire) {\n"
       "            return; // T10: our desktop only -- the accessories must not arm it\n"
       "        }\n"
       "        let _ = &ARMED_BY_EL0;\n"
       "        let script: &[(u32, Step)] = SCRIPT_DESK;\n"
       "        let _ = SCRIPT_TERA;\n")

assert s.count(OLD) == 1, "a78 dual-arming block not found"
open(p, 'w').write(s.replace(OLD, NEW, 1))
print("patched", p)
