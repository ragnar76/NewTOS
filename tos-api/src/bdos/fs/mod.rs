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

/// Read a whole file at `path` into `out` (truncated to `out.len()`).
/// Returns the number of bytes read, or `None` on a path/IO error or if
/// the name is a directory. Used by the program loader to pull a `.PRG`
/// off the FAT16 file system without going through the SFT handle table.
pub fn read_file(path: &str, out: &mut [u8]) -> Option<usize> {
    let (vol, parent, name11) = path::findit(path).ok()?;
    let entry = dir::find_in_dir(&vol, parent, &name11)?;
    if entry.attrib & 0x10 != 0 {
        return None; // a sub-directory, not a file
    }
    let want = (entry.size as usize).min(out.len());
    let recsiz = vol.recsiz as usize;
    let mut cluster = entry.cluster;
    let mut done = 0;
    let mut sbuf = [0u8; 512];
    while done < want && vol.is_data_cluster(cluster) {
        for sec in 0..vol.clsiz {
            if done >= want {
                break;
            }
            if !vol.read_cluster_sector(cluster, sec, &mut sbuf) {
                return None;
            }
            let n = recsiz.min(want - done);
            out[done..done + n].copy_from_slice(&sbuf[..n]);
            done += n;
        }
        match vol.fat_entry(cluster) {
            Some(nx) if !vol.is_eoc(nx) => cluster = nx,
            _ => break,
        }
    }
    Some(done)
}
