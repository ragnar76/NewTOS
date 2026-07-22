//! VDI logical input state — the four input devices' modes and the
//! sample-mode queries. Spec: `docs/api/vdi/input/_input.md`,
//! `docs/api/vdi/inquiry/_color-pixel-input.md`.
//!
//! The four GEM logical input devices (locator, valuator, choice,
//! string) each have an operating **mode**: REQUEST (block until input)
//! or SAMPLE (return immediately). `vsin_mode` sets one; `vqin_mode`
//! reads it back. Locator/choice/string sampling reads the live keyboard
//! and mouse state via the [`host`](crate::host).

use core::sync::atomic::{AtomicI16, Ordering};

use super::Vdipb;

// Per-device operating mode in EmuTOS convention (`INTIN[1] − 1`):
// 0 = REQUEST, 1 = SAMPLE (`vdi_input.c:122-141`). Index 1..=4 =
// locator/valuator/choice/string; index 0 unused. Default 0 (request),
// matching `vdimouse_init` (`vdi_mouse.c:575-578`).
static MODES: [AtomicI16; 5] = [const { AtomicI16::new(0) }; 5];

/// Opcode 33 — `vsin_mode`: set the input mode of device `INTIN[0]`
/// (1..=4) to `INTIN[1]` (1 request / 2 sample). **Echoes the raw mode**
/// `INTIN[1]` in `INTOUT[0]`, not the device id (EmuTOS `vdi_vsin_mode`,
/// `vdi_input.c:118-142`).
pub fn vsin_mode(pb: &Vdipb) {
    let dev = pb.intin(0);
    let mode = pb.intin(1);
    // Echo the raw requested mode (1 = request, 2 = sample), even for an
    // out-of-range device.
    pb.set_intout(0, mode);
    pb.set_contrl(4, 1);
    // Store `mode − 1` (0 = request, 1 = sample). A device selector outside
    // 1..4 stores nothing (the C `switch` has no default). No range check on
    // the mode value — EmuTOS stores it verbatim.
    if (1..=4).contains(&dev) {
        MODES[dev as usize].store(mode - 1, Ordering::Relaxed);
    }
}

/// Opcode 115 — `vqin_mode`: report the input mode of device `INTIN[0]`.
/// Returns the stored `mode − 1` (**0/1**), NOT the documented 1/2 — a known
/// Atari-TOS quirk EmuTOS reproduces (`vdi_input.c:154-176`, D-VDI-11).
pub fn vqin_mode(pb: &Vdipb) {
    let dev = pb.intin(0);
    // Out-of-range device selectors leave INTOUT[0] UNTOUCHED (its stale
    // caller value): EmuTOS' `switch` has no `default` case, so it never
    // writes INTOUT[0] for an invalid device (`vdi_input.c:154-176`,
    // AUD-V1-013). CONTRL[4]=1 is still returned (the jump table's nintout).
    if (1..=4).contains(&dev) {
        pb.set_intout(0, MODES[dev as usize].load(Ordering::Relaxed));
    }
    pb.set_contrl(4, 1);
}

/// Whether device `dev` (1..=4) is in REQUEST mode. Stored 0 = request;
/// any non-zero stored value = sample (`if (loc_mode == 0)` in EmuTOS).
fn is_request(dev: usize) -> bool {
    MODES[dev].load(Ordering::Relaxed) == 0
}

/// Read one byte (ASCII) from the keyboard, blocking until one arrives, or
/// `None` if no host console is installed.
fn read_key() -> Option<u8> {
    crate::host::host().map(|h| h.console_read_byte_blocking())
}

/// Non-blocking read of one ASCII byte from the keyboard, or `None` if no
/// key is pending (or no host).
fn try_read_key() -> Option<u8> {
    crate::host::host().and_then(|h| h.console_try_read())
}

/// Blocking read of one whole key word (`scancode << 8 | ascii`) — the
/// scancode-carrying source used by `v_string`'s extended (`j < 0`) mode,
/// mirroring EmuTOS `gchr_key`/`TERM_CH`. `None` if no host.
fn read_key_word() -> Option<u16> {
    crate::host::host().map(|h| h.console_read_key_blocking())
}

/// Non-blocking read of one whole key word (`scancode << 8 | ascii`), or
/// `None` if no key is pending (or no host).
fn try_read_key_word() -> Option<u16> {
    crate::host::host().and_then(|h| h.console_try_read_key())
}

/// Opcode 28 — `v_locator`: report the locator point with a terminator.
/// The returned point is always the **input** point (`PTSIN[0..1]`), per
/// EmuTOS `vdi_v_locator` — not the live `GCURX`/`GCURY` (`vdi_mouse.c:277-278`).
///
/// REQUEST blocks until a terminating key and returns it (`CONTRL[2] =
/// CONTRL[4] = 1`, `PTSOUT` = input point). SAMPLE returns immediately: a
/// pending key sets `CONTRL[4] = 1` plus `INTOUT[0]`, motion sets `CONTRL[2] =
/// 1` plus `PTSOUT`; with neither, both counts stay 0 ("no event"). NewTOS has
/// no mouse-button terminator (the `cur_ms_stat` button bits are not
/// maintained, K-series), so only keyboard keys terminate; and it maintains no
/// motion-change bit, so SAMPLE reports no motion (never fabricated).
pub fn v_locator(pb: &Vdipb) {
    let (px, py) = (pb.ptsin(0), pb.ptsin(1));

    if is_request(1) {
        // REQUEST: block for the terminating key; the char is the terminator.
        let term = read_key().unwrap_or(0);
        pb.set_intout(0, term as i16);
        pb.set_contrl(4, 1);
        pb.set_ptsout(0, px);
        pb.set_ptsout(1, py);
        pb.set_contrl(2, 1);
    } else {
        // SAMPLE: a pending key → terminator; no motion source → no point.
        // The dispatcher pre-loaded CONTRL[2] = CONTRL[4] = 0.
        if let Some(term) = try_read_key() {
            // `try_read_key` already yields the ASCII byte (EmuTOS TERM_CH & 0xff).
            pb.set_intout(0, term as i16);
            pb.set_contrl(4, 1);
        }
    }
}

/// Opcode 30 — `v_choice`: DRI **dummy** device in both Atari TOS and EmuTOS.
/// `gchc_key` always sets `TERM_CH = 1` and returns, so `v_choice` reports
/// choice value **1** immediately in every mode, never blocking, ignoring
/// `chc_mode` and the keyboard (`vdi_input.c:31-36,199-203`).
pub fn v_choice(pb: &Vdipb) {
    pb.set_intout(0, 1);
    pb.set_contrl(4, 1);
}

/// Opcode 31 — `v_string`: read a string into `INTOUT`, one char per WORD.
/// `INTIN[0]` = max length `j` (negative → extended codes, see below);
/// `INTIN[1]` = echo flag, **ignored** (`vdi_input.c:62,77`). The sign of `j`
/// selects the encoding: `j ≥ 0` → ASCII only (`INTOUT[i] = ascii`); `j < 0`
/// → extended, `INTOUT[i] = scancode << 8 | ascii`. REQUEST blocks per char
/// until CR or the length limit (the CR is stored but not counted); SAMPLE
/// drains pending chars non-blocking, CR treated as an ordinary character.
pub fn v_string(pb: &Vdipb) {
    let max = pb.intin(0);
    let extended = max < 0;
    let limit = max.unsigned_abs() as usize;
    // Safety cap so a bogus length can't run past the caller's INTOUT; real
    // callers size INTOUT to their requested maximum.
    const MAX_STRING: usize = 256;
    let cap = limit.min(MAX_STRING);
    let encode = |w: u16| -> i16 {
        if extended { w as i16 } else { (w & 0x00ff) as i16 }
    };

    if is_request(4) {
        // REQUEST: block for each key; terminate when the last char's ASCII
        // byte is CR (0x0d) or the limit is reached. The CR is stored into
        // INTOUT[n] but excluded from the returned count (EmuTOS `--i`).
        let mut term: u16 = 0;
        let mut n = 0usize;
        while n < cap && (term & 0x00ff) != 0x0d {
            term = match read_key_word() {
                Some(w) => w,
                None => break, // no host console
            };
            pb.set_intout(n, encode(term));
            n += 1;
        }
        if (term & 0x00ff) == 0x0d && n > 0 {
            n -= 1; // drop the trailing CR from the count
        }
        pb.set_contrl(4, n as i16);
    } else {
        // SAMPLE: non-blocking drain of pending keys; CR is an ordinary char
        // (stored, not a terminator).
        let mut n = 0usize;
        while n < cap {
            match try_read_key_word() {
                Some(w) => {
                    pb.set_intout(n, encode(w));
                    n += 1;
                }
                None => break,
            }
        }
        pb.set_contrl(4, n as i16);
    }
}
