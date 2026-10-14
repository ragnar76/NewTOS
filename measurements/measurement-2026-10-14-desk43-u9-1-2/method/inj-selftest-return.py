#!/usr/bin/env python3
# (DESK43 U9-1 copy of measurement-2026-10-14-desk43-u8b/method/inj-selftest-return.py, unchanged below; applied after u91-instr.py.)
#!/usr/bin/env python3
# (DESK43 U8b copy of measurement-2026-10-14-desk43-u6b/method/inj-selftest-return.py. Changed: the arming. The self-test
#  runner is in the desktop crate now (programs/desktop/src/selftest.rs), which cannot name the kernel's
#  `crate::host_env` -- the point of U8b -- so the scratch copy declares an `extern "C"` hook there and the kernel copy
#  defines it (host_env.rs, #[no_mangle]); the linker joins the two. Same arming point, same script, same tag [u8a-st].)
"""inj-selftest-return.py <scratch-tree> -- DESK43 U8a: method instrument for the self-test run.

The four desktop self-tests are run as they are (test code unchanged since HEAD). One of them,
dir::selftest's recursive delete (dir_op(Op::Delete)), puts up the ADCPYDEL "DELETE FILE(S)" box
and waits for OK because the card's NEWDESK.INF has "confirm deletes" on (dir.rs dir_op doc,
deskdir.c:906,939-945) -- the test predates that wait. QEMU raspi4b has no USB keyboard, so the
run answers it the way a user would: Return (OK is the default button), fed as IKBD make/break by
the U5a injector (inj-desk-seq.py's host_env::pump_devices hook), armed right before
"[desk-selftest] begin" and pressing Return at +2 s, +7 s and +12 s (200 Hz ticks). Extra
presses land in the keyboard queue of a desktop that has nothing selected.
Applied to a SCRATCH copy of the tree only, never to the shared tree.
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])

he = t / "kernel/src/host_env.rs"
s = he.read_text()
anchor = "crate::drivers::usb::poll();\n"
assert s.count(anchor) == 2, "host_env.rs: expected 2 usb::poll sites"
s = s.replace(anchor, "crate::drivers::usb::poll();\n        seq_inject::step(); // U8A-TEMP\n", 1)
s += '''
// U8A-TEMP BEGIN -- scripted IKBD make/break (QEMU raspi4b has no USB keyboard)
pub mod seq_inject {
    use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
    /// Set right before "[desk-selftest] begin".
    pub static DESK_UP: AtomicBool = AtomicBool::new(false);
    static T0: AtomicU32 = AtomicU32::new(0);
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    const SCRIPT: &[(u32, u8)] = &[(400, 0x1C), (410, 0x9C), (1400, 0x1C), (1410, 0x9C), (2400, 0x1C), (2410, 0x9C)];
    pub fn step() {
        let now = tos_api::bios::tick::hz_200();
        if T0.load(Ordering::Relaxed) == 0 {
            if !DESK_UP.load(Ordering::Acquire) {
                return;
            }
            T0.store(now.max(1), Ordering::Relaxed);
            crate::kprintln!("[u8a-st] injector armed at hz_200 {}", now);
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
            crate::kprintln!("[u8a-st] feed {:#04x} at hz_200 {} (+{})", b, now, now.wrapping_sub(t0));
            crate::ikbd::feed(b);
            NEXT.store(i + 1, Ordering::Relaxed);
        }
    }
}
// U8A-TEMP END
'''
he.write_text(s)

st = t / "programs/desktop/src/selftest.rs"
s = st.read_text()
old = '    tprintln!("[desk-selftest] begin'
assert s.count(old) == 1, "selftest.rs begin anchor"
s = s.replace(old, "    unsafe { u8b_arm_injector() } // U8A-TEMP\n" + old)
s += '\n// U8A-TEMP (U8b: arms the kernel-side injector, kernel/src/host_env.rs of this scratch copy)\nextern "C" {\n    fn u8b_arm_injector();\n}\n'
st.write_text(s)
he = t / "kernel/src/host_env.rs"
h = he.read_text()
h += '\n// U8A-TEMP (U8b) -- the desktop crate cannot reach host_env; it calls this hook.\n#[no_mangle]\npub extern "C" fn u8b_arm_injector() {\n    seq_inject::DESK_UP.store(true, core::sync::atomic::Ordering::Release);\n}\n'
he.write_text(h)
print("instrumented", t)
