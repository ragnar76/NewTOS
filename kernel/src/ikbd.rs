//! Atari ST IKBD (Intelligent Keyboard) decoder.
//!
//! The Atari ST IKBD streams a serial protocol to the host: key make/break
//! scancodes plus mouse / joystick / clock packets. This module is the
//! protocol **decoder** — the byte sink the rest of the system reads
//! keyboard/mouse input through, independent of where the bytes come from.
//!
//! The byte **source** is the USB-HID stack (PHASE-16 / ADR-015): the USB
//! keyboard/mouse class drivers translate HID reports into the exact IKBD
//! byte stream a real keyboard would send and push it through [`feed`].
//! (Wiring a native ST keyboard to UART2 was shelved — too much analogue
//! glue; the `ikbd_uart` driver was removed.)
//!
//! [`feed`] consumes one byte at a time, assembles the multi-byte
//! mouse/joystick/clock packets, turns key make events into ASCII (US
//! layout) → the console RX ring (`Cnecin`/`Cconin`/the shell), and
//! relative-mouse packets → the VDI cursor.
//!
//! Protocol + scancode set: Atari IKBD documentation; the scancode→ASCII
//! tables mirror EmuTOS `bios/keyboard.c` (US layout). Country layouts
//! and dead-key handling come from EmuTOS later.

use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use crate::console;

// ── modifier state ────────────────────────────────────────────────────
static SHIFT: AtomicBool = AtomicBool::new(false);
static CTRL: AtomicBool = AtomicBool::new(false);
static CAPS: AtomicBool = AtomicBool::new(false);
static ALT: AtomicBool = AtomicBool::new(false);

// IKBD scancodes of the modifier keys.
const SC_CTRL: u8 = 0x1D;
const SC_LSHIFT: u8 = 0x2A;
const SC_RSHIFT: u8 = 0x36;
const SC_ALT: u8 = 0x38;
const SC_CAPS: u8 = 0x3A;

/// Remaining data bytes to skip for the packet currently being received
/// (mouse / joystick / status / clock). `0` = expecting a scancode or a
/// new packet header.
static PKT_REMAINING: AtomicU8 = AtomicU8::new(0);

/// Number of data bytes that follow an IKBD status-packet header.
fn packet_data_len(header: u8) -> u8 {
    match header {
        0xF6 => 6,         // status report
        0xF7 => 5,         // absolute mouse
        0xF8..=0xFB => 2,  // relative mouse (low 2 bits = buttons)
        0xFC => 6,         // time-of-day
        0xFD => 2,         // both joysticks
        0xFE | 0xFF => 1,  // joystick 0 / joystick 1
        _ => 0,
    }
}

/// Relative-mouse packet assembly: `0` = idle, `1` = expecting `dx`,
/// `2` = expecting `dy`.
static MS_STEP: AtomicU8 = AtomicU8::new(0);
/// Button bitmask (already swapped to VDI convention) from the packet
/// header, held until the movement bytes complete the packet.
static MS_BUTTONS: AtomicU8 = AtomicU8::new(0);
/// First movement byte (`dx`), held until `dy` arrives.
static MS_DX: AtomicU8 = AtomicU8::new(0);

/// Feed one received IKBD byte to the decoder.
pub fn feed(byte: u8) {
    // Mid relative-mouse packet → collect dx, then dy, then dispatch.
    match MS_STEP.load(Ordering::Relaxed) {
        1 => {
            MS_DX.store(byte, Ordering::Relaxed);
            MS_STEP.store(2, Ordering::Relaxed);
            return;
        }
        2 => {
            MS_STEP.store(0, Ordering::Relaxed);
            let dx = MS_DX.load(Ordering::Relaxed) as i8 as i16;
            let dy = byte as i8 as i16;
            mouse_event(MS_BUTTONS.load(Ordering::Relaxed), dx, dy);
            return;
        }
        _ => {}
    }

    // Inside a non-mouse status packet → consume and ignore.
    let rem = PKT_REMAINING.load(Ordering::Relaxed);
    if rem > 0 {
        PKT_REMAINING.store(rem - 1, Ordering::Relaxed);
        return;
    }
    // A packet header (>= 0xF6) starts a run of data bytes.
    if byte >= 0xF6 {
        // Relative-mouse report (0xF8..0xFB): the low 2 header bits are
        // the buttons (IKBD wire: bit 0 = right, bit 1 = left). Swap into
        // VDI convention (bit 0 = left, bit 1 = right) and collect the two
        // movement bytes. See docs/api/vdi/input/_mouse-packet.md.
        if (0xF8..=0xFB).contains(&byte) {
            let wire = byte & 0x03;
            let gem = ((wire >> 1) & 0x01) | ((wire & 0x01) << 1);
            MS_BUTTONS.store(gem, Ordering::Relaxed);
            MS_STEP.store(1, Ordering::Relaxed);
            return;
        }
        PKT_REMAINING.store(packet_data_len(byte), Ordering::Relaxed);
        return;
    }
    // Otherwise it is a key scancode (bit 7 = break).
    decode_key(byte);
}

/// Apply a decoded relative-mouse event to the VDI mouse state. The byte
/// decode (hardware) lives here; the cursor semantics (position
/// accumulation + clamp, with `buttons` already in VDI convention) live
/// in the TOS API layer.
fn mouse_event(buttons: u8, dx: i16, dy: i16) {
    // No screen device yet → nothing to position the cursor against.
    if !crate::vdi_backend::available() {
        return;
    }
    tos_api::vdi::feed_relative(dx, dy, buttons, &crate::vdi_backend::VDI_SCREEN);
}

/// The seven IKBD scancodes that are **not** keys but extra mouse buttons and
/// wheel detents — EmuTOS `kbd_int`'s `CONF_WITH_EXTENDED_MOUSE` block
/// (`bios/ikbd.c:748-760`), which lists them in this order:
///
/// ```c
/// if (   scancode_only == 0x37  /* Mouse button 3 */
///     || scancode_only == 0x5e  /* Mouse button 4 */
///     || scancode_only == 0x5f  /* Mouse button 5 */
///     || scancode_only == 0x59  /* Wheel up */
///     || scancode_only == 0x5a  /* Wheel down */
///     || scancode_only == 0x5c  /* Wheel left */
///     || scancode_only == 0x5d) /* Wheel right */
/// ```
///
/// Compared on the **make** code (`scancode & ~KEY_RELEASED`), so both the
/// press and the release form reach the vector.
const fn is_extended_mouse(code: u8) -> bool {
    matches!(code, 0x37 | 0x59 | 0x5A | 0x5C | 0x5D | 0x5E | 0x5F)
}

fn decode_key(raw: u8) {
    let make = raw & 0x80 == 0;
    let code = raw & 0x7F;
    // `CONF_WITH_EXTENDED_MOUSE` is `1` in the reference build
    // (`include/config.h:1491-1498`): the extra mouse buttons and the wheel
    // arrive as IKBD scancodes and "use a separate vector" — `kbd_int` hands
    // them to `mousexvec(scancode)` and **returns** (`bios/ikbd.c:746-760`),
    // before the modifier bookkeeping, the ASCII translation and the key
    // queue, so none of the seven is ever seen as a key. EmuTOS places the
    // block after its Ctrl-Alt-Del reset check, which NewTOS' decoder does not
    // have; relative to everything the decoder *does* do, this is the same
    // position. The **full** scancode (release bit included) is passed on; the
    // handler tells press from release by it.
    if is_extended_mouse(code) {
        tos_api::bios::vectors::mousexvec(raw);
        return;
    }
    match code {
        SC_LSHIFT | SC_RSHIFT => SHIFT.store(make, Ordering::Relaxed),
        SC_CTRL => CTRL.store(make, Ordering::Relaxed),
        SC_ALT => ALT.store(make, Ordering::Relaxed), // AltGr for the DE layout
        SC_CAPS if make => {
            CAPS.fetch_xor(true, Ordering::Relaxed);
        }
        _ if make => {
            // Queue every make key as a word `scancode << 8 | ascii`; keys
            // with no character (arrows, function keys) carry ascii 0 so the
            // AES still sees the scancode (GEMDOS console readers skip them).
            let ascii = to_ascii(code).unwrap_or(0);
            console::push_rx((code as u16) << 8 | ascii as u16);
        }
        _ => {}
    }
}

/// The current keyboard shift-state bitmask, in BIOS `Kbshift` /
/// VDI `vq_key_s` encoding (`MODE_*` bits, `include/biosdefs.h`):
/// `0x01` right-shift, `0x02` left-shift, `0x04` ctrl, `0x08` alt,
/// `0x10` caps-lock. The decoder tracks a single shift flag (it does not
/// distinguish left/right), so a held shift reports the left-shift bit.
pub fn shift_state() -> u8 {
    let mut m = 0u8;
    if SHIFT.load(Ordering::Relaxed) {
        m |= 0x02; // MODE_LSHIFT
    }
    if CTRL.load(Ordering::Relaxed) {
        m |= 0x04; // MODE_CTRL
    }
    if ALT.load(Ordering::Relaxed) {
        m |= 0x08; // MODE_ALT
    }
    if CAPS.load(Ordering::Relaxed) {
        m |= 0x10; // MODE_CAPS
    }
    m
}

/// Overwrite the keyboard shift-state from a `Kbshift(flag)` set call
/// (`flag >= 0`). Mirrors EmuTOS storing `flag` into the single `shifty`
/// byte (`bios/ikbd.c:150`): each modifier atomic is written from the
/// matching `MODE_*` bit so a following [`shift_state`] reads the value
/// back, and a later physical make/break still updates its own bit. The
/// decoder tracks one shift flag, so either shift bit (`MODE_RSHIFT` /
/// `MODE_LSHIFT`, `0x03`) sets it; bits 5-7 (`MODE_HOME`/`MODE_INSERT`/
/// reserved) are TT/Falcon-era and not represented, so they do not
/// round-trip (`docs/api/bios/0b-kbshift.md`).
pub fn set_shift_state(mask: u8) {
    SHIFT.store(mask & 0x03 != 0, Ordering::Relaxed); // MODE_RSHIFT|MODE_LSHIFT
    CTRL.store(mask & 0x04 != 0, Ordering::Relaxed); // MODE_CTRL
    ALT.store(mask & 0x08 != 0, Ordering::Relaxed); // MODE_ALT
    CAPS.store(mask & 0x10 != 0, Ordering::Relaxed); // MODE_CAPS
}

/// Translate a key scancode to ASCII against the active keyboard layout
/// ([`crate::keymap`]) and the current modifiers, folding Ctrl + letter to
/// its control code. Returns `None` when the key has no character (cursor /
/// function keys, unmapped scancodes).
fn to_ascii(code: u8) -> Option<u8> {
    let shift = SHIFT.load(Ordering::Relaxed);
    let caps = CAPS.load(Ordering::Relaxed);
    let alt = ALT.load(Ordering::Relaxed);
    let mut ch = crate::keymap::to_ascii(code, shift, caps, alt);
    if ch == 0 {
        return None;
    }
    // Ctrl + letter → the corresponding control code (Ctrl-A = 0x01 …).
    if CTRL.load(Ordering::Relaxed) {
        let up = ch.to_ascii_uppercase();
        if up.is_ascii_uppercase() {
            ch = up - b'A' + 1;
        }
    }
    Some(ch)
}
