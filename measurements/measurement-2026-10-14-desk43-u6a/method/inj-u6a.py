#!/usr/bin/env python3
"""inj-u6a.py <scratch-tree>

DESK43 U6a: scripted IKBD key injector for the HELLOGEM runs (QEMU raspi4b has
no USB keyboard). Applied to a SCRATCH copy of the tree only (make-scratch.sh),
never to the shared tree; the product kernel is built unpatched by checks.sh.

Base: measurement-2026-10-14-desk43-u5a/method/inj-desk-seq.py (one step after
the trap-side usb::poll() in host_env::pump_devices, ikbd::feed of make/break
scancodes), with two arming points instead of one:

  A  "[desk] desktop up" while no program has been launched yet:
       +200 F1 make 0x3B, +210 F1 break 0xBB
     = the desktop launches the application installed on F1 (NEWDESK.INF
       `#G ... 001 @`, EmuTOS check_function_key/process_funkey) -- the
       desktop's own launch path (do_aopen -> shel_write -> sh_ldapp).
     Not armed when a program ran first (the #Z runs), and only once.
  B  "[launch] EL0" (kernel/src/launch.rs, the first EL0 program start):
       +1600 Return make 0x1C, +1610 Return break 0x9C   (8 s after the start)
     = the key HELLOGEM's evnt_multi(MU_KEYBD|...) waits for. Only once.
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])

he = t / "kernel/src/host_env.rs"
s = he.read_text()
anchor = "crate::drivers::usb::poll();\n"
assert s.count(anchor) == 2, "host_env.rs: expected 2 usb::poll sites"
s = s.replace(anchor, "crate::drivers::usb::poll();\n        u6a_inject::step(); // U6A-TEMP\n", 1)
s += '''
// U6A-TEMP BEGIN -- scripted IKBD make/break (QEMU raspi4b has no USB keyboard)
pub mod u6a_inject {
    use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
    /// Set by the desktop right before "[desk] desktop up".
    pub static DESK_UP: AtomicBool = AtomicBool::new(false);
    /// Set by launch.rs right before "[launch] EL0".
    pub static LAUNCHED: AtomicBool = AtomicBool::new(false);
    static TA: AtomicU32 = AtomicU32::new(0);
    static TB: AtomicU32 = AtomicU32::new(0);
    static NA: AtomicUsize = AtomicUsize::new(0);
    static NB: AtomicUsize = AtomicUsize::new(0);
    static A_OFF: AtomicBool = AtomicBool::new(false);
    const SCRIPT_A: &[(u32, u8)] = &[(200, 0x3B), (210, 0xBB)];
    const SCRIPT_B: &[(u32, u8)] = &[(1600, 0x1C), (1610, 0x9C)];
    fn run(tag: &str, t0: &AtomicU32, next: &AtomicUsize, script: &[(u32, u8)], now: u32) {
        let t = t0.load(Ordering::Relaxed);
        loop {
            let i = next.load(Ordering::Relaxed);
            if i >= script.len() {
                return;
            }
            let (at, b) = script[i];
            if now.wrapping_sub(t) < at {
                return;
            }
            crate::kprintln!("[u6a] {} feed {:#04x} at hz_200 {} (+{})", tag, b, now, now.wrapping_sub(t));
            crate::ikbd::feed(b);
            next.store(i + 1, Ordering::Relaxed);
        }
    }
    pub fn step() {
        let now = tos_api::bios::tick::hz_200();
        let launched = LAUNCHED.load(Ordering::Acquire);
        if TB.load(Ordering::Relaxed) == 0 && launched {
            TB.store(now.max(1), Ordering::Relaxed);
            crate::kprintln!("[u6a] B armed at hz_200 {}", now);
            if TA.load(Ordering::Relaxed) == 0 {
                A_OFF.store(true, Ordering::Relaxed);
            }
            return;
        }
        if TA.load(Ordering::Relaxed) == 0 && !A_OFF.load(Ordering::Relaxed) && !launched && DESK_UP.load(Ordering::Acquire) {
            TA.store(now.max(1), Ordering::Relaxed);
            crate::kprintln!("[u6a] A armed at hz_200 {}", now);
            return;
        }
        if TA.load(Ordering::Relaxed) != 0 {
            run("A", &TA, &NA, SCRIPT_A, now);
        }
        if TB.load(Ordering::Relaxed) != 0 {
            run("B", &TB, &NB, SCRIPT_B, now);
        }
    }
}
// U6A-TEMP END
'''
he.write_text(s)

dm = t / "kernel/src/desk/mod.rs"
s = dm.read_text()
old = '    kprintln!(\n        "[desk] desktop up ({} drives, {} window(s) from the context)",'
assert s.count(old) == 1, "desk/mod.rs desktop-up anchor"
s = s.replace(old, "    crate::host_env::u6a_inject::DESK_UP.store(true, core::sync::atomic::Ordering::Release); // U6A-TEMP\n" + old)
dm.write_text(s)

la = t / "kernel/src/launch.rs"
s = la.read_text()
old = '    kprintln!("[launch] EL0: slot {} entry 0x{:x}", child.slot, child.entry_va);\n'
assert s.count(old) == 1, "launch.rs EL0 anchor"
s = s.replace(old, "    crate::host_env::u6a_inject::LAUNCHED.store(true, core::sync::atomic::Ordering::Release); // U6A-TEMP\n" + old)
la.write_text(s)
print("instrumented", t)
