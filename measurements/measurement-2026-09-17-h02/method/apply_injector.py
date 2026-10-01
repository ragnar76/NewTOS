#!/usr/bin/env python3
"""Patch <ws>/kernel/src/host_env.rs with the H02 injector + the given SCRIPT (Rust text)."""
import sys, re
ws, script_rs = sys.argv[1], open(sys.argv[2]).read()
p = ws + '/kernel/src/host_env.rs'
s = open(p).read()
BEGIN = "// H02-TEMP BEGIN"; END = "// H02-TEMP END\n"
# strip a previous module
if BEGIN in s:
    s = s[:s.index(BEGIN)] + s[s.index(END)+len(END):]
a = "        crate::drivers::usb::poll();\n        core::hint::spin_loop();\n"
if "h02_inject::step();" not in s:
    assert s.count(a) == 1
    s = s.replace(a, "        crate::drivers::usb::poll();\n        h02_inject::step(); // H02-TEMP: scripted IKBD injector (audit scaffolding)\n        core::hint::spin_loop();\n", 1)
b = "    fn console_write_bytes(&self, s: &[u8]) {\n        for &b in s {\n            CONSOLE.write_byte(b);\n            crate::fbcon::write_byte(b);\n        }\n    }\n"
if "fn debug_write" not in s:
    assert s.count(b) == 1
    s = s.replace(b, b + "\n    /// H02-TEMP: serial-only trace sink (no fbcon).\n    fn debug_write(&self, s: &[u8]) {\n        for &b in s {\n            CONSOLE.write_byte(b);\n        }\n    }\n", 1)
s += BEGIN + '''
// Scripted IKBD byte injector for the H02 audit (QEMU raspi4b has no input
// path). Feeds mouse packets into `crate::ikbd::feed` on a schedule relative
// to the first AES pump of the first EL0 program. `Mark` prints a serial
// marker the QMP screendump driver waits for; `Arm` switches the AES trace on.
pub mod h02_inject {
    use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
    static T0: AtomicU32 = AtomicU32::new(0);
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    #[derive(Clone, Copy)]
    pub enum Step { Mv(i8, i8), Dn, Up, Arm, Mark(&'static str), Pause }

''' + script_rs + '''
    fn say(s: &str) {
        use tos_api::Host;
        super::KERNEL_HOST.debug_write(s.as_bytes());
    }
    pub fn step() {
        if tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null() {
            return; // only while an EL0 program is the running process
        }
        let now = tos_api::bios::tick::hz_200();
        if T0.load(Ordering::Relaxed) == 0 {
            T0.store(now.max(1), Ordering::Relaxed);
            say("[h02] injector armed\\r\\n");
            return;
        }
        let t0 = T0.load(Ordering::Relaxed);
        let i = NEXT.load(Ordering::Relaxed);
        if i >= SCRIPT.len() {
            return;
        }
        let (at, st) = SCRIPT[i];
        if now.wrapping_sub(t0) >= at {
            match st {
                Step::Mv(dx, dy) => { for b in [0xF8u8, dx as u8, dy as u8] { crate::ikbd::feed(b); } }
                Step::Dn => { for b in [0xFAu8, 0, 0] { crate::ikbd::feed(b); } }
                Step::Up => { for b in [0xF8u8, 0, 0] { crate::ikbd::feed(b); } }
                Step::Arm => { tos_api::aes::H02_TRACE.store(true, Ordering::Relaxed); }
                Step::Mark(_) | Step::Pause => {}
            }
            let (mx, my) = tos_api::vdi::mouse_position();
            let mb = tos_api::vdi::mouse_buttons();
            let mut buf = [0u8; 160];
            let mut w = W(&mut buf, 0);
            use core::fmt::Write as _;
            match st {
                Step::Mark(name) => { let _ = write!(w, "[h02] MARK {} step {} at +{} mouse ({},{}) mb={}\\r\\n", name, i, now.wrapping_sub(t0), mx, my, mb); }
                _ => { let _ = write!(w, "[h02] step {} at +{} -> mouse ({},{}) mb={}\\r\\n", i, now.wrapping_sub(t0), mx, my, mb); }
            }
            let n = w.1;
            say(core::str::from_utf8(&buf[..n]).unwrap_or("?"));
            NEXT.store(i + 1, Ordering::Relaxed);
        }
    }
    struct W<'a>(&'a mut [u8], usize);
    impl core::fmt::Write for W<'_> {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let n = s.len().min(self.0.len() - self.1);
            self.0[self.1..self.1 + n].copy_from_slice(&s.as_bytes()[..n]);
            self.1 += n;
            Ok(())
        }
    }
}
''' + END
open(p, 'w').write(s)
print("patched", p)
