//! BDOS file system (Phase 09).
//!
//! Layered after EmuTOS `bdos/`: [`fat`] is the FAT-table / cluster-chain
//! reader (mirrors `fsfat.c`) — the foundation the directory traversal
//! and the file handles build on. Read-only in D1; the sector cache
//! (D2), path parser (D3) and the GEMDOS file/dir calls follow.

pub mod cache;
pub mod dir;
pub mod dirops;
pub mod drive;
pub mod fat;
pub mod file;
pub mod find;
pub mod path;

/// Read a NUL-terminated path string (max 128 bytes) from a raw pointer.
///
/// # Safety
/// `ptr` must point to a readable NUL-terminated string. EL1 self-tests
/// pass kernel pointers; the EL0 user-pointer path lands with the
/// process-aware file layer.
pub(crate) unsafe fn cstr<'a>(ptr: u64) -> Option<&'a str> {
    if ptr == 0 {
        return None;
    }
    let p = ptr as *const u8;
    let mut len = 0usize;
    while len < 128 && unsafe { *p.add(len) } != 0 {
        len += 1;
    }
    core::str::from_utf8(unsafe { core::slice::from_raw_parts(p, len) }).ok()
}
