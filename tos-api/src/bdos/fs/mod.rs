//! BDOS file system (Phase 09).
//!
//! Layered after EmuTOS `bdos/`: [`fat`] is the FAT-table / cluster-chain
//! reader (mirrors `fsfat.c`) — the foundation the directory traversal
//! and the file handles build on. Read-only in D1; the sector cache
//! (D2), path parser (D3) and the GEMDOS file/dir calls follow.

pub mod fat;
