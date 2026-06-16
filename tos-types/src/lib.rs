//! Shared TOS-compatible types and constants.
//!
//! This crate is `#![no_std]` and contains only data definitions used
//! by both the kernel (`tos-api`, `kernel`) and host-side tooling.
//!
//! Phase 02 introduces the GEMDOS error codes and the integer width
//! aliases used throughout the TOS API.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

pub mod basepage;
pub mod error;
pub mod memory;

pub use basepage::{
    BasePage, NUMCURDIR, NUMSTD, PDCLSIZE, PF_FASTLOAD, PF_STANDARD, PF_TTRAMLOAD, PF_TTRAMMEM,
};
pub use memory::{MD, MPB, MX_MODEMASK, MX_PREFSTRAM, MX_PREFTTRAM, MX_STRAM, MX_TTRAM};

/// 16-bit signed integer, equivalent to TOS `WORD`.
pub type Word = i16;
/// 16-bit unsigned integer, equivalent to TOS `UWORD`.
pub type UWord = u16;
/// 32-bit signed integer, equivalent to TOS `LONG`.
pub type Long = i32;
/// 32-bit unsigned integer, equivalent to TOS `ULONG`.
pub type ULong = u32;
