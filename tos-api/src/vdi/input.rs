//! VDI logical input state — the four input devices' modes and the
//! sample-mode queries. Spec: `docs/api/vdi/input/_input.md`,
//! `docs/api/vdi/inquiry/_color-pixel-input.md`.
//!
//! The four GEM logical input devices (locator, valuator, choice,
//! string) each have an operating **mode**: REQUEST (block until input)
//! or SAMPLE (return immediately). `vsin_mode` sets one; `vqin_mode`
//! reads it back. Locator/choice/string sampling reads the live keyboard
//! and mouse state via the [`host`](crate::host).

use core::sync::atomic::{AtomicU8, Ordering};

use super::Vdipb;

/// Input mode: request (block) — the power-on default for every device.
pub const MODE_REQUEST: i16 = 1;
/// Input mode: sample (poll, return immediately).
pub const MODE_SAMPLE: i16 = 2;

// Per-device operating mode (index 1..=4 = locator/valuator/choice/string;
// index 0 unused). Default REQUEST.
static MODES: [AtomicU8; 5] = [const { AtomicU8::new(MODE_REQUEST as u8) }; 5];

/// Opcode 33 — `vsin_mode`: set the input mode of device `INTIN[0]`
/// (1..=4) to `INTIN[1]` (1 request / 2 sample). Echoes the device id.
pub fn vsin_mode(pb: &Vdipb) {
    let dev = pb.intin(0);
    let mode = pb.intin(1);
    if (1..=4).contains(&dev) && (MODE_REQUEST..=MODE_SAMPLE).contains(&mode) {
        MODES[dev as usize].store(mode as u8, Ordering::Relaxed);
    }
    pb.set_intout(0, dev);
    pb.set_contrl(4, 1);
}

/// Opcode 115 — `vqin_mode`: report the input mode of device `INTIN[0]`.
pub fn vqin_mode(pb: &Vdipb) {
    let dev = pb.intin(0);
    let mode = if (1..=4).contains(&dev) {
        MODES[dev as usize].load(Ordering::Relaxed) as i16
    } else {
        0
    };
    pb.set_intout(0, mode);
    pb.set_contrl(4, 1);
}
