//! Atari ST IKBD (Intelligent Keyboard) decoder.
//!
//! The Atari ST keyboard has its own HD6301 controller that streams a
//! serial protocol (7812.5 baud, 8N1) to the host: key make/break
//! scancodes plus mouse / joystick / clock packets. A native ST keyboard
//! can be wired to a spare Pi UART (via a 5V->3.3V divider on the
//! keyboard's TX line), giving authentic Atari scancodes with no USB
//! stack — see [`crate::drivers::bcm2711::ikbd_uart`].
//!
//! This module is the protocol decoder: [`feed`] consumes one byte at a
//! time, skips the multi-byte mouse/joystick/clock packets, and turns key
//! make events into ASCII (US layout), pushing the result into the
//! console RX ring so `Cnecin`/`Cconin`/the shell read it transparently.
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

/// Feed one received IKBD byte to the decoder.
pub fn feed(byte: u8) {
    // Inside a multi-byte status packet → consume and ignore.
    let rem = PKT_REMAINING.load(Ordering::Relaxed);
    if rem > 0 {
        PKT_REMAINING.store(rem - 1, Ordering::Relaxed);
        return;
    }
    // A packet header (>= 0xF6) starts a run of data bytes to skip.
    if byte >= 0xF6 {
        PKT_REMAINING.store(packet_data_len(byte), Ordering::Relaxed);
        return;
    }
    // Otherwise it is a key scancode (bit 7 = break).
    decode_key(byte);
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
            if let Some(ch) = to_ascii(code) {
                console::push_rx(ch);
            }
        }
        _ => {}
    }
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
