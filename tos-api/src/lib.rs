//! NewTOS TOS-API surface.
//!
//! This crate hosts the TOS syscall dispatchers (GEMDOS / BIOS / XBIOS)
//! and the function implementations that operate on architecture-
//! independent abstractions provided by the kernel through the
//! [`Host`] trait.
//!
//! The kernel installs a `&'static dyn Host` once at boot and then
//! routes synchronous-from-EL0 SVC traps into [`gemdos::dispatch`],
//! [`bios::dispatch`] and [`xbios::dispatch`] (the latter two are
//! introduced in PHASE-07).

// `no_std` for every real build; the host test harness needs `std` for its
// runner, allocator and panic handler. Only `cargo test -p tos-api` sees
// `std` -- the kernel builds this crate as a dependency, never as a test, so
// the freestanding build is unaffected.
#![cfg_attr(not(test), no_std)]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

extern crate alloc;

pub mod aes;
pub mod bdos;
pub mod bios;
pub mod build;
pub mod cookie;
pub mod gemdos;
pub mod i18n;
pub mod host;
pub mod trap;
pub mod uaccess;
pub mod vdi;
pub mod xbios;

pub use host::Host;
pub use trap::{TrapArgs, TrapClass};
