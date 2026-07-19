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

/// A resolved on-disk program file, streamable by byte offset without
/// the SFT handle table. The program loader opens one of these and reads
/// each `PT_LOAD` segment's file range straight into the mapped TPA, so
/// there is no fixed intermediate buffer capping the program size.
///
/// Holds the mounted [`fat::Volume`], the file's first cluster and its
/// length — the minimum to walk the cluster chain for a range read.
#[derive(Clone, Copy)]
pub struct FileImage {
    vol: fat::Volume,
    start_cluster: u32,
    file_len: u32,
}

impl FileImage {
    /// Resolve `path` to a file on its drive. Returns `None` for a
    /// path/IO error or if the name is a directory (the loader only
    /// streams regular files).
    pub fn open(path: &str) -> Option<FileImage> {
        let (vol, parent, name11) = path::findit(path).ok()?;
        let entry = dir::find_in_dir(&vol, parent, &name11)?;
        if entry.attrib & 0x10 != 0 {
            return None; // a sub-directory, not a file
        }
        Some(FileImage {
            vol,
            start_cluster: entry.cluster,
            file_len: entry.size,
        })
    }

    /// File length in bytes (the ELF total image size).
    pub fn len(&self) -> u32 {
        self.file_len
    }

    /// `true` for a zero-length file (no cluster chain).
    pub fn is_empty(&self) -> bool {
        self.file_len == 0
    }

    /// Read up to `dst.len()` bytes starting at byte `offset` into `dst`,
    /// walking the cluster chain and copying sector by sector (so no
    /// whole-cluster or whole-file staging buffer is needed). Returns the
    /// number of bytes read — fewer than `dst.len()` only at end of file —
    /// or `None` on a block-device read error.
    ///
    /// Mirrors [`file::Fread`](super::file)'s cluster walk but against a
    /// standalone `(cluster, offset)` rather than an open SFT handle.
    pub fn read_at(&self, offset: u32, dst: &mut [u8]) -> Option<usize> {
        let vol = &self.vol;
        let clsizb = vol.clsizb;
        let recsiz = vol.recsiz as usize;
        if offset >= self.file_len {
            return Some(0);
        }
        let want = dst.len().min((self.file_len - offset) as usize);
        if want == 0 {
            return Some(0);
        }

        // Walk to the cluster holding `offset`.
        let mut cluster = self.start_cluster;
        for _ in 0..(offset / clsizb) {
            match vol.fat_entry(cluster) {
                Some(n) if !vol.is_eoc(n) => cluster = n,
                _ => return Some(0), // chain ended before `offset`
            }
        }

        let mut byte_in_cluster = offset % clsizb;
        let mut done = 0usize;
        let mut sbuf = [0u8; 512];
        while done < want && vol.is_data_cluster(cluster) {
            while byte_in_cluster < clsizb && done < want {
                let sec_in_cl = byte_in_cluster / recsiz as u32;
                let off_in_sec = (byte_in_cluster % recsiz as u32) as usize;
                if !vol.read_cluster_sector(cluster, sec_in_cl, &mut sbuf) {
                    return None;
                }
                let n = (recsiz - off_in_sec)
                    .min(want - done)
                    .min((clsizb - byte_in_cluster) as usize);
                dst[done..done + n].copy_from_slice(&sbuf[off_in_sec..off_in_sec + n]);
                done += n;
                byte_in_cluster += n as u32;
            }
            byte_in_cluster = 0;
            match vol.fat_entry(cluster) {
                Some(nx) if !vol.is_eoc(nx) => cluster = nx,
                _ => break,
            }
        }
        Some(done)
    }
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
