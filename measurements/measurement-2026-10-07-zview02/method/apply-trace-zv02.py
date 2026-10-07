#!/usr/bin/env python3
"""apply-trace-zv02.py <scratch-tree> [--no-count-clicks]

TASK/ZVIEW02 scratch-only instrument (NEVER applied to the shared tree; run
after apply-injector-m3.py on the same scratch copy). It records, without any
I/O on the AES path, what evnt_multi / evnt_button returned for a MU_BUTTON
wait, into a small static ring in tos-api (`aes::zv02`); the injector's
step() (kernel/src/host_env.rs, M3-TEMP) drains the ring onto the serial line
as "[zv02] ..." lines on its next poll.

Records (all values i32):
  kind 1  evnt_multi return: hz_in hz_out flags bclk bmask bstate fired
          EV_MX EV_MY EV_MB EV_BRET  -- logged when MU_BUTTON was requested
          AND (MU_BUTTON fired OR EV_MB != 0); the 20 ms timer-only returns
          with the button up are not logged (zView polls 50 times a second).
  kind 2  count_clicks exit (only if the tree still has it): hz_in hz_out
          max_clicks cap_ms_left got
  kind 3  evnt_button return: hz_in hz_out bclk bmask bstate EV_MB RET
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])

mod = t / "tos-api/src/aes/mod.rs"
s = mod.read_text()
anchor = "pub mod trace;\n"
assert s.count(anchor) == 1
s = s.replace(anchor, anchor + "pub mod zv02; // ZV02-TEMP\n")
mod.write_text(s)

(t / "tos-api/src/aes/zv02.rs").write_text('''//! ZV02-TEMP scratch trace ring (TASK/ZVIEW02), never in the shared tree.
use core::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
const N: usize = 64;
const W: usize = 12;
static RING: [AtomicI32; N * W] = [const { AtomicI32::new(0) }; N * W];
static HEAD: AtomicUsize = AtomicUsize::new(0);
static TAIL: AtomicUsize = AtomicUsize::new(0);
pub static LAST_MB: AtomicI32 = AtomicI32::new(0);
pub fn hz() -> i32 { crate::bios::tick::hz_200() as i32 }
pub fn rec(r: &[i32]) {
    let h = HEAD.load(Ordering::Relaxed);
    if h.wrapping_sub(TAIL.load(Ordering::Relaxed)) >= N { return; }
    let base = (h % N) * W;
    for i in 0..W { RING[base + i].store(*r.get(i).unwrap_or(&0), Ordering::Relaxed); }
    HEAD.store(h.wrapping_add(1), Ordering::Relaxed);
}
pub fn drain(mut f: impl FnMut(&[i32; W])) {
    loop {
        let t = TAIL.load(Ordering::Relaxed);
        if t == HEAD.load(Ordering::Relaxed) { return; }
        let base = (t % N) * W;
        let mut r = [0i32; W];
        for i in 0..W { r[i] = RING[base + i].load(Ordering::Relaxed); }
        TAIL.store(t.wrapping_add(1), Ordering::Relaxed);
        f(&r);
    }
}
''')

ev = t / "tos-api/src/aes/event.rs"
s = ev.read_text()
# fill_rets: remember the EV_MB it wrote
a = "    pb.set_intout(3, mb as i16); // EV_MB\n"
assert s.count(a) == 1, "fill_rets anchor"
s = s.replace(a, a + "    super::zv02::LAST_MB.store(mb as i32, core::sync::atomic::Ordering::Relaxed); // ZV02-TEMP\n")
# evnt_multi entry/exit
a = "pub fn evnt_multi(pb: &Aespb) -> Option<sched::Switch> {\n"
assert s.count(a) == 1
s = s.replace(a, a + "    let zv_hz_in = super::zv02::hz(); // ZV02-TEMP\n")
a = "    pb.set_intout(6, clicks); // EV_BRET\n    pb.set_ret(fired as i16);\n"
assert s.count(a) == 1, "evnt_multi exit anchor"
s = s.replace(a, a + """    { // ZV02-TEMP
        let mb = super::zv02::LAST_MB.load(core::sync::atomic::Ordering::Relaxed);
        if flags & MU_BUTTON != 0 && (fired & MU_BUTTON != 0 || mb != 0) {
            super::zv02::rec(&[1, zv_hz_in, super::zv02::hz(), flags as i32, b_flgclks as i32,
                b_mask as i32, b_state as i32, fired as i32,
                crate::vdi::mouse_position().0 as i32, crate::vdi::mouse_position().1 as i32, mb, clicks as i32]);
        }
    }
""")
# evnt_button entry/exit
a = "pub fn evnt_button(pb: &Aespb) -> Option<sched::Switch> {\n"
assert s.count(a) == 1
s = s.replace(a, a + "    let zv_hz_in = super::zv02::hz(); // ZV02-TEMP\n")
a = "    fill_rets(pb);\n    pb.set_ret(got.max(1));\n"
if s.count(a) == 1:
    s = s.replace(a, a + """    super::zv02::rec(&[3, zv_hz_in, super::zv02::hz(), bflgclks as i32, mask as i32, state as i32,
        super::zv02::LAST_MB.load(core::sync::atomic::Ordering::Relaxed), got.max(1) as i32]); // ZV02-TEMP
""")
else:
    print("note: evnt_button exit anchor not found (changed tree) -- kind 3 not recorded")
# count_clicks exit (pre-fix tree only)
a = "    if armed {\n        super::input::b_disarm();\n    }\n    got\n}\n"
if s.count(a) == 1:
    s = s.replace("    let win = dclick_ms();\n    let mut got = 1i16;\n",
                  "    let win = dclick_ms();\n    let mut got = 1i16;\n    let zv_hz_in = super::zv02::hz(); // ZV02-TEMP\n    let zv_now = h.monotonic_millis();\n", 1)
    s = s.replace(a, "    if armed {\n        super::input::b_disarm();\n    }\n"
                  "    super::zv02::rec(&[2, zv_hz_in, super::zv02::hz(), max_clicks as i32, "
                  "cap.map(|c| c.saturating_sub(zv_now) as i32).unwrap_or(-1), got as i32]); // ZV02-TEMP\n    got\n}\n")
else:
    print("note: count_clicks not present -- kind 2 not recorded")
ev.write_text(s)

he = t / "kernel/src/host_env.rs"
s = he.read_text()
a = "    pub fn step() {\n"
assert s.count(a) == 1, "injector step() anchor (apply-injector-m3.py first)"
s = s.replace(a, a + """        tos_api::aes::zv02::drain(|r| match r[0] { // ZV02-TEMP
            1 => crate::kprintln!("[zv02] evnt_multi hz {}..{} flags {:#x} bclk {:#x} bmask {} bstate {} -> fired {:#x} mx {} my {} EV_MB {} EV_BRET {}", r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8], r[9], r[10], r[11]),
            2 => crate::kprintln!("[zv02] count_clicks hz {}..{} max {} cap_ms_left {} -> got {}", r[1], r[2], r[3], r[4], r[5]),
            _ => crate::kprintln!("[zv02] evnt_button hz {}..{} bclk {:#x} bmask {} bstate {} -> EV_MB {} RET {}", r[1], r[2], r[3], r[4], r[5], r[6], r[7]),
        });
""")
he.write_text(s)
print("zv02 trace applied to", t)
