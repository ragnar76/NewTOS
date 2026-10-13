#!/usr/bin/env python3
"""apply-instrumentation.py <tree>

Scratch-only instrumentation for the KEYS02 kbdvec run (never applied to the
shared tree). Based on the KEYS01 injector
(measurement-2026-10-03-krepeat/method/apply-instrumentation.py), with two
changes:

* the injector step sits after EVERY usb::poll() -- pump_input (AES waits,
  Bconstat) AND both blocking loops (read_key_loop: Bconin/Cconin; byte
  read: Cconrs): the
  KEYS01 injector only sat in pump_input, so it froze during a blocking read
  (memory note injector-does-not-run-during-cconin);
* once armed (first pump with an EL0 program running) it keeps running with
  no program (the desktop after the program exits).

It also logs every word into (push_rx) and out of (rx_pop_key) the console
queue, every byte the kbdvec ring takes, and every kbdvec upcall delivered.

Schedule (200 Hz ticks after arming):
  +400 up make 0x48     +500 right make 0x4D
  +600 right break 0xCD AND up break 0xC8 in the SAME poll (program phase A,
       AES) -- the second byte can only reach the hook through the
       Psigreturn chain (make+break-in-one-poll case of spec B4)
  +900 'a' make 0x1E    +920 'a' break 0x9E     (program phase B, Bconin)
  +2000 Alt make 0x38   +2010 'c' make 0x2E
  +2020 'c' break 0xAE  +2030 Alt break 0xB8    (desktop after the exit:
                                                 Alt-C = open drive C)
"""
import sys, pathlib
t = pathlib.Path(sys.argv[1])

he = t / "kernel/src/host_env.rs"
s = he.read_text()
anchor = "crate::drivers::usb::poll();\n"
n = s.count(anchor)
assert n == 2, f"host_env.rs: expected 2 usb::poll sites, found {n}"  # A13: was 3 before pump_devices()
s = s.replace(anchor, "crate::drivers::usb::poll();\n        krp_inject::step(); // KRP-TEMP\n", 1)
s += '''
// KRP-TEMP BEGIN -- scripted IKBD make/break (QEMU raspi4b has no USB keyboard)
pub mod krp_inject {
    use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
    static T0: AtomicU32 = AtomicU32::new(0);
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    const SCRIPT: &[(u32, u8)] = &[
        (400, 0x48), (500, 0x4D), (600, 0xCD), (600, 0xC8),
        (900, 0x1E), (920, 0x9E),
        (2000, 0x38), (2010, 0x2E), (2020, 0xAE), (2030, 0xB8),
    ];
    pub fn step() {
        let now = tos_api::bios::tick::hz_200();
        if T0.load(Ordering::Relaxed) == 0 {
            if tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null() {
                return;
            }
            T0.store(now.max(1), Ordering::Relaxed);
            crate::kprintln!("[krp] injector armed at hz_200 {}", now);
            return;
        }
        let t0 = T0.load(Ordering::Relaxed);
        // Every entry that is due goes out in this one poll (entries with the
        // same time arrive together, like two changes in one HID report).
        loop {
            let i = NEXT.load(Ordering::Relaxed);
            if i >= SCRIPT.len() {
                return;
            }
            let (at, b) = SCRIPT[i];
            if now.wrapping_sub(t0) < at {
                return;
            }
            let run = !tos_api::bdos::process::RUN.load(Ordering::Acquire).is_null();
            crate::kprintln!("[krp] feed {:#04x} at hz_200 {} (+{}) run={}", b, now, now.wrapping_sub(t0), run);
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

ik = t / "kernel/src/ikbd.rs"
s = ik.read_text()
old = """    if !tos_api::xbios::kbdvecs::route_key(byte) {
        decode_key(byte);
    }"""
assert s.count(old) == 1, "ikbd.rs route anchor"
s = s.replace(old, """    if !tos_api::xbios::kbdvecs::route_key(byte) {
        decode_key(byte);
    } else {
        crate::kprintln!("[krp] ring <- {:#04x} at hz_200 {}", byte, tos_api::bios::tick::hz_200()); // KRP-TEMP
    }""")
old = """pub fn kbd_int(raw: u8) {
    decode_key(raw);
}"""
assert s.count(old) == 1, "ikbd.rs kbd_int anchor"
s = s.replace(old, """pub fn kbd_int(raw: u8) {
    crate::kprintln!("[krp] kbd_int {:#04x} at hz_200 {}", raw, tos_api::bios::tick::hz_200()); // KRP-TEMP
    decode_key(raw);
}""")
ik.write_text(s)

ex = t / "kernel/src/arch/aarch64/exceptions.rs"
s = ex.read_text()
old = """        deliver_upcall(
            frame,
            UpcallDesc {
                handler_va: handler,"""
assert s.count(old) == 1, "exceptions.rs kbdvec deliver anchor"
s = s.replace(old, """        crate::kprintln!("[krp] kbdvec upcall {:#04x} -> {:#x} elr={:#x}", byte, handler, frame.elr); // KRP-TEMP
        deliver_upcall(
            frame,
            UpcallDesc {
                handler_va: handler,""")
old = """fn restart_for_kbdvec(frame: &mut ExceptionFrame) {
    frame.elr = frame.elr.wrapping_sub(4);"""
assert s.count(old) == 1, "exceptions.rs restart anchor"
s = s.replace(old, """fn restart_for_kbdvec(frame: &mut ExceptionFrame) {
    crate::kprintln!("[krp] restart svc at {:#x} x8={:#x} x0={}", frame.elr.wrapping_sub(4), frame.gpr[8], frame.gpr[0]); // KRP-TEMP
    frame.elr = frame.elr.wrapping_sub(4);""")
ex.write_text(s)
print("instrumented", t)
