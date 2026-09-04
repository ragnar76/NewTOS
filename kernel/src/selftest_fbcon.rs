//! Phase 10 D5 demo: activate the framebuffer console and print through
//! the real GEMDOS `Cconout` path (svc #1), demonstrating that console
//! output renders as glyphs on the HDMI display (EC1).
//!
//! Output uses `Cconout` (the character travels in a register) rather
//! than `Cconws` (which reads a NUL-terminated string with the
//! unprivileged `ldtrb` load and therefore needs an EL0-accessible
//! pointer — a kernel string would fault).

/// Clear the framebuffer console and render a banner via `Cconout`.
pub fn run() {
    if !crate::vdi_backend::available() {
        return;
    }
    crate::fbcon::clear_and_home();
    puts(b"NewTOS - framebuffer console (VT52)\r\n");
    puts(b"-----------------------------------\r\n");
    puts(b"GEMDOS Cconout now renders on HDMI.\r\n");
    puts(b"\r\n");
    puts(b"Drives mounted: C: D: E: F:\r\n");
    puts(b"Filesystem:     FAT16 (read + write)\r\n");
    puts(b"\r\n");
    puts(b"C:\\> _\r\n");
}

/// Write a byte string through GEMDOS `Cconout` (one trap per byte).
fn puts(s: &[u8]) {
    for &b in s {
        cconout(b);
    }
}

/// GEMDOS `Cconout` (0x02) via `svc #1`: x0 = character.
fn cconout(c: u8) {
    // SAFETY: svc #1, GEMDOS Cconout ABI; the char travels in x0.
    unsafe {
        core::arch::asm!("svc #1", in("x8") 0x02u64, inout("x0") c as u64 => _, options(nostack));
    }
}
