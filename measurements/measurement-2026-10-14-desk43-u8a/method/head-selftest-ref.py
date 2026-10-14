#!/usr/bin/env python3
"""head-selftest-ref.py <scratch-tree> -- DESK43 U8a: reference run of the four desktop self-tests on
HEAD 6aedd159 (before U8a), to tell pre-existing test results from U8a effects.

Applied to /tmp/m5j/u8a/headtree (`git archive 6aedd159`), never to the shared tree: calls the four
`selftest()`s (unchanged, still printing with kprintln) once, at the same place U8a's
`selftest::run_once` sits (deskmain, after `pro::desk_busy_off()`). Built `cargo xtask build --board
rpi4 --release` (SOURCE_DATE_EPOCH=1791979200) -> bin/kernel8-ref-head-selftest.img; booted like
selftest-run.sh step 2 but WITHOUT the Return injector, screendump at 60 s:
serial/ref-head-selftest-rpi4-serial.log, screendumps/ref-head-selftest-rpi4-t60.png.
"""
import sys, pathlib
p = pathlib.Path(sys.argv[1]) / "kernel/src/desk/mod.rs"
s = p.read_text()
old = """    pro::desk_busy_off();

    kprintln!("""
assert s.count(old) == 1
s = s.replace(old, """    pro::desk_busy_off();
    { // U8A-SCRATCH: HEAD reference run of the four self-tests at the U8a call site
        static RAN: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
        if !RAN.swap(true, core::sync::atomic::Ordering::AcqRel) {
            kprintln!("[desk-selftest] begin (HEAD scratch)");
            ins::selftest(); dir::selftest(); inf::selftest(); inf::selftest_install();
            kprintln!("[desk-selftest] done");
        }
    }

    kprintln!(""")
p.write_text(s)
print("instrumented", p)
