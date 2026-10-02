#!/usr/bin/env python3
"""apply-instrumentation.py <tree>

Scratch-only instrumentation for the KEYS01 key-repeat runs (never applied to
the shared tree). Identical for the "before" (HEAD 3387f75) and "after"
(working tree with KEYS01) kernels:

* host_env.rs: a scripted IKBD injector right after `usb::poll()` in
  `pump_input` -- it feeds exactly what drivers/usb/hid.rs feeds for a key
  that goes down and later up (one make, one break; a boot keyboard reports
  changes only). In the "after" tree it therefore runs BEFORE poll_repeat,
  in the same position as the real USB input.
* console.rs: log every word entering (push_rx) and leaving (rx_pop_key) the
  key queue with hz_200.

Schedule (200 Hz ticks after the first pump with an EL0 program running):
  +3000/+3200/+3400/+3600  Return make, break 10 ticks later
                           (Doom: open menu, New Game, episode, skill)
  +4200 .. +5200           hold up-arrow 5 s
  +5800 .. +6200           hold up-arrow 2 s
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])

he = t / "kernel/src/host_env.rs"
s = he.read_text()
old = """        crate::drivers::usb::poll();
        core::hint::spin_loop();"""
old_after = """        crate::drivers::usb::poll();
        // Key auto-repeat runs polled, after the input above (a break has"""
if old in s:
    s = s.replace(old, """        crate::drivers::usb::poll();
        krp_inject::step(); // KRP-TEMP
        core::hint::spin_loop();""", 1)
elif old_after in s:
    s = s.replace(old_after, """        crate::drivers::usb::poll();
        krp_inject::step(); // KRP-TEMP
        // Key auto-repeat runs polled, after the input above (a break has""", 1)
else:
    sys.exit("host_env.rs: anchor not found")
s += '''
// KRP-TEMP BEGIN -- scripted IKBD make/break (QEMU raspi4b has no USB keyboard)
pub mod krp_inject {
    use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
    static T0: AtomicU32 = AtomicU32::new(0);
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    const SCRIPT: &[(u32, u8)] = &[
        (3000, 0x1C), (3010, 0x9C),
        (3200, 0x1C), (3210, 0x9C),
        (3400, 0x1C), (3410, 0x9C),
        (3600, 0x1C), (3610, 0x9C),
        (4200, 0x48), (5200, 0xC8),
        (5800, 0x48), (6200, 0xC8),
    ];
    pub fn step() {
        if tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null() {
            return;
        }
        let now = tos_api::bios::tick::hz_200();
        if T0.load(Ordering::Relaxed) == 0 {
            T0.store(now.max(1), Ordering::Relaxed);
            crate::kprintln!("[krp] injector armed at hz_200 {}", now);
            return;
        }
        let t0 = T0.load(Ordering::Relaxed);
        let i = NEXT.load(Ordering::Relaxed);
        if i >= SCRIPT.len() {
            return;
        }
        let (at, b) = SCRIPT[i];
        if now.wrapping_sub(t0) >= at {
            crate::kprintln!("[krp] feed {:#04x} at hz_200 {} (+{})", b, now, now.wrapping_sub(t0));
            crate::ikbd::feed(b);
            NEXT.store(i + 1, Ordering::Relaxed);
        }
    }
}
// KRP-TEMP END
'''
he.write_text(s)

co = t / "kernel/src/console.rs"
s = co.read_text()
old = """    RX_RING.push(word);
}"""
assert s.count(old) == 1, "console.rs push anchor"
s = s.replace(old, """    crate::kprintln!("[krp] push_rx {:#06x} at hz_200 {}", word, tos_api::bios::tick::hz_200()); // KRP-TEMP
    RX_RING.push(word);
}""")
old = """pub fn rx_pop_key() -> Option<u16> {
    RX_RING.pop()
}"""
assert old in s, "console.rs pop anchor"
s = s.replace(old, """pub fn rx_pop_key() -> Option<u16> {
    let w = RX_RING.pop();
    if let Some(w) = w { crate::kprintln!("[krp] rx_pop_key {:#06x} at hz_200 {}", w, tos_api::bios::tick::hz_200()); } // KRP-TEMP
    w
}""")
co.write_text(s)
print("instrumented", t)
