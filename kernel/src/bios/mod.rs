//! BIOS / machine-abstraction layer.
//!
//! In the original Atari TOS architecture, the BIOS sat between the
//! GEMDOS file/process layer and the hardware: memory map, console
//! device routing, block-device probing, RTC, and the bootstrap of
//! the application image all lived here. NewTOS keeps that split.
//!
//! Phase 03 introduces the `machine` sub-module, which owns the
//! physical memory layout, the frame allocator, and the kernel heap.
//! Earlier phases (01/02) parked these under `crate::mm`; the move
//! into `bios::machine` reflects their actual responsibility — they
//! describe the *machine* the BIOS sits on, not a generic memory
//! manager.
//!
//! Future phases will add:
//!   * `bios::console` (Phase 07) — Bconin/Bconout/Bconstat/Bconmap.
//!   * `bios::devices` (Phase 04/07) — block device + RTC.
//!   * `bios::bootstrap` — image-load and device-probing wiring.

pub mod machine;
