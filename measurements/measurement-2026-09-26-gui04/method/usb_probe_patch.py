#!/usr/bin/env python3
"""GUI04 run D (scratch copy only): a serial probe on the USB->IKBD hop.

Run D is the one run with a REAL emulated USB mouse: QEMU's `virt` machine
carries `-device qemu-xhci -device usb-mouse`, our stack enumerates it
("HID iface 0 protocol 2 -> int-IN ep 0x81 max 4"), and QMP's
`input-send-event` can turn its wheel. What that measures -- and what the
raspi4b runs cannot -- is the piece above `mouse_report`: does a real wheel
report arrive with byte 3 set and a transfer length of 4?

Nothing about that is observable from outside without saying so on the serial
console, hence this probe. It prints one line per report that carries a wheel
byte and one line per extended-mouse scancode the IKBD decoder hands to
`mousexvec`. It is applied to an rsync'd scratch copy of the crates, never to
the shared tree.
"""
import sys

scr = sys.argv[1]


def rep(path, a, b):
    s = open(path).read()
    assert s.count(a) == 1, (path, a[:70])
    open(path, 'w').write(s.replace(a, b, 1))


# 1) the report as `mouse_report` receives it, and each scancode `wheel` emits.
rep(scr + '/kernel/src/drivers/usb/hid.rs',
    """fn wheel(r: &[u8; 8], len: usize) {
    if len <= 3 {
        return;
    }""",
    """fn wheel(r: &[u8; 8], len: usize) {
    // GUI04-TEMP probe
    if len > 3 && r[3] != 0 {
        crate::kprintln!("[gui04] report len={} byte3={:#04x}", len, r[3]);
    }
    if len <= 3 {
        return;
    }""")
rep(scr + '/kernel/src/drivers/usb/hid.rs',
    """    for _ in 0..n {
        ikbd::feed(code);
        ikbd::feed(code | 0x80);
    }""",
    """    crate::kprintln!("[gui04] wheel -> scancode {:#04x} x{}", code, n);
    for _ in 0..n {
        ikbd::feed(code);
        ikbd::feed(code | 0x80);
    }""")

# 2) the IKBD decoder's extended-mouse branch.
rep(scr + '/kernel/src/ikbd.rs',
    """    if is_extended_mouse(code) {
        tos_api::bios::vectors::mousexvec(raw);
        return;
    }""",
    """    if is_extended_mouse(code) {
        crate::kprintln!("[gui04] mousexvec({:#04x})", raw);
        tos_api::bios::vectors::mousexvec(raw);
        return;
    }""")

print("patched", scr)
