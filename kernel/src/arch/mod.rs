//! Architecture-specific support modules.
//!
//! Currently AArch64-only; the layer remains a single point of indirection
//! so that future ports (none planned) would slot in here.

pub mod aarch64;
