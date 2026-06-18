//! IKBD decoder self-test: feed a simulated Atari scancode stream and
//! verify it decodes to the expected ASCII in the console RX ring, with
//! mouse packets ignored. Runs before the shell (which drains the ring).

use crate::{console, ikbd, kprintln};

/// Feed a simulated IKBD stream and check the decoded ASCII.
pub fn run() {
    // A relative-mouse packet (0xFA + 2 data bytes) must be skipped.
    for b in [0xFA, 0x12, 0xEE] {
        ikbd::feed(b);
    }

    // Type "Hi!": Shift+h, i, Shift+1. Make = scancode, break = | 0x80.
    let stream = [
        0x2A, // LShift down
        0x23, 0xA3, // h make/break -> 'H'
        0x2A | 0x80, // LShift up
        0x17, 0x97, // i make/break -> 'i'
        0x2A, // LShift down
        0x02, 0x82, // 1 make/break -> '!'
        0x2A | 0x80, // LShift up
    ];
    for b in stream {
        ikbd::feed(b);
    }

    // Drain what the decoder produced.
    let mut out = [0u8; 8];
    let mut n = 0;
    while let Some(c) = console::rx_pop() {
        if n < out.len() {
            out[n] = c;
            n += 1;
        }
    }
    let ok = &out[..n] == b"Hi!";
    kprintln!(
        "[ikbd] decoded \"{}\" ({} bytes) {} (mouse packet ignored)",
        core::str::from_utf8(&out[..n]).unwrap_or("?"),
        n,
        if ok { "OK" } else { "FAIL" },
    );
}
