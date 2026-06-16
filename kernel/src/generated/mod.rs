//! Generated artifacts embedded into the kernel.
//!
//! The contents of the per-program modules here are produced by
//! `cargo xtask build-programs` from the standalone `programs/*` crates.
//! They are committed (so a plain `cargo build` of the kernel works) and
//! regenerated whenever a program's source changes.

pub mod lineecho_elf;
