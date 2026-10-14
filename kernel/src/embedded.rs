//! The programs embedded in the kernel image — the run-time half of DESK43
//! decision D3 (`kernel/embed.rs` is the build-time half).
//!
//! `kernel/build.rs` generates [`EMBEDDED_PROGRAMS`] from the list in
//! `kernel/embed.rs`, one `include_bytes!` per program from the board's
//! `target/programs/<board>/` directory, and fails the build when a listed
//! program has not been built (`cargo xtask build-programs --board <board>`).
//! [`register`] hands every entry to the in-kernel program registry
//! (`tos_api::bdos::process::registry`), which `Pexec` consults before the
//! file system (`pexec.rs` `load`) — the place EmuTOS' ROM desktop has
//! (`aes_run_rom_program(deskstart)`, `aes/gemshlib.c:606`).
//!
//! Since DESK43 U9-2 the table holds the desktop program `NEWDESK`
//! (`programs/newdesk`), registered at boot; until U9-4 nothing starts it on
//! the boot path (the shell's DESKTOP arm still runs the built-in desktop).

use tos_api::bdos::process::registry;

use crate::kprintln;

include!(concat!(env!("OUT_DIR"), "/embedded_programs.rs"));

/// Register every embedded program under its registry name. Called once at
/// boot, before the AES starts (`kernel_main_rust`).
// NEWTOS: EmuTOS links its desktop into the ROM image; there is no registry.
pub fn register() {
    for &(name, blob) in EMBEDDED_PROGRAMS {
        if !registry::register(name, blob) {
            // `registry::MAX_PROGRAMS` is a build-time constant; a full table
            // is a build mistake, reported rather than hidden.
            kprintln!(
                "[embed] registry full: {} ({} bytes) not registered",
                core::str::from_utf8(name).unwrap_or("?"),
                blob.len()
            );
        }
    }
}
