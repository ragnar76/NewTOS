//! HID boot keyboard / mouse reports → the IKBD byte stream.
//!
//! Translate per `docs/api/usb/hid-keyboard.md` / `hid-mouse.md`: turn USB
//! HID boot reports into the exact bytes a real Atari IKBD would send and push
//! them through [`crate::ikbd::feed`], reusing the whole existing decoder
//! (scancode→ASCII→console, relative-mouse→VDI cursor).

use core::sync::atomic::{AtomicU64, Ordering};

use crate::ikbd;

/// USB HID usage → Atari scancode (make code; break = `| 0x80`).
/// `translate_key` in FreeMiNT `udd/hid/keyboard/usb_keyboard.c`.
fn translate_key(usb: u8) -> u8 {
    match usb {
        0x04 => 0x1e, 0x05 => 0x30, 0x06 => 0x2e, 0x07 => 0x20, 0x08 => 0x12,
        0x09 => 0x21, 0x0a => 0x22, 0x0b => 0x23, 0x0c => 0x17, 0x0d => 0x24,
        0x0e => 0x25, 0x0f => 0x26, 0x10 => 0x32, 0x11 => 0x31, 0x12 => 0x18,
        0x13 => 0x19, 0x14 => 0x10, 0x15 => 0x13, 0x16 => 0x1f, 0x17 => 0x14,
        0x18 => 0x16, 0x19 => 0x2f, 0x1a => 0x11, 0x1b => 0x2d, 0x1c => 0x15,
        0x1d => 0x2c, // Z
        0x1e => 0x02, 0x1f => 0x03, 0x20 => 0x04, 0x21 => 0x05, 0x22 => 0x06,
        0x23 => 0x07, 0x24 => 0x08, 0x25 => 0x09, 0x26 => 0x0a, 0x27 => 0x0b, // 0
        0x28 => 0x1c, // Return
        0x29 => 0x01, // Esc
        0x2a => 0x0e, // Backspace
        0x2b => 0x0f, // Tab
        0x2c => 0x39, // Space
        0x2d => 0x0c, 0x2e => 0x0d, 0x2f => 0x1a, 0x30 => 0x1b, 0x31 => 0x2b,
        0x32 => 0x2b, 0x33 => 0x27, 0x34 => 0x28, 0x35 => 0x29, 0x36 => 0x33,
        0x37 => 0x34, 0x38 => 0x35, 0x39 => 0x3a, // Caps Lock
        0x3a => 0x3b, 0x3b => 0x3c, 0x3c => 0x3d, 0x3d => 0x3e, 0x3e => 0x3f,
        0x3f => 0x40, 0x40 => 0x41, 0x41 => 0x42, 0x42 => 0x43, 0x43 => 0x44, // F1..F10
        0x44 => 0x62, 0x45 => 0x61, // F11→Help, F12→Undo
        0x47 => 0x63, 0x48 => 0x64, 0x49 => 0x52, 0x4a => 0x47, 0x4c => 0x53,
        0x4f => 0x4d, 0x50 => 0x4b, 0x51 => 0x50, 0x52 => 0x48, // arrows
        0x54 => 0x65, 0x55 => 0x66, 0x56 => 0x4a, 0x57 => 0x4e, 0x58 => 0x72,
        0x59 => 0x6d, 0x5a => 0x6e, 0x5b => 0x6f, 0x5c => 0x6a, 0x5d => 0x6b,
        0x5e => 0x6c, 0x5f => 0x67, 0x60 => 0x68, 0x61 => 0x69, 0x62 => 0x70,
        0x63 => 0x71, 0x64 => 0x60,
        _ => 0,
    }
}

/// Previous keyboard report (byte 0 = modifiers, bytes 2..8 = 6 keycodes),
/// packed into a u64 so the diff is allocation-free.
static PREV: AtomicU64 = AtomicU64::new(0);

/// Emit make/break scancodes for the changed modifier bits (`off` = 0 make,
/// 0x80 break). USB byte-0 bits: 0 LCtrl 1 LShift 2 LAlt … 4 RCtrl 5 RShift
/// 6 RAlt. Atari: Ctrl 0x1d, LShift 0x2a, RShift 0x36, Alt 0x38.
fn modifiers(bits: u8, off: u8) {
    if bits & 0x11 != 0 {
        ikbd::feed(0x1d | off);
    }
    if bits & 0x02 != 0 {
        ikbd::feed(0x2a | off);
    }
    if bits & 0x20 != 0 {
        ikbd::feed(0x36 | off);
    }
    if bits & 0x44 != 0 {
        ikbd::feed(0x38 | off);
    }
}

/// Process one 8-byte boot keyboard report: diff against the previous report
/// and feed make/break scancodes into the IKBD decoder.
pub fn keyboard_report(r: &[u8; 8]) {
    let modn = r[0];
    let prev = PREV.load(Ordering::Relaxed).to_le_bytes();
    let oldmod = prev[0];

    modifiers(oldmod & !modn, 0x80); // released modifiers
    modifiers(modn & !oldmod, 0x00); // pressed modifiers

    let oldkeys = &prev[2..8];
    let newkeys = &r[2..8];
    for &k in oldkeys {
        if k != 0 && !newkeys.contains(&k) {
            let s = translate_key(k);
            if s != 0 {
                ikbd::feed(s | 0x80); // break
            }
        }
    }
    for &k in newkeys {
        if k != 0 && !oldkeys.contains(&k) {
            let s = translate_key(k);
            if s != 0 {
                ikbd::feed(s); // make
            }
        }
    }

    let mut p = [0u8; 8];
    p[0] = modn;
    p[2..8].copy_from_slice(newkeys);
    PREV.store(u64::from_le_bytes(p), Ordering::Relaxed);
}

/// Process one boot mouse report (byte 0 = buttons, 1 = dx, 2 = dy) → an IKBD
/// relative-mouse packet `[0xF8|wire, dx, dy]` fed to the decoder. USB buttons
/// (bit0 left, bit1 right) are swapped into IKBD wire order (bit0 right, bit1
/// left); the decoder swaps back to VDI.
pub fn mouse_report(r: &[u8; 8]) {
    let usb = r[0];
    let wire = ((usb & 0x01) << 1) | ((usb & 0x02) >> 1);
    ikbd::feed(0xF8 | (wire & 0x03));
    ikbd::feed(r[1]); // dx
    ikbd::feed(r[2]); // dy (the IKBD decoder owns the screen-Y convention)
}
