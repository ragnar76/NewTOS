//! `NEWDESK.NAP` — the NewTOS desktop as an EL0 program (DESK43 U9-2).
//!
//! A thin entry around the `desktop` library: one source, two link targets
//! (decision D1). The kernel links the same library and calls `desktop::run()`
//! in EL1 only with `desk=kernel` or as the load-failure fallback; its DESKTOP
//! arm starts this program (DESK43 U9-4). This crate links it with the program
//! runtime (`newtos-gem` with `rt`) into a position-independent program,
//! built for each board by `cargo xtask build-programs` and embedded in the
//! kernel image under the registry name `NEWDESK` (decision D3,
//! `kernel/embed.rs`), as EmuTOS' desktop lies in the ROM
//! (`aes_run_rom_program(deskstart)`, `aes/gemshlib.c:606`).
//!
//! ## What `main` has to do — EmuTOS `_deskstart` (`desk/deskstart.S:26-59`)
//!
//! | `deskstart.S` | here |
//! |---|---|
//! | stack switch to `deskustack` (2560 bytes in `.bss`, `:15`), `Mshrink` to basepage + text + data + bss (`:28-41`) | `sdk/crt0.S`: `Mshrink` to image + `MINKEEP` (64 KiB) stack, SP at its top (the size is decision STK, after the U9-1 measurement) |
//! | `restart: jsr _deskmain; tst.w d0; jeq restart` (`:53-56`) | [`desktop::run`] (the same loop, *Read .INF file* restarts in the process) |
//! | `clr.w -(sp); trap #1` — `Pterm(0)` (`:58-59`) | `main` returns 0, crt0 `Pterm`s with it |
//!
//! The rest of what the kernel's DESKTOP arm does around its `desktop::run()`
//! call has no counterpart in the program:
//!
//! * `sh_show("")`, `p_setappdir` (`gemshlib.c:603-605`) are the **shell's**
//!   steps before the start; they stay in the kernel's `sh_ldapp`, as they
//!   run in EmuTOS's `sh_ldapp` before `aes_run_rom_program`.
//! * `desktop::reset()` stands in for the fresh process the ROM desktop gets
//!   every time (the kernel's desktop state outlives its call). A program *is*
//!   a fresh process: the loader zeroes `.bss` and copies `.data`, so the
//!   desktop's lazily built state starts empty — nothing to reset.
//! * The "desktop up" observation and its `DESKTOP_ACTIVE` flag are the
//!   kernel's (`kernel/src/shell_loop.rs`); it recognises this program by the
//!   shell child's slot (DESK43 U9-0c).
//!
//! **Crash policy (D6-a).** A Rust panic in the desktop shows its message and
//! waits for a key before ending with `Pterm(-1)` (`newtos-gem` feature
//! `panic-wait-key`, on for this program only), the counterpart of EmuTOS's
//! exception report (`bios/kprint.c:581-584`) — the key is the brake against
//! a desktop that panics at every start being restarted without pause.

#![no_std]

// The program runtime: the global allocator and the panic handler come with
// `newtos-gem`'s `rt` (the desktop library links it without `rt`); naming the
// crate keeps it linked.
use newtos_gem as _;

/// The program's entry, called by `sdk/crt0.S` with the basepage address
/// (unused: the desktop reaches everything through the traps). Runs the
/// desktop until it is done and returns `deskstart.S`'s `Pterm` code, 0.
#[no_mangle]
pub extern "C" fn main(_basepage: usize) -> i32 {
    desktop::run();
    0
}
