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
/// Default drive when a path carries no `X:` prefix — `C:` for now (the
/// current-drive state is `Dgetdrv`/`Dsetdrv`, a later deliverable).
const DEFAULT_DRIVE: u16 = 2;
/// `FA_RO` read-only attribute bit.
const FA_RO: u8 = 0x01;

/// Open File Descriptor — reduced EmuTOS `OFD`.
#[derive(Clone, Copy)]
struct Ofd {
    vol: Volume,
    /// First cluster of the file (0 ⇒ empty file).
    start_cluster: u32,
    /// File length in bytes.
    file_len: u32,
    /// Current byte offset (`o_bytnum`).
    pos: u32,
    /// Open mode bits (`mod & 0x77`). Recorded per the TOS contract;
    /// sharing bits are not enforced (read by the write side later).
    #[allow(dead_code)]
    mode: u8,
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
    let mut drive = DEFAULT_DRIVE;
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

    let slot = match table().iter().position(Option::is_none) {
        Some(s) => s,
        None => return ENHNDL,
    };
    table()[slot] = Some(Ofd {
        vol,
        start_cluster: entry.cluster,
        file_len: entry.size,
        pos: 0,
        mode,
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
            *s = None;
            E_OK
        }
        _ => EIHNDL,
    }
}
