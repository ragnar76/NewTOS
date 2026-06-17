//! BDOS layer.
//!
//! Despite the name, BDOS in TOS is what GEMDOS calls into for
//! anything beyond pure character I/O — file system, process
//! management, and the memory pools live here. Phase 04 introduces
//! the memory subsystem; later phases extend BDOS with FAT and
//! process tables.
//!
//! GEMDOS trap entries (`tos-api::gemdos`) are the public ABI;
//! they delegate to the implementation modules in this `bdos`
//! tree. Splitting it this way keeps the trap dispatcher free of
//! algorithmic logic and lets the BDOS layer be unit-tested on the
//! host without an SVC path.

pub mod fs;
pub mod mem;
pub mod process;
