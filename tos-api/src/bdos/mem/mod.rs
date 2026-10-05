//! BDOS memory subsystem.
//!
//! Implements the EmuTOS memory layer: `MD`/`MPB` data structures,
//! the `ffit` first-fit allocator, the `freeit` coalescing free,
//! `shrinkit`, and the `xmgetmd`/`xmfremd` MD meta-allocator.
//! GEMDOS trap entries (`Maddalt`, `Mxalloc`, `Malloc`, `Mfree`,
//! `Mshrink`) wrap these primitives.
//!
//! Specs:
//! - [`docs/api/structures/md.md`](../../../../docs/api/structures/md.md)
//! - [`docs/api/gemdos/memory/14-maddalt.md`](../../../../docs/api/gemdos/memory/14-maddalt.md)
//! - [`docs/api/gemdos/memory/44-mxalloc.md`](../../../../docs/api/gemdos/memory/44-mxalloc.md)
//! - [`docs/api/gemdos/memory/48-malloc.md`](../../../../docs/api/gemdos/memory/48-malloc.md)
//! - [`docs/api/gemdos/memory/49-mfree.md`](../../../../docs/api/gemdos/memory/49-mfree.md)
//! - [`docs/api/gemdos/memory/4a-mshrink.md`](../../../../docs/api/gemdos/memory/4a-mshrink.md)
//!
//! The submodules are populated phase-by-phase:
//!
//! | Module     | Deliverable | Content                            |
//! |------------|-------------|------------------------------------|
//! | `pool`     | D2..D4      | `ffit`, `freeit`, `shrinkit`       |
//! | `mdpool`   | D5          | `xmgetmd` / `xmfremd` (`MDBlock`)  |
//! | `init`     | D6          | Pool init from `bios::machine`     |
//! | `traps`    | D7..D12     | `Maddalt` / `Mxalloc` / `Malloc` / `Mfree` / `Mshrink` + owner attribution |
//! | `stats`    | D13         | Per-pool diagnostics               |
//! | `region`   | ARCH03 2d   | Further owners of a block (links)  |
//!
//! Host-side unit tests for the pure pool logic are added in a
//! separate crate when D2 lands — `tos-api` itself is
//! `forced-target = aarch64-unknown-none-softfloat`, so direct
//! `cargo test` against the kernel target is not viable.

pub mod free_all_owned;
pub mod init;
pub mod mdpool;
pub mod pool;
pub mod region;
pub mod stats;
pub mod traps;
