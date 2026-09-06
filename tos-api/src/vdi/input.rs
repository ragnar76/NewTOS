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

use super::backend::RasterBackend;
use super::Vdipb;

// Per-device operating mode in EmuTOS convention (`INTIN[1] − 1`):
// 0 = REQUEST, 1 = SAMPLE (`vdi_input.c:122-141`). Index 1..=4 =
// locator/valuator/choice/string; index 0 unused. Default 0 (request),
// matching `vdimouse_init` (`vdi_mouse.c:575-578`).
static MODES: [AtomicI16; 5] = [const { AtomicI16::new(0) }; 5];

/// The four logical input devices, in the VDI's own `INTIN[0]` numbering
/// (`vdi_input.c:122-141`). Only the string device is named here — it is the
/// one the AES writes (see [`set_input_mode`]).
pub(crate) const DEV_STRING: i16 = 4;

/// `INTIN[1]` operating modes as a caller passes them (the stored value is
/// this minus one): 1 = REQUEST (block until input), 2 = SAMPLE (return at
/// once). Only SAMPLE is named here — it is the one the AES writes.
pub(crate) const MODE_SAMPLE: i16 = 2;

/// The state half of [`vsin_mode`]: store `mode − 1` for device `dev`,
/// exactly as `vdi_vsin_mode`'s `switch` does (`vdi/vdi_input.c:122-141`).
/// A device selector outside 1..=4 stores nothing (the C `switch` has no
/// `default` arm); the mode itself is not range-checked, EmuTOS stores it
/// verbatim.
///
/// Split out of the opcode because the **AES** performs this same write, and
/// on EmuTOS it reaches it through the VDI trap: `gsx_char()` issues
/// `gsx_ncode(SET_INPUT_MODE, 0, 2)` with `intin = {4, 2}` before every
/// `v_string` (`aes/gemgsxif.c:459-463`). The four mode variables are
/// file-scope VDI globals, not per-workstation fields
/// (`vdi/vdi_defs.h:258`), so both writers must land in this one state —
/// which is why an AES keyboard poll is observable to an unrelated
/// application through `vqin_mode`.
pub(crate) fn set_input_mode(dev: i16, mode: i16) {
    if (1..=4).contains(&dev) {
        MODES[dev as usize].store(mode - 1, Ordering::Relaxed);
    }
}

/// The `vdimouse_init` defaults: all four devices back to REQUEST
/// (`vdi/vdi_mouse.c:575-578`). EmuTOS runs `vdimouse_init()` from
/// `vdi_v_opnwk` alone (`vdi/vdi_control.c:563`), so opening the **physical**
/// workstation clears a mode an earlier caller — the AES included — had set,
/// while `v_opnvwk` leaves it standing.
pub(crate) fn reset_input_modes() {
    for m in &MODES {
        m.store(0, Ordering::Relaxed);
    }
}

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
    set_input_mode(dev, mode);
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

/// Opcode 28 — `v_locator`: report the locator point + terminator (EmuTOS
/// `vdi_v_locator`, `vdi_mouse.c:280-314`).
///
/// On entry the locator / VDI cursor position `GCURX`/`GCURY` is set from the
/// input point `PTSIN[0..1]`, unconditionally in both modes (`:286-287`).
/// REQUEST (`vrq_locator`) brackets the wait with `dis_cur`/`hide_cur` (the
/// cursor is shown while waiting), loops until a keyboard key **or** a mouse
/// button press/release (`gloc_key & 1`), then returns the terminator in
/// `INTOUT[0]`, sets `CONTRL[2] = CONTRL[4] = 1`, and the point in
/// `PTSOUT[0..1]` = `GCURX`/`GCURY`. SAMPLE (`vsm_locator`) polls `gloc_key`
/// once: a key/button sets `CONTRL[4]` + `INTOUT[0]`, motion sets `CONTRL[2]` +
/// `PTSOUT`; with neither, the dispatcher's pre-loaded zero counts report
/// "no event". Cooperative single-tasking: the REQUEST wait pumps the input
/// sources (`pump_input`) like the AES event manager, so mouse packets (and
/// the resulting `cur_ms_stat` button/motion bits) advance while it blocks.
pub fn v_locator(pb: &Vdipb, backend: &dyn RasterBackend) {
    // Set the initial locator position from the input point (both modes).
    super::mouse::set_locator_pos(pb.ptsin(0), pb.ptsin(1));

    if is_request(1) {
        // REQUEST: show the cursor (dis_cur), wait for a key or button, then
        // hide it again (hide_cur) — the EmuTOS `:290`/`:301` bracket.
        super::mouse::show(backend);
        let mut term_ch = 0i16;
        if let Some(h) = crate::host::host() {
            loop {
                let (r, tc) = gloc_key();
                if r & 1 != 0 {
                    term_ch = tc;
                    break;
                }
                h.pump_input();
            }
        }
        super::mouse::hide(backend);
        pb.set_intout(0, term_ch & 0x00ff);
        pb.set_contrl(4, 1);
        pb.set_contrl(2, 1);
        let (gx, gy) = super::mouse::position();
        pb.set_ptsout(0, gx);
        pb.set_ptsout(1, gy);
    } else {
        // SAMPLE: one non-blocking poll of the terminator sources. The
        // dispatcher pre-loaded CONTRL[2] = CONTRL[4] = 0 ("no event").
        let (i, term_ch) = gloc_key();
        if i & 1 != 0 {
            pb.set_contrl(4, 1);
            pb.set_intout(0, term_ch & 0x00ff);
        }
        if i & 2 != 0 {
            pb.set_contrl(2, 1);
            let (gx, gy) = super::mouse::position();
            pb.set_ptsout(0, gx);
            pb.set_ptsout(1, gy);
        }
    }
}

/// EmuTOS `gloc_key` (`vdi_mouse.c:195-231`): poll the locator terminator
/// sources once. Returns `(retval, term_ch)`: `retval` bit 0 = a mouse-button
/// change or a keyboard key occurred (`term_ch` = the 16-bit terminator, 0x20
/// / 0x21 for the left/right button or `scancode<<8 | ascii` for a key), bit 1
/// = the mouse moved (the caller reads the live cursor position for the point).
/// A pending mouse-button change takes precedence over the keyboard, matching
/// the EmuTOS test order (`cur_ms_stat & 0xc0` before `Bconstat(2)`).
fn gloc_key() -> (i16, i16) {
    let mut retval = 0i16;
    let mut term_ch = 0i16;
    if let Some(t) = super::mouse::take_button_terminator() {
        term_ch = t;
        retval = 1;
    } else if let Some(w) = try_read_key_word() {
        // EmuTOS `TERM_CH = (ch>>8)|(ch&0xff)` = scancode<<8 | ascii, exactly
        // the key word; `v_locator` masks it to the ASCII byte for INTOUT[0].
        term_ch = w as i16;
        retval = 1;
    }
    if super::mouse::take_motion() {
        retval += 2;
    }
    (retval, term_ch)
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
    // The length limit `j` is the app-supplied maximum only — EmuTOS imposes no
    // fixed cap; the caller sizes INTOUT to its requested maximum (AUD-V6-004).
    let limit = max.unsigned_abs() as usize;
    let encode = |w: u16| -> i16 {
        if extended { w as i16 } else { (w & 0x00ff) as i16 }
    };

    if is_request(4) {
        // REQUEST: block for each key; terminate when the last char's ASCII
        // byte is CR (0x0d) or the limit is reached. The CR is stored into
        // INTOUT[n] but excluded from the returned count (EmuTOS `--i`).
        let mut term: u16 = 0;
        let mut n = 0usize;
        while n < limit && (term & 0x00ff) != 0x0d {
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
        // (stored, not a terminator). EmuTOS reads the char *before* the limit
        // check (`while ((gchr_key() != 0) && (i < j))`, `vdi_input.c:101`), so
        // a char read once the limit is reached is consumed but not stored —
        // the buffer-consume count matches EmuTOS exactly (AUD-V6-004).
        let mut n = 0usize;
        while let Some(w) = try_read_key_word() {
            if n < limit {
                pb.set_intout(n, encode(w));
                n += 1;
            } else {
                break; // char already consumed; not stored (i >= j)
            }
        }
        pb.set_contrl(4, n as i16);
    }
}
