#!/usr/bin/env python3
"""GUI04 run B (scratch copy only), applied after apply_injector.py.

The injector arms on the first pump that sees a live EL0 process. Since the
accessories landed (ACC02-ACC04) that is an accessory, several seconds before
the desktop exists -- and once the accessories have issued their message wait
the running process is the EL1 desktop again, so `RUN` is null and the injector
never steps at all. Run B drives our own desktop, so the arming condition is
replaced by the one that matches: step only once "[desk] desktop up" has set
DESK_UP. Same workaround as
measurement-2026-09-25-desk42/method/t10_workaround.py (TASK/T10 stays open;
this file only picks the case this measurement needs).
"""
import sys

scr = sys.argv[1]


def rep(path, a, b):
    s = open(path).read()
    assert s.count(a) == 1, (path, a[:70])
    open(path, 'w').write(s.replace(a, b, 1))


p = scr + '/kernel/src/host_env.rs'
rep(p,
    "    static T0: AtomicU32 = AtomicU32::new(0);\n",
    "    static T0: AtomicU32 = AtomicU32::new(0);\n"
    "    /// GUI04-TEMP: set by `desk::…` once our desktop is on screen.\n"
    "    pub static DESK_UP: core::sync::atomic::AtomicBool =\n"
    "        core::sync::atomic::AtomicBool::new(false);\n")
rep(p,
    "        if tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null() {\n"
    "            return; // only while an EL0 program is the running process\n"
    "        }\n",
    "        if !DESK_UP.load(Ordering::Acquire) {\n"
    "            return; // GUI04 run B: the clock starts when our desktop is up\n"
    "        }\n")

rep(scr + '/kernel/src/desk/mod.rs',
    '    kprintln!(\n        "[desk] desktop up ({} drives, {} window(s) from the context)",',
    '    crate::host_env::h02_inject::DESK_UP.store(true, core::sync::atomic::Ordering::Release); // GUI04-TEMP\n'
    '    kprintln!(\n        "[desk] desktop up ({} drives, {} window(s) from the context)",')

print("patched", p)
