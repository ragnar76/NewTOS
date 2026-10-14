#!/usr/bin/env python3
# DESK43 U8a copy of measurement-2026-10-14-desk43-u7/method/inj-desk-views.py: only the arming anchor moved
# (desk/mod.rs -> shell_loop.rs, the kernel's new desktop-up line) and the metrics print is crate::kprintln.
r"""inj-desk-views.py <scratch-tree>  -- DESK43 U7: the U5a/U5b oracle key
injector (inj-desk-seq.py, same arming at "[desk] desktop up", same IKBD feed in
host_env::pump_devices) with a longer script that also drives the IKBD mouse
(relative packets 0xF8/0xFA + dx + dy, kernel/src/ikbd.rs decode_byte), so the
screen paths that use gl_wchar/gl_hchar/gl_wschar/gl_hschar are reached:
  +200   Alt-C           drive C:\ window (opens in text view per NEWDESK.INF)
  +1000  View -> Show as icons (pointer homed to 0,0 first; hover opens a title)
  +2000  Desk -> About   (inf.rs do_aboutdesk)      +3000 Return
  +3400  Options -> Set preferences (inf_pref)      +4400 Return
  +4800  View -> Set background (set_background)    +5800 Return
  +6200  Options -> Desktop configuration (inf_conf) +7200 Return
Also prints "[u7v] cell metrics ..." once, after deskmain stored them, when the
tree has global::set_cell_metrics (the U7 tree; HEAD has no such call).
Applied to a SCRATCH copy of the tree only, never to the shared tree.
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])

he = t / "kernel/src/host_env.rs"
s = he.read_text()
anchor = "crate::drivers::usb::poll();\n"
assert s.count(anchor) == 2, "host_env.rs: expected 2 usb::poll sites"
s = s.replace(anchor, "crate::drivers::usb::poll();\n        seq_inject::step(); // U7-TEMP\n", 1)
s += '''
// U7-TEMP BEGIN -- scripted IKBD make/break (QEMU raspi4b has no USB keyboard)
pub mod seq_inject {
    use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
    /// Set by the desktop right before "[desk] desktop up".
    pub static DESK_UP: AtomicBool = AtomicBool::new(false);
    static T0: AtomicU32 = AtomicU32::new(0);
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    const SCRIPT: &[(u32, u8)] = &[(200, 0x38), (210, 0x2E), (220, 0xAE), (230, 0xB8), (1000, 0xF8), (1000, 0x80), (1000, 0x80), (1000, 0xF8), (1000, 0x80), (1000, 0x80), (1000, 0xF8), (1000, 0x80), (1000, 0x80), (1000, 0xF8), (1000, 0x80), (1000, 0x80), (1010, 0xF8), (1010, 0x64), (1010, 0x09), (1010, 0xF8), (1010, 0x23), (1010, 0x00), (1100, 0xF8), (1100, 0x00), (1100, 0x14), (1300, 0xFA), (1300, 0x00), (1300, 0x00), (1310, 0xF8), (1310, 0x00), (1310, 0x00), (2000, 0xF8), (2000, 0x80), (2000, 0x80), (2000, 0xF8), (2000, 0x80), (2000, 0x80), (2000, 0xF8), (2000, 0x80), (2000, 0x80), (2000, 0xF8), (2000, 0x80), (2000, 0x80), (2010, 0xF8), (2010, 0x26), (2010, 0x09), (2100, 0xF8), (2100, 0x00), (2100, 0x14), (2200, 0xFA), (2200, 0x00), (2200, 0x00), (2210, 0xF8), (2210, 0x00), (2210, 0x00), (3000, 0x1C), (3010, 0x9C), (3400, 0xF8), (3400, 0x80), (3400, 0x80), (3400, 0xF8), (3400, 0x80), (3400, 0x80), (3400, 0xF8), (3400, 0x80), (3400, 0x80), (3400, 0xF8), (3400, 0x80), (3400, 0x80), (3410, 0xF8), (3410, 0x64), (3410, 0x09), (3410, 0xF8), (3410, 0x60), (3410, 0x00), (3500, 0xF8), (3500, 0x00), (3500, 0x62), (3600, 0xFA), (3600, 0x00), (3600, 0x00), (3610, 0xF8), (3610, 0x00), (3610, 0x00), (4400, 0x1C), (4410, 0x9C), (4800, 0xF8), (4800, 0x80), (4800, 0x80), (4800, 0xF8), (4800, 0x80), (4800, 0x80), (4800, 0xF8), (4800, 0x80), (4800, 0x80), (4800, 0xF8), (4800, 0x80), (4800, 0x80), (4810, 0xF8), (4810, 0x64), (4810, 0x09), (4810, 0xF8), (4810, 0x23), (4810, 0x00), (4900, 0xF8), (4900, 0x00), (4900, 0x64), (4900, 0xF8), (4900, 0x00), (4900, 0x5E), (5000, 0xFA), (5000, 0x00), (5000, 0x00), (5010, 0xF8), (5010, 0x00), (5010, 0x00), (5800, 0x1C), (5810, 0x9C), (6200, 0xF8), (6200, 0x80), (6200, 0x80), (6200, 0xF8), (6200, 0x80), (6200, 0x80), (6200, 0xF8), (6200, 0x80), (6200, 0x80), (6200, 0xF8), (6200, 0x80), (6200, 0x80), (6210, 0xF8), (6210, 0x64), (6210, 0x09), (6210, 0xF8), (6210, 0x60), (6210, 0x00), (6300, 0xF8), (6300, 0x00), (6300, 0x72), (6400, 0xFA), (6400, 0x00), (6400, 0x00), (6410, 0xF8), (6410, 0x00), (6410, 0x00), (7200, 0x1C), (7210, 0x9C)];
    pub fn step() {
        let now = tos_api::bios::tick::hz_200();
        if T0.load(Ordering::Relaxed) == 0 {
            if !DESK_UP.load(Ordering::Acquire) {
                return;
            }
            T0.store(now.max(1), Ordering::Relaxed);
            crate::kprintln!("[u7v] injector armed at hz_200 {}", now);
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
            crate::kprintln!("[u7v] feed {:#04x} at hz_200 {} (+{})", b, now, now.wrapping_sub(t0));
            crate::ikbd::feed(b);
            NEXT.store(i + 1, Ordering::Relaxed);
        }
    }
}
// U7-TEMP END
'''
he.write_text(s)

# DESK43 U8a: the "desktop up" line is the kernel's now (shell_loop.rs,
# observe_desktop_gem_call, at the desktop's first evnt_multi); arm there.
sl = t / "kernel/src/shell_loop.rs"
s = sl.read_text()
old = '            kprintln!(\n                "[desk] desktop up ({} drives)",'
assert s.count(old) == 1, "shell_loop.rs desktop-up anchor"
s = s.replace(old, "            crate::host_env::seq_inject::DESK_UP.store(true, core::sync::atomic::Ordering::Release); // U7-TEMP\n" + old)
sl.write_text(s)

dm = t / "kernel/src/desk/mod.rs"
s = dm.read_text()
anch = "    global::set_cell_metrics(gl_wchar, gl_hchar, gl_wschar, gl_hschar);\n"
if s.count(anch) == 1:
    s = s.replace(anch, anch + '    crate::kprintln!("[u7v] cell metrics gl_handle={} gl_wchar={} gl_hchar={} gl_wschar={} gl_hschar={}", gl_handle, gl_wchar, gl_hchar, gl_wschar, gl_hschar); // U7-TEMP\n')
    print("metrics print added")
dm.write_text(s)
print("instrumented", t)
