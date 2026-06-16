//! Hardware Abstraction Layer (HAL) — trait definitions.
//!
//! Each trait describes a category of hardware that the kernel expects.
//! Concrete drivers in `crate::drivers::*` implement these traits for a
//! specific board.

pub mod framebuffer;
pub mod uart;
