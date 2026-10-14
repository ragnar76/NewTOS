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
//! * [`gemdos`], [`aes`], [`vdi`], [`xbios`] — the trap bindings (`svc #1`,
//!   `svc #2` selectors 200 / 115, `svc #14`). Since DESK43 U6b they are the
//!   **only** copy: the kernel desktop (`programs/desktop/src/`, still EL1 until
//!   U9) calls the system through them as well;
//! * [`heap`] — the global allocator, over GEMDOS `Malloc` hunks (feature
//!   `rt`);
//! * the panic handler — the program ends with [`PANIC_EXIT`] (feature `rt`;
//!   with feature `panic-wait-key` after a key, DESK43 U9 D6-a — the NEWDESK
//!   program's choice).
//!
//! **Feature `rt`** (default on) is the program runtime proper: the
//! `#[global_allocator]` and the `#[panic_handler]`. A program needs both; the
//! kernel, which links this crate for the bindings only, has its own and turns
//! the feature off (`default-features = false` in `kernel/Cargo.toml`).
//!
//! Nothing here touches the kernel: every service is reached through the
//! trap, as for any TOS program.

#![no_std]

extern crate alloc;

pub mod aes;
pub mod gemdos;
#[cfg(feature = "rt")]
pub mod heap;
pub mod vdi;
pub mod xbios;

use core::fmt::Write;

/// The global allocator of every program built on this crate.
#[cfg(feature = "rt")]
#[global_allocator]
pub static HEAP: heap::Heap = heap::Heap::new();

/// The exit code of a program that panicked: GEMDOS `ERROR` (−1), the generic
/// error — this runtime's choice (TOS reserves no code for it). The parent gets
/// the `UWORD` zero-extended (EmuTOS `xterm`, `bdos/proc.c:601,616`): 65535, as
/// the shell's `[shell] sh_ldapp: ... exited with rc=65535` line shows (PROC05).
pub const PANIC_EXIT: i16 = tos_types::error::ERROR as i16;

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
#[cfg(feature = "rt")]
static PANICKING: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// The panic handler: report the location and message on the console
/// (`Cconws`: the screen, and the serial line with `serial=on`), then end
/// the program with [`PANIC_EXIT`]. No allocation, no unwinding
/// (`panic = "abort"`). Windows and AES state the program leaves behind are
/// swept by the AES shell after the program ends (`sh_ldapp`, `wm_new`,
/// the missing-`appl_exit` path) as for any program that terminates early.
///
/// With the feature `panic-wait-key` the handler first waits for a key
/// (`wait_key`), as EmuTOS's exception report does before it ends the
/// program (`bios/kprint.c:581-584`).
#[cfg(feature = "rt")]
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
    gemdos::dos_conws(m.as_cstr());
    #[cfg(feature = "panic-wait-key")]
    wait_key();
    gemdos::pterm(PANIC_EXIT)
}

/// Feature `panic-wait-key` (DESK43 U9, crash policy D6-a): the tail of
/// EmuTOS's `dopanic` (`bios/kprint.c:581-585`) before `kill_program`, through
/// the GEMDOS counterparts of its BIOS calls on the console device —
/// `Cconis`/`Crawcin` read the console like `bconstat2`/`bconin2`, without
/// echo and without Ctrl-C handling (`docs/api/gemdos/console/07-crawcin.md`):
///
/// ```c
/// while(bconstat2())      /* eat any pending ones */
///     bconin2();
/// cprintf(_("\n*** Press any key to continue ***"));
/// bconin2();
/// cprintf("\n");
/// ```
///
/// The prompt is EmuTOS's English text (`cprintf` turns `\n` into CR LF);
/// a program has no access to the kernel's translated copy.
// NEWTOS: EmuTOS has no Rust panic; this gives one the brake its exception
// report has (docs/analysis/desk43-u9-prep-2026-10-14.md §1.3 D6-2).
#[cfg(feature = "panic-wait-key")]
fn wait_key() {
    while gemdos::cconis() != 0 {
        gemdos::crawcin();
    }
    gemdos::dos_conws(c"\r\n*** Press any key to continue ***");
    gemdos::crawcin();
    gemdos::dos_conws(c"\r\n");
}
