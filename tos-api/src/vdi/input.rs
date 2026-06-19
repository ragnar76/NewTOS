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

use super::mouse;
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

/// Device index → its operating mode.
fn mode_of(dev: usize) -> i16 {
    MODES[dev].load(Ordering::Relaxed) as i16
}

/// Read one byte from the keyboard (blocking), or `None` if no host
/// console is installed.
fn read_key() -> Option<u8> {
    crate::host::host().map(|h| h.console_read_byte_blocking())
}

/// Opcode 28 — `v_locator`: report the cursor position with a terminator.
/// SAMPLE returns the live position immediately; REQUEST blocks for a key
/// (the cursor cannot move without the mouse subsystem), returning that
/// key as the terminator. `PTSIN[0..1]` is the requested start position.
pub fn v_locator(pb: &Vdipb) {
    let (mx, my) = mouse::position();
    let request = mode_of(1) == MODE_REQUEST;
    let term = if request { read_key().unwrap_or(0) } else { 0 };
    pb.set_ptsout(0, if request { mx } else { mx.max(pb.ptsin(0)) });
    pb.set_ptsout(1, if request { my } else { my.max(pb.ptsin(1)) });
    pb.set_intout(0, term as i16);
    pb.set_contrl(2, 1);
    pb.set_contrl(4, 1);
}

/// Opcode 30 — `v_choice`: report a choice number. REQUEST blocks for a
/// key and returns it; SAMPLE returns 0 (no choice pending).
pub fn v_choice(pb: &Vdipb) {
    let choice = if mode_of(3) == MODE_REQUEST {
        read_key().unwrap_or(0) as i16
    } else {
        0
    };
    pb.set_intout(0, choice);
    pb.set_contrl(4, 1);
}

/// Opcode 31 — `v_string`: read a string into `INTOUT`, one char per
/// WORD. `INTIN[0]` = max length (negative = until terminator),
/// `INTIN[1]` = echo flag. REQUEST reads from the keyboard until CR or
/// the length limit; SAMPLE returns an empty string.
pub fn v_string(pb: &Vdipb) {
    if mode_of(4) != MODE_REQUEST {
        pb.set_contrl(4, 0);
        return;
    }
    let max = pb.intin(0);
    let limit = if max < 0 { (-max) as usize } else { max as usize };
    let echo = pb.intin(1) != 0;
    // Safety cap so a bogus length can't run away; real callers size
    // INTOUT to their requested maximum.
    const MAX_STRING: usize = 256;
    let mut n = 0usize;
    while n < limit.min(MAX_STRING) {
        let c = match read_key() {
            Some(c) => c,
            None => break,
        };
        if c == b'\r' || c == b'\n' {
            break;
        }
        if echo {
            if let Some(h) = crate::host::host() {
                h.console_write_byte(c);
            }
        }
        pb.set_intout(n, c as i16);
        n += 1;
    }
    pb.set_contrl(4, n as i16);
}
