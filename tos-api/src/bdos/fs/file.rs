//! GEMDOS read-side file API: `Fopen` / `Fread` / `Fseek` / `Fclose`.
//!
//! Spec: `docs/api/gemdos/file/{3d-fopen,3f-fread,42-fseek,3e-fclose}.md`.
//! Mirrors the EmuTOS `bdos/fsopnclo.c` + `bdos/fsio.c` model in reduced
//! form: a global open-file table (the SFT) holding one [`Ofd`] per open
//! handle. Handles `>= NUMSTD` (6) index this table; standard handles
//! `0..5` and the per-process handle map arrive with the process model.
//!
//! Read-only / root-directory only for this deliverable: paths resolve a
//! drive letter and a single 8.3 component in the volume root (sub-
//! directory `findit` walking lands with `Dsetpath`).

use core::cell::UnsafeCell;

use super::dir;
use super::fat::Volume;
use crate::trap::TrapArgs;
use tos_types::error::{
    EACCDN, EDRIVE, EFILNF, EIHNDL, EINVFN, ENHNDL, EPTHNF, ERANGE, E_OK,
};
use tos_types::Long;

const SECTOR: usize = 512;
/// Number of reserved standard handles (0..5); first real handle is 6.
const NUMSTD: u16 = 6;
/// Open-file table size (EmuTOS `OPNFILES`).
const OPEN_FILES: usize = 16;
/// `FA_RO` read-only attribute bit.
const FA_RO: u8 = 0x01;

/// Open File Descriptor — reduced EmuTOS `OFD`.
#[derive(Clone, Copy)]
struct Ofd {
    vol: Volume,
    /// Drive the file lives on (recorded for future open-file tracking).
    #[allow(dead_code)]
    drive: u16,
    /// Raw root-directory slot of this file's FCB (for size/cluster
    /// write-back on close).
    dir_slot: u32,
    /// First cluster of the file (0 ⇒ empty file, no clusters yet).
    start_cluster: u32,
    /// File length in bytes.
    file_len: u32,
    /// Current byte offset (`o_bytnum`).
    pos: u32,
    /// Open mode bits (`mod & 0x77`). Sharing bits are recorded, not
    /// enforced, per the TOS contract.
    #[allow(dead_code)]
    mode: u8,
    /// The directory entry needs a flush (size / start cluster changed).
    dirty: bool,
}

struct FileTable(UnsafeCell<[Option<Ofd>; OPEN_FILES]>);
// SAFETY: accessed only from the single-CPU, IRQ-off trap path.
unsafe impl Sync for FileTable {}
static FILES: FileTable = FileTable(UnsafeCell::new([None; OPEN_FILES]));

fn table() -> &'static mut [Option<Ofd>; OPEN_FILES] {
    // SAFETY: single trap path; never concurrently borrowed.
    unsafe { &mut *FILES.0.get() }
}

/// Resolve a handle (`>= NUMSTD`) to its open-file slot.
fn ofd_of(handle: u16) -> Option<&'static mut Ofd> {
    if handle < NUMSTD {
        return None;
    }
    let slot = (handle - NUMSTD) as usize;
    table().get_mut(slot)?.as_mut()
}

/// Parse a path into `(drive, 8.3-name)`. Accepts an optional `X:` drive
/// prefix and `/` or `\` separators; resolves the final path component as
/// the file name in the volume root.
fn parse_path(path: &str) -> Option<(u16, [u8; 11])> {
    let mut s = path;
    let mut drive = super::drive::current_drive();
    let b = s.as_bytes();
    if b.len() >= 2 && b[1] == b':' {
        let c = b[0].to_ascii_uppercase();
        if !c.is_ascii_uppercase() {
            return None;
        }
        drive = (c - b'A') as u16;
        s = &s[2..];
    }
    // Last path component (root-directory lookup only for now).
    let comp = s.rsplit(['/', '\\']).next().unwrap_or(s);
    if comp.is_empty() {
        return None;
    }
    Some((drive, dir::fcb_name(comp)?))
}

/// `Fopen` (0x3D) — open an existing file for reading/writing.
pub fn fopen(args: &TrapArgs) -> Long {
    // SAFETY: caller-supplied path pointer (kernel pointer at EL1).
    let path = match unsafe { super::cstr(args.arg_u64(0)) } {
        Some(p) => p,
        None => return EFILNF,
    };
    let mode = (args.arg_u16(1) & 0x77) as u8;

    let (drive, name11) = match parse_path(path) {
        Some(x) => x,
        None => return EPTHNF,
    };
    let vol = match Volume::mount(drive) {
        Some(v) => v,
        None => return EDRIVE,
    };
    let entry = match dir::find_in_root(&vol, &name11) {
        Some(e) => e,
        None => return EFILNF,
    };
    // Read-only files reject any write access mode.
    if entry.attrib & FA_RO != 0 && (mode & 0x07) != 0 {
        return EACCDN;
    }
    let dir_slot = match dir::slot_of(&vol, &name11) {
        Some(s) => s,
        None => return EFILNF,
    };

    let slot = match table().iter().position(Option::is_none) {
        Some(s) => s,
        None => return ENHNDL,
    };
    table()[slot] = Some(Ofd {
        vol,
        drive,
        dir_slot,
        start_cluster: entry.cluster,
        file_len: entry.size,
        pos: 0,
        mode,
        dirty: false,
    });
    (slot as u16 + NUMSTD) as Long
}

/// `Fcreate` (0x3C) — create (or truncate) a file and open it for
/// writing. Root-directory only.
pub fn fcreate(args: &TrapArgs) -> Long {
    // SAFETY: caller-supplied path pointer (kernel pointer at EL1).
    let path = match unsafe { super::cstr(args.arg_u64(0)) } {
        Some(p) => p,
        None => return EPTHNF,
    };
    // Mask off the sub-directory bit — Fcreate cannot make folders.
    let attr = (args.arg_u16(1) as u8) & !0x10;

    let (drive, name11) = match parse_path(path) {
        Some(x) => x,
        None => return EPTHNF,
    };
    let vol = match Volume::mount(drive) {
        Some(v) => v,
        None => return EDRIVE,
    };
    if !vol.is_writable() {
        return EACCDN;
    }

    // If the file already exists, delete it first (unless RO / folder).
    if let Some(existing) = dir::find_in_root(&vol, &name11) {
        if existing.attrib & (FA_RO | 0x10) != 0 {
            return EACCDN;
        }
        if let Some(s) = dir::slot_of(&vol, &name11) {
            erase_entry(&vol, s, existing.cluster);
        }
    }

    let dir_slot = match dir::find_free_slot(&vol) {
        Some(s) => s,
        None => return EACCDN, // fixed root directory full
    };
    let (time, date) = crate::gemdos::time::dos_now();
    let fcb = dir::build_fcb(&name11, attr, 0, 0, time, date);
    if !dir::write_slot(&vol, dir_slot, &fcb) {
        return EACCDN;
    }
    super::cache::flush();

    let slot = match table().iter().position(Option::is_none) {
        Some(s) => s,
        None => return ENHNDL,
    };
    table()[slot] = Some(Ofd {
        vol,
        drive,
        dir_slot,
        start_cluster: 0,
        file_len: 0,
        pos: 0,
        mode: 0x01, // write access
        dirty: false,
    });
    (slot as u16 + NUMSTD) as Long
}

/// `Fread` (0x3F) — read `len` bytes from a handle into a buffer.
pub fn fread(args: &TrapArgs) -> Long {
    let handle = args.arg_u16(0);
    let len = args.arg_i32(1);
    let ubuf = args.arg_u64(2);

    let ofd = match ofd_of(handle) {
        Some(o) => o,
        None => return EIHNDL,
    };
    if len <= 0 {
        return 0;
    }
    let want = (len as u32).min(ofd.file_len.saturating_sub(ofd.pos));
    if want == 0 {
        return 0;
    }

    let vol = ofd.vol;
    let clsizb = vol.clsizb;
    let recsiz = vol.recsiz;

    // Walk to the cluster containing the current position.
    let mut cluster = ofd.start_cluster;
    let skip = ofd.pos / clsizb;
    for _ in 0..skip {
        match vol.fat_entry(cluster) {
            Some(n) if !vol.is_eoc(n) => cluster = n,
            _ => return 0, // chain ended early
        }
    }

    let dst = ubuf as *mut u8;
    let mut byte_in_cluster = ofd.pos % clsizb;
    let mut done = 0u32;
    let mut sbuf = [0u8; SECTOR];

    while done < want && vol.is_data_cluster(cluster) {
        while byte_in_cluster < clsizb && done < want {
            let sec_in_cl = byte_in_cluster / recsiz;
            let off_in_sec = byte_in_cluster % recsiz;
            if !vol.read_cluster_sector(cluster, sec_in_cl, &mut sbuf) {
                break;
            }
            let avail = recsiz - off_in_sec;
            let n = avail.min(want - done).min(clsizb - byte_in_cluster);
            // SAFETY: `dst..dst+want` is the caller's buffer; `n` bytes fit.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    sbuf.as_ptr().add(off_in_sec as usize),
                    dst.add(done as usize),
                    n as usize,
                );
            }
            done += n;
            byte_in_cluster += n;
        }
        byte_in_cluster = 0;
        match vol.fat_entry(cluster) {
            Some(nx) if !vol.is_eoc(nx) => cluster = nx,
            _ => break,
        }
    }

    ofd.pos += done;
    done as Long
}

/// `Fwrite` (0x40) — write `len` bytes from a buffer to a handle,
/// extending the file (allocating clusters) as needed.
pub fn fwrite(args: &TrapArgs) -> Long {
    let handle = args.arg_u16(0);
    let len = args.arg_i32(1);
    let ubuf = args.arg_u64(2);

    let ofd = match ofd_of(handle) {
        Some(o) => o,
        None => return EIHNDL,
    };
    if len <= 0 {
        return 0;
    }
    let vol = ofd.vol;
    if !vol.is_writable() {
        return EACCDN;
    }
    let clsizb = vol.clsizb;
    let recsiz = vol.recsiz;
    let want = len as u32;

    let src = ubuf as *const u8;
    let mut done = 0u32;
    let mut sbuf = [0u8; SECTOR];

    while done < want {
        let pos = ofd.pos + done;
        let cl_index = pos / clsizb; // which cluster of the file

        // Resolve (allocating/linking as needed) the cluster holding pos.
        let cluster = match resolve_cluster(ofd, cl_index) {
            Some(c) => c,
            None => break, // disk full → short write
        };

        let byte_in_cluster = pos % clsizb;
        let sec_in_cl = byte_in_cluster / recsiz;
        let off_in_sec = (byte_in_cluster % recsiz) as usize;
        let avail = recsiz - byte_in_cluster % recsiz;
        let n = avail.min(want - done) as usize;

        // Read-modify-write the target sector. A full, aligned sector
        // overwrite needs no prior read (the copy below fills all 512
        // bytes); a partial write reads first, starting from zero if the
        // sector is past EOF and not yet allocated.
        let full_sector = off_in_sec == 0 && n == recsiz as usize;
        if !full_sector {
            if !vol.read_cluster_sector(cluster, sec_in_cl, &mut sbuf) {
                sbuf = [0u8; SECTOR];
            }
        } else {
            sbuf = [0u8; SECTOR];
        }
        // SAFETY: `src..src+want` is the caller's buffer.
        unsafe {
            core::ptr::copy_nonoverlapping(src.add(done as usize), sbuf.as_mut_ptr().add(off_in_sec), n);
        }
        if !vol.write_cluster_sector(cluster, sec_in_cl, &sbuf) {
            break;
        }
        done += n as u32;
    }

    ofd.pos += done;
    if ofd.pos > ofd.file_len {
        ofd.file_len = ofd.pos;
    }
    if done > 0 {
        ofd.dirty = true;
    }
    super::cache::flush();
    done as Long
}

/// Resolve the `index`-th cluster of the open file, allocating and
/// linking new clusters when writing past the current end of chain.
/// Updates `start_cluster` for a previously empty file.
fn resolve_cluster(ofd: &mut Ofd, index: u32) -> Option<u32> {
    let vol = ofd.vol;
    if ofd.start_cluster == 0 {
        // File has no clusters yet: allocate the first.
        let c = vol.alloc_cluster()?;
        ofd.start_cluster = c;
        ofd.dirty = true;
    }
    let mut cluster = ofd.start_cluster;
    for _ in 0..index {
        match vol.fat_entry(cluster) {
            Some(n) if !vol.is_eoc(n) && vol.is_data_cluster(n) => cluster = n,
            _ => {
                // End of chain: allocate and link a new cluster.
                let new = vol.alloc_cluster()?;
                if !vol.set_fat_entry(cluster, new) {
                    return None;
                }
                cluster = new;
            }
        }
    }
    Some(cluster)
}

/// Free a file's cluster chain and mark its directory entry erased.
fn erase_entry(vol: &Volume, slot: u32, start_cluster: u32) {
    if vol.is_data_cluster(start_cluster) {
        vol.free_chain(start_cluster);
    }
    let mut e = [0u8; 32];
    if dir::read_slot(vol, slot, &mut e) {
        e[0] = 0xE5; // erased marker
        dir::write_slot(vol, slot, &e);
    }
}

/// Flush an open file's directory entry (size + start cluster).
fn flush_ofd(ofd: &Ofd) {
    let mut e = [0u8; 32];
    if dir::read_slot(&ofd.vol, ofd.dir_slot, &mut e) {
        e[0x1A..0x1C].copy_from_slice(&((ofd.start_cluster & 0xFFFF) as u16).to_le_bytes());
        e[0x1C..0x20].copy_from_slice(&ofd.file_len.to_le_bytes());
        dir::write_slot(&ofd.vol, ofd.dir_slot, &e);
    }
}

/// `Fdelete` (0x41) — delete a file by path.
pub fn fdelete(args: &TrapArgs) -> Long {
    // SAFETY: caller-supplied path pointer (kernel pointer at EL1).
    let path = match unsafe { super::cstr(args.arg_u64(0)) } {
        Some(p) => p,
        None => return EFILNF,
    };
    let (drive, name11) = match parse_path(path) {
        Some(x) => x,
        None => return EPTHNF,
    };
    let vol = match Volume::mount(drive) {
        Some(v) => v,
        None => return EDRIVE,
    };
    let entry = match dir::find_in_root(&vol, &name11) {
        Some(e) => e,
        None => return EFILNF,
    };
    // Read-only files and sub-directories cannot be deleted.
    if entry.attrib & (FA_RO | 0x10) != 0 {
        return EACCDN;
    }
    let slot = match dir::slot_of(&vol, &name11) {
        Some(s) => s,
        None => return EFILNF,
    };
    erase_entry(&vol, slot, entry.cluster);
    super::cache::flush();
    E_OK
}

/// `Fseek` (0x42) — reposition a handle's file pointer.
pub fn fseek(args: &TrapArgs) -> Long {
    let n = args.arg_i32(0) as i64;
    let handle = args.arg_u16(1);
    let flg = args.arg_u16(2);

    let ofd = match ofd_of(handle) {
        Some(o) => o,
        None => return EIHNDL,
    };
    let base = match flg {
        0 => 0,                  // SEEK_SET
        1 => ofd.pos as i64,     // SEEK_CUR
        2 => ofd.file_len as i64, // SEEK_END
        _ => return EINVFN,
    };
    let target = base + n;
    if target < 0 || target > ofd.file_len as i64 {
        return ERANGE;
    }
    ofd.pos = target as u32;
    ofd.pos as Long
}

/// `Fclose` (0x3E) — close a handle.
pub fn fclose(args: &TrapArgs) -> Long {
    let h = args.arg_i16(0);
    if h < 0 {
        return E_OK; // BIOS character device: no-op
    }
    let handle = h as u16;
    if handle < NUMSTD {
        return E_OK; // standard handle: nothing to free yet
    }
    let slot = (handle - NUMSTD) as usize;
    match table().get_mut(slot) {
        Some(s @ Some(_)) => {
            // Flush the directory entry (size / start cluster) if writes
            // changed it, then write back dirty cached sectors.
            if let Some(ofd) = s {
                if ofd.dirty {
                    flush_ofd(ofd);
                }
            }
            super::cache::flush();
            *s = None;
            E_OK
        }
        _ => EIHNDL,
    }
}
