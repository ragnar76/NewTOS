#!/usr/bin/env python3
"""PORT01-TEMP (scratch copy only), applied after a78_patch2.py.

a78's dual arming picks SCRIPT_TERA whenever an EL0 process happens to be the
running one at the first injector tick. Since commit 03394c2 the boot starts
two DESK ACCESSORIES, which are EL0 processes scheduled before the desktop
appears -- so the very first tick sees `el0 == true`, the run silently picks the
empty SCRIPT_TERA, and no action ever fires (observed: `[h02] injector armed`
during `[sched] switch -> slot 0`, then nothing).

PORT01 drives our own desktop only, so this replaces the dual arming with the
one case it needs: step only once "[desk] desktop up" has set DESK_UP, and
always run SCRIPT_DESK. The clock therefore starts AT the desktop, which is
what the action lists' timings assume.
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
       "            return; // PORT01: our desktop only -- the accessories must not arm it\n"
       "        }\n"
       "        let _ = &ARMED_BY_EL0;\n"
       "        let script: &[(u32, Step)] = SCRIPT_DESK;\n"
       "        let _ = SCRIPT_TERA;\n")

assert s.count(OLD) == 1, "a78 dual-arming block not found"
open(p, 'w').write(s.replace(OLD, NEW, 1))
print("patched", p)
