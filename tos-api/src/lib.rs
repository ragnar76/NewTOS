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

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

pub mod aes;
pub mod bdos;
pub mod bios;
pub mod cookie;
pub mod gemdos;
pub mod host;
pub mod trap;
pub mod vdi;
pub mod xbios;

pub use host::Host;
pub use trap::{TrapArgs, TrapClass};
