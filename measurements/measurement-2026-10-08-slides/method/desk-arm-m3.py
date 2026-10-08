#!/usr/bin/env python3
"""desk-arm-m3.py <scratch-tree> -- applied after apply-injector-m3.py, scratch
copy only. The EmuDESK run has no EL0 program to arm on (the desktop runs in
the kernel), so the m3 injector's clock starts at "[desk] desktop up" instead
(the same workaround as measurement-2026-10-06-arch03-4/method/
desk_arm_patch.py, for the m3_inject module).
"""
import sys
scr = sys.argv[1]
def rep(path, a, b):
    s = open(path).read()
    assert s.count(a) == 1, (path, a[:70])
    open(path, 'w').write(s.replace(a, b, 1))
p = scr + '/kernel/src/host_env.rs'
rep(p, "    static T0: AtomicU32 = AtomicU32::new(0);\n",
       "    static T0: AtomicU32 = AtomicU32::new(0);\n"
       "    pub static DESK_UP: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);\n")
rep(p, "        if tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null() && T0.load(Ordering::Relaxed) == 0 {\n",
       "        if !DESK_UP.load(Ordering::Acquire) && T0.load(Ordering::Relaxed) == 0 {\n")
rep(scr + '/kernel/src/desk/mod.rs',
    '    kprintln!(\n        "[desk] desktop up ({} drives, {} window(s) from the context)",',
    '    crate::host_env::m3_inject::DESK_UP.store(true, core::sync::atomic::Ordering::Release); // EVB-TEMP\n'
    '    kprintln!(\n        "[desk] desktop up ({} drives, {} window(s) from the context)",')
print("patched", p)
