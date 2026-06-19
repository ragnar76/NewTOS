//! IKBD decoder self-test: feed a simulated Atari scancode stream and
//! verify it decodes to the expected ASCII in the console RX ring, and
//! that a relative-mouse packet drives the VDI cursor. Run on demand
//! (the boot path no longer invokes the dev self-tests).

use crate::{console, ikbd, kprintln};

/// Feed a simulated IKBD stream and check the decoded ASCII + mouse state.
pub fn run() {
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

    // Drain what the decoder produced. A mouse packet (below) injects no
    // ASCII, so the ring holds exactly "Hi!".
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
        "[ikbd] decoded \"{}\" ({} bytes) {}",
        core::str::from_utf8(&out[..n]).unwrap_or("?"),
        n,
        if ok { "OK" } else { "FAIL" },
    );

    // Relative-mouse decode: a packet now drives the VDI cursor instead of
    // being skipped. Needs a screen backend to position against.
    if !crate::vdi_backend::available() {
        kprintln!("[ikbd] mouse decode: no screen backend, skipped");
        return;
    }
    let (w, h) = crate::vdi_backend::dims();

    // A zero-move packet forces the cursor to its initial (centre)
    // position, so the delta below is checked from a known base.
    for b in [0xF8u8, 0x00, 0x00] {
        ikbd::feed(b);
    }
    let (base_x, base_y) = tos_api::vdi::mouse_position();

    // Header 0xF9: IKBD wire bit 0 = right button → VDI bit 1 (0x02).
    // Movement dx = +5, dy = -3.
    for b in [0xF9u8, 5, (-3i8) as u8] {
        ikbd::feed(b);
    }
    let (mx, my) = tos_api::vdi::mouse_position();
    let mb = tos_api::vdi::mouse_buttons();

    let exp_x = (base_x + 5).clamp(0, w as i16 - 1);
    let exp_y = (base_y - 3).clamp(0, h as i16 - 1);
    let mouse_ok = mx == exp_x && my == exp_y && mb == 0x02;
    kprintln!(
        "[ikbd] mouse → ({},{}) bt={:#04x} (want ({},{}) 0x02) {}",
        mx,
        my,
        mb,
        exp_x,
        exp_y,
        if mouse_ok { "OK" } else { "FAIL" },
    );
}
