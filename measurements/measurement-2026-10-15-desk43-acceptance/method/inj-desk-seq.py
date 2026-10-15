#!/usr/bin/env python3
# DESK43 U8b copy of measurement-2026-10-14-desk43-u6b/method/inj-desk-seq.py. Only change: the (no-op) read/write of the
# desktop root follows the move kernel/src/desk/mod.rs -> programs/desktop/src/lib.rs.
"""inj-desk-seq.py <scratch-tree>  (copied unchanged from measurement-2026-10-14-desk43-u5a/method/ for DESK43 U5b)

DESK43 U5a, G2 prerequisite: the scripted key injector for the call-sequence
oracle run. Applied to a SCRATCH copy of the tree only (make-scratch.sh), never
to the shared tree.

Base: the KEYS02 injector (measurement-2026-10-13-mem05-acceptance/method/
inj-keys02-apply-instrumentation.py: one step after the trap-side usb::poll()
in host_env::pump_devices), without its console/kbdvec logging. Arming: NOT
"first EL0 process" -- since accessories run, that is an accessory, several
seconds before the desktop exists (TASK/T10; the same workaround as
measurement-2026-10-06-arch03-4/method/desk_arm_patch.py): the clock starts when
`[desk] desktop up` is printed.

Schedule (200 Hz ticks after "desktop up"):
  +200 Alt make 0x38   +210 'c' make 0x2E   +220 'c' break 0xAE   +230 Alt break 0xB8
  = Alt-C, "open drive C" -- a window open with wind_create/wind_calc/wind_open
  and the WF_WORKXYWH queries, exactly the calls the QED-mode tracer moved into
  its prologue (U1 §5 point 2).
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])

he = t / "kernel/src/host_env.rs"
s = he.read_text()
anchor = "crate::drivers::usb::poll();\n"
assert s.count(anchor) == 2, "host_env.rs: expected 2 usb::poll sites"
s = s.replace(anchor, "crate::drivers::usb::poll();\n        seq_inject::step(); // U5A-TEMP\n", 1)
s += '''
// U5A-TEMP BEGIN -- scripted IKBD make/break (QEMU raspi4b has no USB keyboard)
pub mod seq_inject {
    use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
    /// Set by the desktop right before "[desk] desktop up".
    pub static DESK_UP: AtomicBool = AtomicBool::new(false);
    static T0: AtomicU32 = AtomicU32::new(0);
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    const SCRIPT: &[(u32, u8)] = &[(200, 0x38), (210, 0x2E), (220, 0xAE), (230, 0xB8)];
    pub fn step() {
        let now = tos_api::bios::tick::hz_200();
        if T0.load(Ordering::Relaxed) == 0 {
            if !DESK_UP.load(Ordering::Acquire) {
                return;
            }
            T0.store(now.max(1), Ordering::Relaxed);
            crate::kprintln!("[u5a] injector armed at hz_200 {}", now);
            return;
        }
        let t0 = T0.load(Ordering::Relaxed);
        loop {
            let i = NEXT.load(Ordering::Relaxed);
            if i >= SCRIPT.len() {
                return;
            }
            let (at, b) = SCRIPT[i];
            if now.wrapping_sub(t0) < at {
                return;
            }
            crate::kprintln!("[u5a] feed {:#04x} at hz_200 {} (+{})", b, now, now.wrapping_sub(t0));
            crate::ikbd::feed(b);
            NEXT.store(i + 1, Ordering::Relaxed);
        }
    }
}
// U5A-TEMP END
'''
he.write_text(s)

# DESK43 U8a: the "desktop up" line is the kernel's now (shell_loop.rs,
# observe_desktop_gem_call, at the desktop's first evnt_multi); arm there.
sl = t / "kernel/src/shell_loop.rs"
s = sl.read_text()
old = '            kprintln!(\n                "[desk] desktop up ({} drives)",'
assert s.count(old) == 1, "shell_loop.rs desktop-up anchor"
s = s.replace(old, "            crate::host_env::seq_inject::DESK_UP.store(true, core::sync::atomic::Ordering::Release); // U5A-TEMP\n" + old)
sl.write_text(s)

dm = t / "programs/desktop/src/lib.rs"
s = dm.read_text()
dm.write_text(s)
print("instrumented", t)
