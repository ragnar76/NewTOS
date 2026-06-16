//! Machine-level memory management (Phase 03).
//!
//! Layered, bottom-up:
//!   1. [`layout`] — physical memory map and ST-RAM/TT-RAM pool
//!      boundaries (D1, D8).
//!   2. [`frames`] — physical frame allocator (D4).
//!   3. [`heap`]   — kernel heap on top of frames (D5).
//!
//! These were originally placed under `crate::mm` while the BIOS
//! layer did not yet exist. The Phase 03 D9 refactor moves them here
//! to make ownership explicit: this is the BIOS's view of the
//! physical machine.

pub mod frames;
pub mod heap;
pub mod layout;
