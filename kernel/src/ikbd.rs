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

fn decode_key(raw: u8) {
    let make = raw & 0x80 == 0;
    let code = raw & 0x7F;
    match code {
        SC_LSHIFT | SC_RSHIFT => SHIFT.store(make, Ordering::Relaxed),
        SC_CTRL => CTRL.store(make, Ordering::Relaxed),
        SC_ALT => { /* tracked for completeness; no ASCII effect yet */ }
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
    if CAPS.load(Ordering::Relaxed) {
        m |= 0x10; // MODE_CAPS
    }
    m
}

/// Translate a key scancode to ASCII given the current modifier state.
fn to_ascii(code: u8) -> Option<u8> {
    let (normal, shifted) = *TABLE.get(code as usize)?;
    if normal == 0 {
        return None;
    }
    let shift = SHIFT.load(Ordering::Relaxed);
    let caps = CAPS.load(Ordering::Relaxed);

    // Letters: effective case is shift XOR caps-lock. Other keys: shifted
    // variant only when shift is held.
    let is_letter = normal.is_ascii_lowercase();
    let mut ch = if is_letter {
        if shift ^ caps { shifted } else { normal }
    } else if shift {
        shifted
    } else {
        normal
    };

    // Ctrl + letter → the corresponding control code (Ctrl-A = 0x01 …).
    if CTRL.load(Ordering::Relaxed) {
        let up = ch.to_ascii_uppercase();
        if up.is_ascii_uppercase() {
            ch = up - b'A' + 1;
        }
    }
    Some(ch)
}

/// Atari ST scancode → (unshifted, shifted) ASCII, US layout. `(0, 0)`
/// means "no ASCII" (modifier / function / keypad / cursor keys).
/// Index = scancode (0x00..0x53). Mirrors EmuTOS' US key table.
#[rustfmt::skip]
static TABLE: [(u8, u8); 0x54] = {
    let mut t = [(0u8, 0u8); 0x54];
    t[0x01] = (0x1B, 0x1B);        // Esc
    t[0x02] = (b'1', b'!');
    t[0x03] = (b'2', b'@');
    t[0x04] = (b'3', b'#');
    t[0x05] = (b'4', b'$');
    t[0x06] = (b'5', b'%');
    t[0x07] = (b'6', b'^');
    t[0x08] = (b'7', b'&');
    t[0x09] = (b'8', b'*');
    t[0x0A] = (b'9', b'(');
    t[0x0B] = (b'0', b')');
    t[0x0C] = (b'-', b'_');
    t[0x0D] = (b'=', b'+');
    t[0x0E] = (0x08, 0x08);        // Backspace
    t[0x0F] = (0x09, 0x09);        // Tab
    t[0x10] = (b'q', b'Q');
    t[0x11] = (b'w', b'W');
    t[0x12] = (b'e', b'E');
    t[0x13] = (b'r', b'R');
    t[0x14] = (b't', b'T');
    t[0x15] = (b'y', b'Y');
    t[0x16] = (b'u', b'U');
    t[0x17] = (b'i', b'I');
    t[0x18] = (b'o', b'O');
    t[0x19] = (b'p', b'P');
    t[0x1A] = (b'[', b'{');
    t[0x1B] = (b']', b'}');
    t[0x1C] = (0x0D, 0x0D);        // Return
    t[0x1E] = (b'a', b'A');
    t[0x1F] = (b's', b'S');
    t[0x20] = (b'd', b'D');
    t[0x21] = (b'f', b'F');
    t[0x22] = (b'g', b'G');
    t[0x23] = (b'h', b'H');
    t[0x24] = (b'j', b'J');
    t[0x25] = (b'k', b'K');
    t[0x26] = (b'l', b'L');
    t[0x27] = (b';', b':');
    t[0x28] = (b'\'', b'"');
    t[0x29] = (b'`', b'~');
    t[0x2B] = (b'\\', b'|');
    t[0x2C] = (b'z', b'Z');
    t[0x2D] = (b'x', b'X');
    t[0x2E] = (b'c', b'C');
    t[0x2F] = (b'v', b'V');
    t[0x30] = (b'b', b'B');
    t[0x31] = (b'n', b'N');
    t[0x32] = (b'm', b'M');
    t[0x33] = (b',', b'<');
    t[0x34] = (b'.', b'>');
    t[0x35] = (b'/', b'?');
    t[0x39] = (b' ', b' ');        // Space
    t
};
