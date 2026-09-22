#!/usr/bin/env python3
"""A78-TEMP (scratch copy only), after apply_injector.py: A46 dual arming (EL0
program running OR our desktop up) and script selection by arming source —
SCRIPT_TERA when an EL0 program (TeraDesk via #Z) armed the injector,
SCRIPT_DESK when "[desk] desktop up" did."""
import sys
p = sys.argv[1] + '/kernel/src/host_env.rs'; s = open(p).read()
def rep(a, b):
    global s
    assert s.count(a) == 1, a[:60]
    s = s.replace(a, b, 1)
rep("        if tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null() {\n            return; // only while an EL0 program is the running process\n        }\n",
    "        let el0 = !tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null();\n"
    "        if !el0 && !DESK_UP.load(Ordering::Acquire) {\n            return; // A46: while an EL0 program runs, or once our desktop is up\n        }\n"
    "        if ARMED_BY_EL0.load(Ordering::Relaxed) == 0 { ARMED_BY_EL0.store(if el0 { 1 } else { 2 }, Ordering::Relaxed); }\n"
    "        let script: &[(u32, Step)] = if ARMED_BY_EL0.load(Ordering::Relaxed) == 1 { SCRIPT_TERA } else { SCRIPT_DESK };\n")
rep("    static T0: AtomicU32 = AtomicU32::new(0);\n",
    "    static T0: AtomicU32 = AtomicU32::new(0);\n"
    "    pub static DESK_UP: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false); // A46-TEMP\n"
    "    static ARMED_BY_EL0: AtomicU32 = AtomicU32::new(0); // A78-TEMP: 1 = EL0 program, 2 = desktop\n")
rep("        if i >= SCRIPT.len() {", "        if i >= script.len() {")
rep("        let (at, st) = SCRIPT[i];", "        let (at, st) = script[i];")
open(p, 'w').write(s)
print("patched", p)
