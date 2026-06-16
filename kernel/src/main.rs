//! Binary shim. The real entry point is `kernel::kernel_main`, exported by
//! the library crate and invoked from the AArch64 boot stub.
//!
//! Cargo requires a `[[bin]]` to produce a linked artifact; this file is
//! that artifact's root. We pull the library in with `extern crate` so its
//! `_start` (in `arch::aarch64::boot`) and `kernel_main` symbols end up in
//! the final binary.

#![no_std]
#![no_main]

extern crate kernel;
