//! `newtos-gem` — the runtime for NewTOS native Rust programs.
//!
//! DESK43 U6a (`docs/analysis/desk43-u5-u10-review-2026-10-14.md` §3.2,
//! toolchain T1 §5): a Rust program is a `no_std` + `alloc` `staticlib`,
//! linked with `sdk/crt0.S` and `sdk/newtos.ld` exactly like a C program
//! (`programs/build.sh`). crt0 does the TOS start-up (Mshrink to image +
//! 64 KiB stack, the accessory test, the PRGFLAGS note) and calls
//! `int main(BASEPAGE *)`; the program's `main` returns the exit code and
//! crt0 ends it with `Pterm`.
//!
//! This crate supplies what a Rust program needs on top of that:
//!
//! * [`gemdos`], [`aes`], [`vdi`] — trap bindings (`svc #1`, `svc #2`
//!   selectors 200 / 115), in the pattern of the kernel desktop's
//!   `kernel/src/desk/{dosbind,vdibind}.rs` and the AES part of
//!   `desk/mod.rs` (moving those here is U6b);
//! * [`heap`] — the global allocator, over GEMDOS `Malloc` hunks;
//! * the panic handler — the program ends with [`PANIC_EXIT`].
//!
//! Nothing here touches the kernel: every service is reached through the
//! trap, as for any TOS program.

#![no_std]

extern crate alloc;

pub mod aes;
pub mod gemdos;
pub mod heap;
pub mod vdi;

use core::fmt::Write;

/// The global allocator of every program built on this crate.
#[global_allocator]
pub static HEAP: heap::Heap = heap::Heap::new();

/// The exit code of a program that panicked: GEMDOS `ERROR` (−1), the
/// generic error. A panic is the program's own abort; the value is this
/// runtime's choice (TOS reserves no code for it), picked so the shell's
/// `[shell] sh_ldapp: ... exited with rc=-1` names it as a failure.
pub const PANIC_EXIT: i16 = gemdos::ERROR as i16;

/// A `core::fmt` sink into a fixed byte buffer that always keeps room for a
/// terminating NUL; output beyond the buffer is dropped.
pub struct FixedBuf<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> FixedBuf<N> {
    pub const fn new() -> Self {
        Self { buf: [0; N], len: 0 }
    }

    /// The text written so far (no NUL).
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    /// The text as a C string (the buffer always ends in a NUL).
    pub fn as_cstr(&self) -> &core::ffi::CStr {
        // The byte at `len` is still 0: `write_str` never fills the last
        // byte, and nothing after `len` is ever written.
        core::ffi::CStr::from_bytes_until_nul(&self.buf[..=self.len]).unwrap_or(c"")
    }
}

impl<const N: usize> Default for FixedBuf<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Write for FixedBuf<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &b in s.as_bytes() {
            // Keep the last byte for the NUL; an interior NUL would cut the
            // C string short, so it is replaced.
            if self.len + 1 >= N {
                break;
            }
            self.buf[self.len] = if b == 0 { b'?' } else { b };
            self.len += 1;
        }
        Ok(())
    }
}

/// Set while the panic handler runs: a panic inside it ends the program at
/// once.
static PANICKING: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// The panic handler: report the location and message on the console
/// (`Cconws`: the screen, and the serial line with `serial=on`), then end
/// the program with [`PANIC_EXIT`]. No allocation, no unwinding
/// (`panic = "abort"`). Windows and AES state the program leaves behind are
/// swept by the AES shell after the program ends (`sh_ldapp`, `wm_new`,
/// the missing-`appl_exit` path) as for any program that terminates early.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    if PANICKING.swap(true, core::sync::atomic::Ordering::Relaxed) {
        gemdos::pterm(PANIC_EXIT);
    }
    let mut m = FixedBuf::<256>::new();
    let _ = m.write_str("\r\n*** Rust panic");
    if let Some(loc) = info.location() {
        let _ = write!(m, " at {}:{}", loc.file(), loc.line());
    }
    let _ = write!(m, ": {}\r\n", info.message());
    gemdos::cconws(m.as_cstr());
    gemdos::pterm(PANIC_EXIT)
}
