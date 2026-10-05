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
use super::path;
use crate::trap::TrapArgs;
use tos_types::error::{
    EACCDN, EFILNF, EIHNDL, EINTRN, EINVFN, ENHNDL, ENSAME, EPTHNF, ERANGE, E_OK,
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
    /// First cluster of the directory holding this file's FCB
    /// (0 = root), and the raw slot within it (for size/cluster
    /// write-back on close).
    dir_start: u32,
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

/// One open-file-table entry: a real open file, or a `Fdup`'d redirect that
/// forwards to a captured target handle/device (for saving a standard
/// handle's mapping across `Fforce`).
#[derive(Clone, Copy)]
enum Slot {
    File(Ofd),
    Redirect(i16),
}

struct FileTable(UnsafeCell<[Option<Slot>; OPEN_FILES]>);
// SAFETY: accessed only from the single-CPU, IRQ-off trap path.
unsafe impl Sync for FileTable {}
static FILES: FileTable = FileTable(UnsafeCell::new([None; OPEN_FILES]));

fn table() -> &'static mut [Option<Slot>; OPEN_FILES] {
    // SAFETY: single trap path; never concurrently borrowed.
    unsafe { &mut *FILES.0.get() }
}

/// The calling process's UFT entry for standard handle `std` (0..NUMSTD):
/// `-1`/negative = a BIOS device (NewTOS maps all to the console), `>=
/// NUMSTD` = a redirected non-standard handle. No process → the console.
fn uft(std: u16) -> i16 {
    let bp = crate::bdos::process::current_kva();
    if bp.is_null() {
        return -1;
    }
    // SAFETY: `current()` returns a valid basepage pointer or null. The UFT
    // entries are bytes (handles 0..NUMHANDLES, devices -1..-4 all fit).
    unsafe { (*bp).p_uft.get(std as usize).copied().unwrap_or(-1) as i16 }
}

/// Set the calling process's UFT entry (for `Fforce`).
fn set_uft(std: u16, val: i16) {
    let bp = crate::bdos::process::current_kva();
    if bp.is_null() {
        return;
    }
    // SAFETY: valid basepage pointer.
    unsafe {
        if let Some(e) = (*bp).p_uft.get_mut(std as usize) {
            *e = val as i8;
        }
    }
}

/// The final I/O target a handle resolves to.
enum Target {
    /// The console (any default / device-mapped standard handle).
    Console,
    /// The open-file-table slot holding the file's `Ofd`.
    File(usize),
    /// An invalid handle.
    None,
}

/// Resolve a handle to its I/O target, following the per-process UFT (for
/// standard handles) and `Fdup` redirect chains (bounded against cycles).
fn resolve(handle: u16) -> Target {
    let mut t: i16 = if handle < NUMSTD {
        uft(handle)
    } else {
        handle as i16
    };
    for _ in 0..8 {
        if t < 0 {
            return Target::Console; // a BIOS device → the console
        }
        if (t as u16) < NUMSTD {
            t = uft(t as u16); // a standard-handle target → follow the UFT
            continue;
        }
        let slot = (t as u16 - NUMSTD) as usize;
        match table().get(slot).and_then(|s| *s) {
            Some(Slot::File(_)) => return Target::File(slot),
            Some(Slot::Redirect(inner)) => t = inner,
            None => return Target::None,
        }
    }
    Target::None
}

/// Resolve a handle to its open `Ofd` (a file), or `None` for the console /
/// an invalid handle. Used by the file-only ops (`Fseek`, `Fdatime`, …).
fn ofd_of(handle: u16) -> Option<&'static mut Ofd> {
    let slot = match resolve(handle) {
        Target::File(s) => s,
        _ => return None,
    };
    match table().get_mut(slot)?.as_mut()? {
        Slot::File(ofd) => Some(ofd),
        Slot::Redirect(_) => None,
    }
}

/// `Fopen` (0x3D) — open an existing file for reading/writing.
pub fn fopen(args: &TrapArgs) -> Long {
    // SAFETY: caller-supplied path pointer (kernel pointer at EL1).
    let path = match unsafe { super::cstr(args.arg_u64(0)) } {
        Some(p) => p,
        None => return EFILNF,
    };
    let mode = (args.arg_u16(1) & 0x77) as u8;

    let (vol, parent, name11) = match path::findit(path) {
        Ok(x) => x,
        // `Fopen` collapses *any* path-resolution failure to `EFILNF` — an
        // unmountable drive **and** a missing intermediate directory alike.
        // This is the TOS `Fopen` quirk: `ixopen`'s `findit` NULL branch
        // returns `EFILNF`, never `EPTHNF`, whatever the underlying cause
        // (`fsopnclo.c:247-249`; spec `3d-fopen.md:145-148`). Every other
        // caller keeps its own `EPTHNF` for a bad directory portion.
        Err(_) => return EFILNF,
    };
    let drive = super::path::split_drive(path).0;
    // `FA_NORM` scan semantics: a sub-directory (or label) is invisible here,
    // so `Fopen` on a folder returns `EFILNF` and never hands out a handle
    // whose writes would land in the directory cluster (EmuTOS
    // `scan(dn, s, FA_NORM, …)`, `fsopnclo.c:256`).
    let entry = match dir::find_file_in_dir(&vol, parent, &name11) {
        Some(e) => e,
        None => return EFILNF,
    };
    // Read-only files reject any write access mode.
    if entry.attrib & FA_RO != 0 && (mode & 0x07) != 0 {
        return EACCDN;
    }
    let dir_slot = match dir::slot_of(&vol, parent, &name11) {
        Some(s) => s,
        None => return EFILNF,
    };

    let slot = match table().iter().position(Option::is_none) {
        Some(s) => s,
        None => return ENHNDL,
    };
    table()[slot] = Some(Slot::File(Ofd {
        vol,
        drive,
        dir_start: parent,
        dir_slot,
        start_cluster: entry.cluster,
        file_len: entry.size,
        pos: 0,
        mode,
        dirty: false,
    }));
    (slot as u16 + NUMSTD) as Long
}

/// `Fcreate` (0x3C) — create (or truncate) a file and open it for
/// writing.
pub fn fcreate(args: &TrapArgs) -> Long {
    // SAFETY: caller-supplied path pointer (kernel pointer at EL1).
    let path = match unsafe { super::cstr(args.arg_u64(0)) } {
        Some(p) => p,
        None => return EPTHNF,
    };
    // Mask off the sub-directory bit — Fcreate cannot make folders.
    let attr = (args.arg_u16(1) as u8) & !0x10;

    // Resolve the directory portion but keep the *raw* final component, so
    // the illegal-character check runs against the user's path name (before
    // it is folded to an 8.3 FCB name), exactly like EmuTOS `ixcreat`.
    let (vol, parent, comp) = match path::resolve_parent(path) {
        Ok(x) => x,
        // The directory portion of a create resolves to `EPTHNF`, including an
        // unmountable drive (EmuTOS `ixcreat` `findit` NULL → `EPTHNF`,
        // `fsopnclo.c:114`).
        Err(e) => return path::drive_notfound(e, EPTHNF),
    };
    // A path with no final component (`"C:\\"`, `"C:\\SUB\\"`) or one that
    // starts with a dot is not a file `Fcreate` can make: EmuTOS `ixcreat`
    // rejects both right after `findit`, before the illegal-character check
    // (`bdos/fsopnclo.c:117-118`: `if (!*s || (*s == '.')) return EPTHNF;`).
    // `findit` itself resolves such a path successfully, so the guard belongs
    // here and not in `resolve_parent`.
    if comp.is_empty() || comp.starts_with('.') {
        return EPTHNF;
    }
    // F7: reject illegal characters in the new name (EmuTOS `ixcreat`
    // `contains_illegal_characters(s)` → `EACCDN`, `fsopnclo.c:121-122`).
    if path::contains_illegal_characters(comp) {
        return EACCDN;
    }
    let name11 = match dir::fcb_name(comp) {
        Some(n) => n,
        None => return EPTHNF,
    };
    let drive = path::split_drive(path).0;
    if !vol.is_writable() {
        return EACCDN;
    }

    // If the file already exists, delete it first (unless RO / folder).
    if let Some(existing) = dir::find_in_dir(&vol, parent, &name11) {
        if existing.attrib & (FA_RO | 0x10) != 0 {
            return EACCDN;
        }
        if let Some(s) = dir::slot_of(&vol, parent, &name11) {
            erase_entry(&vol, parent, s, existing.cluster);
        }
    }

    // F5: a full non-root directory grows by a cluster; only the fixed-size
    // root still reports `EACCDN` on exhaustion.
    let dir_slot = match dir::find_free_slot_grow(&vol, parent) {
        Some(s) => s,
        None => return EACCDN, // directory full (root is fixed-size)
    };
    let (time, date) = crate::gemdos::time::dos_now();
    let fcb = dir::build_fcb(&name11, attr, 0, 0, time, date);
    if !dir::write_slot(&vol, parent, dir_slot, &fcb) {
        return EACCDN;
    }
    // A device error while the new directory entry goes to the card is the
    // call's result, not a silent success: EmuTOS' `longjmp_rwabs` unwinds
    // `xcreat` with the BIOS code (`bdos/fs.h:60`, `_dispatcher.md` §8).
    if let Err(code) = super::cache::flush() {
        return code;
    }

    let slot = match table().iter().position(Option::is_none) {
        Some(s) => s,
        None => return ENHNDL,
    };
    table()[slot] = Some(Slot::File(Ofd {
        vol,
        drive,
        dir_start: parent,
        dir_slot,
        start_cluster: 0,
        file_len: 0,
        pos: 0,
        mode: 0x01, // write access
        dirty: false,
    }));
    (slot as u16 + NUMSTD) as Long
}

/// `Fread` (0x3F) — read `len` bytes from a handle into a buffer.
pub fn fread(args: &TrapArgs) -> Long {
    let handle = args.arg_u16(0);
    let len = args.arg_i32(1);
    let ubuf = args.arg_u64(2);

    // Route standard handles / redirects: a console-mapped handle reads
    // from the keyboard, a file handle reads the file (below).
    match resolve(handle) {
        Target::Console => return console_read(len, ubuf),
        Target::None => return EIHNDL,
        Target::File(_) => {}
    }
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
            // A chain that ends early is a short read (0 bytes); a chain
            // walk that failed because the *device* failed is a hard error
            // and carries the BIOS code (`EREADF`), the way EmuTOS' FAT
            // walk unwinds through `longjmp_rwabs` (`bdos/fs.h:60`).
            _ => return super::cache::take_error().unwrap_or(0),
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
    // A read that stopped on a device error reports the error, not a short
    // count (`docs/api/gemdos/file/3f-fread.md` §"Error conditions": an
    // underlying disk read failure surfaces as the BIOS code, `EREADF`).
    if let Some(code) = super::cache::take_error() {
        return code;
    }
    done as Long
}

/// `Fwrite` (0x40) — write `len` bytes from a buffer to a handle,
/// extending the file (allocating clusters) as needed.
pub fn fwrite(args: &TrapArgs) -> Long {
    let handle = args.arg_u16(0);
    let len = args.arg_i32(1);
    let ubuf = args.arg_u64(2);

    // Route standard handles / redirects: a console-mapped handle writes
    // to the console, a file handle writes the file (below).
    match resolve(handle) {
        Target::Console => return console_write(len, ubuf),
        Target::None => return EIHNDL,
        Target::File(_) => {}
    }
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
    // Write-back of everything this call dirtied. Its code wins over the
    // byte count: EmuTOS never returns a short count for a hard error, it
    // unwinds with `EWRITF` (`bdos/fs.h:60`).
    if let Err(code) = super::cache::flush() {
        return code;
    }
    // ...and a loop that stopped early because the *device* failed is the
    // same hard error, not the "disk full" short count
    // (`docs/api/gemdos/file/40-fwrite.md` §"Error conditions": disk-full is
    // a short count, a media fault is a BIOS code).
    if let Some(code) = super::cache::take_error() {
        return code;
    }
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

/// Free a file's cluster chain and mark its directory entry — and the
/// long-name slots in front of it — erased. `start` is the directory
/// holding the entry (0 = root).
fn erase_entry(vol: &Volume, start: u32, slot: u32, start_cluster: u32) {
    // F6: close any open handle to this same file *before* freeing its
    // clusters, so a stale handle (`Fopen`→`Fdelete`→`Fwrite`/`Fclose`)
    // cannot write into the freed / re-allocated chain. EmuTOS `ixdel`
    // scans the OFDs open on this directory position and, for the running
    // process, closes them before the chain is freed (`fsopnclo.c:660-669`).
    close_open_file(start, slot);
    if vol.is_data_cluster(start_cluster) {
        vol.free_chain(start_cluster);
    }
    // FS01: the 8.3 entry *and* the `attr == 0x0F` long-name slots that
    // belong to it. Freeing only the short entry leaves the chain behind as
    // an orphan (`dir::erase_slot_with_lfn`).
    dir::erase_slot_with_lfn(vol, start, slot);
}

/// Close every open-file-table entry that refers to the directory entry at
/// (`dir_start`, `dir_slot`) — i.e. an open handle to a file being deleted
/// or overwritten. Invalidating the slot here means a subsequent
/// `Fread`/`Fwrite`/`Fclose` on that handle returns `EIHNDL` rather than
/// operating on freed clusters.
///
/// The identifying key `(dir_start, dir_slot)` is EmuTOS' `(o_dnode,
/// o_dirbyt)` pair and uniquely names the file. EmuTOS distinguishes the
/// *owning* process (close it via `ixclose`) from a foreign one (refuse the
/// delete with `EACCDN`); NewTOS' open-file table is process-agnostic (one
/// shared SFT, the desktop's single active process), so it always takes the
/// owning-process branch and closes the handle.
fn close_open_file(dir_start: u32, dir_slot: u32) {
    for s in table().iter_mut() {
        if let Some(Slot::File(ofd)) = s {
            if ofd.dir_start == dir_start && ofd.dir_slot == dir_slot {
                *s = None;
            }
        }
    }
}

/// Flush an open file's directory entry (size + start cluster).
fn flush_ofd(ofd: &Ofd) {
    let mut e = [0u8; 32];
    if dir::read_slot(&ofd.vol, ofd.dir_start, ofd.dir_slot, &mut e) {
        e[0x1A..0x1C].copy_from_slice(&((ofd.start_cluster & 0xFFFF) as u16).to_le_bytes());
        e[0x1C..0x20].copy_from_slice(&ofd.file_len.to_le_bytes());
        dir::write_slot(&ofd.vol, ofd.dir_start, ofd.dir_slot, &e);
    }
}

/// `Fdelete` (0x41) — delete a file by path.
pub fn fdelete(args: &TrapArgs) -> Long {
    // SAFETY: caller-supplied path pointer (kernel pointer at EL1).
    let path = match unsafe { super::cstr(args.arg_u64(0)) } {
        Some(p) => p,
        None => return EFILNF,
    };
    let (vol, parent, name11) = match path::findit(path) {
        Ok(x) => x,
        // `Fdelete` collapses a resolution failure to `EFILNF`, including an
        // unmountable drive (EmuTOS `ixunlink` `findit` NULL → `EFILNF`,
        // `fsopnclo.c:598`).
        Err(e) => return path::drive_notfound(e, EFILNF),
    };
    let entry = match dir::find_in_dir(&vol, parent, &name11) {
        Some(e) => e,
        None => return EFILNF,
    };
    // Read-only files and sub-directories cannot be deleted.
    if entry.attrib & (FA_RO | 0x10) != 0 {
        return EACCDN;
    }
    let slot = match dir::slot_of(&vol, parent, &name11) {
        Some(s) => s,
        None => return EFILNF,
    };
    erase_entry(&vol, parent, slot, entry.cluster);
    if let Err(code) = super::cache::flush() {
        return code;
    }
    E_OK
}

/// Drop the separators at the end of a path, for the [`fattrib`] GET probe
/// below. `None` when there is nothing to drop, **or** when dropping them
/// would leave no final component at all (`"C:\"`, `"\"`, `""`): the drive
/// root has no directory entry whose attribute byte could be read, and
/// synthesizing one — FreeMiNT does, EmuTOS and TOS 3.06 answer `EFILNF` —
/// is a separate decision this function deliberately does not take.
fn without_trailing_seps(path: &str) -> Option<&str> {
    let stripped = path.trim_end_matches(['/', '\\']);
    if stripped.len() == path.len() {
        return None; // no trailing separator
    }
    let (_, rest) = path::split_drive(stripped);
    let last = match rest.rfind(['/', '\\']) {
        Some(i) => &rest[i + 1..],
        None => rest,
    };
    (!last.is_empty()).then_some(stripped)
}

/// The attribute byte of the entry `path` names — but only if that entry is a
/// **sub-directory**. `None` for a file, a missing name, or a path that does
/// not resolve at all, so the caller can fall through to its unchanged code.
fn subdir_attrib(path: &str) -> Option<u8> {
    let (vol, parent, name11) = path::findit(path).ok()?;
    let entry = dir::find_in_dir(&vol, parent, &name11)?;
    (entry.attrib & dir::FA_SUBDIR != 0).then_some(entry.attrib)
}

/// `Fattrib` (0x43) — get or set a file's (or, on GET, a directory's)
/// attribute byte. `wrt = 0` returns the current attributes — for files
/// *and*, per the TOS4/FreeMiNT extension, folders (with FA_SUBDIR set),
/// written with or without a trailing separator;
/// `wrt = 1` sets `mod` on a **file** only (FA_VOL / FA_SUBDIR bits and any
/// set on a folder/label are rejected with EACCDN) and returns it.
pub fn fattrib(args: &TrapArgs) -> Long {
    // SAFETY: caller-supplied path pointer (kernel pointer at EL1).
    let pathstr = match unsafe { super::cstr(args.arg_u64(0)) } {
        Some(p) => p,
        None => return EPTHNF,
    };
    let wrt = args.arg_u16(1);
    let mode = args.arg_u16(2) as u8;

    let (vol, parent, name11) = match path::findit(pathstr) {
        Ok(x) => x,
        // The directory portion of the path resolves to `EPTHNF`, including an
        // unmountable drive (EmuTOS `xchmod` `findit` NULL → `EPTHNF`,
        // `fsdir.c:410`).
        Err(e) => return path::drive_notfound(e, EPTHNF),
    };
    if wrt == 0 {
        // GET — TOS4/FreeMiNT extension: resolve files *and* directories, so
        // `Fattrib(dir, 0, 0)` returns the directory entry's attribute byte
        // (with FA_SUBDIR = 0x10 set) instead of EFILNF. cflib `path_exists`
        // (used by QED's clipboard init and every cflib app) probes a folder
        // exactly this way. FreeMiNT `sys_f_attrib` returns the raw attribute
        // for both files and folders (`return xattr.attr`, `sys/dosdir.c:1001`);
        // the TOS4 lift of the folder restriction is documented in EmuTOS
        // `bdos/fsdir.c:391-400`. The subdir-aware `find_in_dir` sees folders
        // (volume-label/LFN records are already skipped by `DirIter`).
        //
        // cflib appends a separator to *every* path before it probes
        // (`make_normalpath`, `sdk/cflib/cflib-21/fsnorm.c:85-92`), so the one
        // caller this extension exists for asks with `"C:\CLIPBRD\"` — and
        // `findit` hands that spelling on as the **empty** final component,
        // which `fcb_name` turns into eleven blanks no entry can carry. That is
        // EmuTOS' own behaviour (`scan(builds(""))`, `bdos/fsdir.c:737-795`),
        // but it leaves the sanctioned extension half-done. FreeMiNT, the
        // reference for it, answers the trailing-separator spelling with the
        // folder's attributes: `relpath2cookie` peels an empty last component
        // (`sys/filesys.c:986-1010`) and `fatfs_lookup` resolves the empty name
        // **as `"."`** (`sys/fatfs.c:5753-5764`).
        //
        // So probe the separator-stripped spelling first and accept it **only
        // when it names a directory**. That keeps the extension purely
        // additive — a trailing separator on a *file* (EmuTOS `EPTHNF`;
        // FreeMiNT's ENOTDIR is the same -34, `sys/mint/errno.h:277`), on a
        // missing name, and the bare drive root all fall through to the
        // unchanged code below and keep their current answer.
        if let Some(attr) = without_trailing_seps(pathstr).and_then(subdir_attrib) {
            return attr as i8 as Long; // sign-extended UBYTE
        }
        let entry = match dir::find_in_dir(&vol, parent, &name11) {
            Some(e) => e,
            None => return EFILNF,
        };
        return entry.attrib as i8 as Long; // sign-extended UBYTE
    }
    // SET (wrt == 1) — files only. A directory's (or volume label's) attribute
    // byte stays immutable: never rewrite FA_SUBDIR/FA_VOL, which is the #38
    // corruption guard (Fattrib must not mutate a folder entry). FreeMiNT
    // forbids toggling FA_DIR on a set — `(attr & FA_DIR) != (xattr.attr &
    // FA_DIR)` → EACCES (`sys/dosdir.c:976-981`) — and rejects a volume label
    // (`sys/dosdir.c:987-990`); NewTOS is stricter and rejects *any* set on a
    // folder or label. `find_in_dir` (not `find_file_in_dir`) is used so we can
    // observe the folder and return EACCDN rather than the file-not-found path.
    let entry = match dir::find_in_dir(&vol, parent, &name11) {
        Some(e) => e,
        None => return EFILNF,
    };
    if entry.attrib & (dir::FA_VOL | dir::FA_SUBDIR) != 0 {
        return EACCDN;
    }
    // Set: FA_VOL (0x08) and FA_SUBDIR (0x10) are not settable here.
    if mode & 0x18 != 0 {
        return EACCDN;
    }
    let slot = match dir::slot_of(&vol, parent, &name11) {
        Some(s) => s,
        None => return EFILNF,
    };
    let mut e = [0u8; 32];
    if !dir::read_slot(&vol, parent, slot, &mut e) {
        return EACCDN;
    }
    e[0x0B] = mode;
    if !dir::write_slot(&vol, parent, slot, &e) {
        return EACCDN;
    }
    if let Err(code) = super::cache::flush() {
        return code;
    }
    mode as Long
}

/// `Frename` (0x56) — rename/move a file. `x0` = `n` (unused historical
/// word), `x1` = source path, `x2` = destination path.
pub fn frename(args: &TrapArgs) -> Long {
    // SAFETY: caller-supplied path pointers (kernel pointers at EL1).
    let src = match unsafe { super::cstr(args.arg_u64(1)) } {
        Some(p) => p,
        None => return EPTHNF,
    };
    let dst = match unsafe { super::cstr(args.arg_u64(2)) } {
        Some(p) => p,
        None => return EPTHNF,
    };

    // Both the source and target directory portions resolve to `EPTHNF` on
    // failure, an unmountable drive included (EmuTOS `xrename`: `findit(p1)`
    // and `findit(p2)` NULL → `EPTHNF`, `fsdir.c:916,975`).
    let (vol1, parent1, name1) = match path::findit(src) {
        Ok(x) => x,
        Err(e) => return path::drive_notfound(e, EPTHNF),
    };
    // Resolve the destination directory but keep the *raw* final component
    // so the illegal-character check sees the user's new name.
    let (_vol2, parent2, dcomp) = match path::resolve_parent(dst) {
        Ok(x) => x,
        Err(e) => return path::drive_notfound(e, EPTHNF),
    };
    // F7: the *new* name may not contain illegal characters (EmuTOS
    // `xrename` `contains_illegal_characters(s2)` → `EACCDN`, `fsdir.c:985`).
    // Checked before the cross-device / target-exists checks, matching the
    // source order.
    if path::contains_illegal_characters(dcomp) {
        return EACCDN;
    }
    let name2 = match dir::fcb_name(dcomp) {
        Some(n) => n,
        None => return EPTHNF,
    };
    // Cross-device renames are not supported.
    if path::split_drive(src).0 != path::split_drive(dst).0 {
        return ENSAME;
    }
    // Destination must not already exist.
    if dir::find_in_dir(&vol1, parent2, &name2).is_some() {
        return EACCDN;
    }
    // Source must exist and not be read-only.
    let entry = match dir::find_in_dir(&vol1, parent1, &name1) {
        Some(e) => e,
        None => return EFILNF,
    };
    if entry.attrib & FA_RO != 0 {
        return EACCDN;
    }
    // Cycle guard (before any mutation): a cross-directory move of a *folder*
    // into itself or one of its own descendants would splice the directory
    // tree into a loop — `fix_dotdot` below would then chain the folder's
    // `..` back into its subtree, orphaning the clusters between (the
    // fsck-reproduced corruption). EmuTOS' `is_subdir` only catches the
    // single immediate level (`fsdir.c:1002`, gap D-070); NewTOS walks the
    // full `..` ancestor chain of the target parent, so one-, two- and
    // deeper-level self-nesting are all refused with `EACCDN`. A legitimate
    // cross-directory move (target parent not under the source) is unaffected.
    if entry.attrib & dir::FA_SUBDIR != 0
        && parent1 != parent2
        && is_self_or_descendant(&vol1, entry.cluster, parent2)
    {
        return EACCDN;
    }
    let src_slot = match dir::slot_of(&vol1, parent1, &name1) {
        Some(s) => s,
        None => return EFILNF,
    };
    // Copy the FCB into a free slot of the destination directory with the
    // new name, then erase the source entry.
    let mut fcb = [0u8; 32];
    if !dir::read_slot(&vol1, parent1, src_slot, &mut fcb) {
        return EACCDN;
    }
    fcb[0..11].copy_from_slice(&name2);
    // F5: grow a full non-root destination directory (the root stays fixed).
    let dst_slot = match dir::find_free_slot_grow(&vol1, parent2) {
        Some(s) => s,
        None => return EACCDN, // destination directory full
    };
    if !dir::write_slot(&vol1, parent2, dst_slot, &fcb) {
        return EACCDN;
    }
    let mut old = [0u8; 32];
    if dir::read_slot(&vol1, parent1, src_slot, &mut old) {
        old[0] = 0xE5;
        dir::write_slot(&vol1, parent1, src_slot, &old);
    }
    // Moving a *folder* into a *different* directory leaves its `..` entry
    // pointing at the old parent. Rewrite it to the new parent's start
    // cluster (0 when the new parent is the root, per FAT convention).
    // File renames and same-directory folder renames need no such fix-up.
    if entry.attrib & dir::FA_SUBDIR != 0 && parent1 != parent2 && !fix_dotdot(&vol1, entry.cluster, parent2) {
        return EINTRN;
    }
    if let Err(code) = super::cache::flush() {
        return code;
    }
    E_OK
}

/// FAT short name of the `..` directory entry (`".."` + nine spaces).
const DOTDOT: [u8; 11] = *b"..         ";

/// Whether `target_parent` **is** the folder `folder_cluster`, or lives
/// somewhere beneath it — i.e. moving that folder so its new parent is
/// `target_parent` would nest the folder inside its own subtree (a directory
/// cycle). Walks the `..` chain of `target_parent` toward the root (cluster 0
/// = root, chain terminates); a match against `folder_cluster` anywhere along
/// the way means "self or descendant". The step count is bounded so an
/// already-corrupt (pre-existing cyclic) `..` chain cannot hang the walk —
/// hitting the bound is itself treated as a cycle (refuse the move).
fn is_self_or_descendant(vol: &Volume, folder_cluster: u32, target_parent: u32) -> bool {
    // FAT16 tops out around 65525 clusters, and legitimate directory nesting
    // is far shallower still (bounded by the path-length limit); this cap is a
    // safety net against a corrupt `..` chain, not a real depth limit.
    const MAX_DEPTH: u32 = 4096;
    let mut cur = target_parent;
    for _ in 0..MAX_DEPTH {
        if cur == 0 {
            return false; // reached the root without meeting the folder
        }
        if cur == folder_cluster {
            return true; // target parent is the folder or below it
        }
        cur = match dir::find_in_dir(vol, cur, &DOTDOT) {
            Some(e) => e.cluster,
            None => return false, // no `..` entry to follow (malformed dir)
        };
    }
    true // bound exhausted → assume a cycle and refuse
}

/// Rewrite the `..` entry of a moved folder (whose first cluster is
/// `folder_cluster`) to point at `new_parent`'s start cluster (0 = root).
/// Returns `false` if the `..` entry cannot be located or written back.
fn fix_dotdot(vol: &Volume, folder_cluster: u32, new_parent: u32) -> bool {
    let slot = match dir::slot_of(vol, folder_cluster, &DOTDOT) {
        Some(s) => s,
        None => return false,
    };
    let mut e = [0u8; 32];
    if !dir::read_slot(vol, folder_cluster, slot, &mut e) {
        return false;
    }
    // FAT12/16 has no high cluster word; write the low 16 bits at 0x1A.
    e[0x1A..0x1C].copy_from_slice(&((new_parent & 0xFFFF) as u16).to_le_bytes());
    dir::write_slot(vol, folder_cluster, slot, &e)
}

/// `Fdatime` (0x57) — get or set an open file's time/date stamp.
/// `x0` = `DOSTIME*` (time WORD then date WORD), `x1` = handle,
/// `x2` = `wrt` (0 = get, 1 = set). Values are native byte order.
pub fn fdatime(args: &TrapArgs) -> Long {
    let buf = args.arg_u64(0);
    let handle = args.arg_u16(1);
    let wrt = args.arg_u16(2);

    let ofd = match ofd_of(handle) {
        Some(o) => o,
        None => return EIHNDL,
    };
    let mut e = [0u8; 32];
    if !dir::read_slot(&ofd.vol, ofd.dir_start, ofd.dir_slot, &mut e) {
        return EIHNDL;
    }
    let p = buf as *mut u8;
    if wrt == 0 {
        // Get: copy the entry's time (0x16) + date (0x18) to the buffer.
        // SAFETY: `buf` is the caller's 4-byte DOSTIME buffer.
        unsafe {
            core::ptr::copy_nonoverlapping(e.as_ptr().add(0x16), p, 4);
        }
    } else {
        // Set: copy time/date from the buffer into the entry.
        // SAFETY: as above.
        unsafe {
            core::ptr::copy_nonoverlapping(p as *const u8, e.as_mut_ptr().add(0x16), 4);
        }
        dir::write_slot(&ofd.vol, ofd.dir_start, ofd.dir_slot, &e);
        if let Err(code) = super::cache::flush() {
            return code;
        }
    }
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
            // changed it, then write back dirty cached sectors. A `Fdup`
            // redirect entry holds no file, so it is just released.
            let mut err = E_OK;
            if let Some(Slot::File(ofd)) = s {
                if ofd.dirty {
                    flush_ofd(ofd);
                }
                // The close is where the file's length and time reach the
                // card. A failure here means the file on the card is not
                // the file the application wrote, so it is the result of
                // `Fclose` — the handle is released either way, exactly as
                // EmuTOS' `xclose` runs its buffer flush before returning.
                if let Err(code) = super::cache::flush() {
                    err = code;
                }
            }
            *s = None;
            err
        }
        _ => EIHNDL,
    }
}

/// Read `len` bytes from the console (keyboard) into `ubuf` — the target of
/// a console-mapped `Fread`. Blocks until `len` bytes arrive.
fn console_read(len: i32, ubuf: u64) -> Long {
    if len <= 0 {
        return 0;
    }
    let Some(h) = crate::host::host() else {
        return 0;
    };
    let dst = ubuf as *mut u8;
    for i in 0..len as usize {
        let b = h.console_read_byte_blocking();
        // SAFETY: the caller's buffer holds >= len bytes.
        unsafe { *dst.add(i) = b };
    }
    len as Long
}

/// Write `len` bytes from `ubuf` to the console — the target of a
/// console-mapped `Fwrite`.
fn console_write(len: i32, ubuf: u64) -> Long {
    if len <= 0 {
        return 0;
    }
    let Some(h) = crate::host::host() else {
        return 0;
    };
    let src = ubuf as *const u8;
    for i in 0..len as usize {
        // SAFETY: the caller's buffer holds >= len bytes.
        let b = unsafe { *src.add(i) };
        h.console_write_byte(b);
    }
    len as Long
}

/// `Fdup` (0x45) — duplicate the current mapping of standard handle `h`
/// (0..NUMSTD) into a fresh non-standard handle, so a program can save it
/// before redirecting with `Fforce` (EmuTOS `xdup`). Returns the new
/// handle, `EIHNDL` for a non-standard `h`, or `ENHNDL` when the table is
/// full.
pub fn fdup(args: &TrapArgs) -> Long {
    let h = args.arg_i16(0);
    if h < 0 || h as u16 >= NUMSTD {
        return EIHNDL; // only standard handles may be duplicated
    }
    let captured = uft(h as u16);
    let slot = match table().iter().position(Option::is_none) {
        Some(s) => s,
        None => return ENHNDL,
    };
    table()[slot] = Some(Slot::Redirect(captured));
    (slot as u16 + NUMSTD) as Long
}

/// `Fforce` (0x46) — redirect standard handle `std` (0..NUMSTD) to source
/// handle `h` (a non-standard handle `>= NUMSTD`, or a negative BIOS
/// device) by rewriting the process UFT (EmuTOS `xforce`). Returns `E_OK`,
/// or `EIHNDL` for a bad `std`, a standard-handle source, or a source that
/// references no live table entry.
pub fn fforce(args: &TrapArgs) -> Long {
    let std = args.arg_i16(0);
    let h = args.arg_i16(1);
    if std < 0 || std as u16 >= NUMSTD {
        return EIHNDL;
    }
    if h >= 0 && (h as u16) < NUMSTD {
        return EIHNDL; // cannot force to another standard handle
    }
    if h >= NUMSTD as i16 {
        let slot = (h as u16 - NUMSTD) as usize;
        if table().get(slot).and_then(|s| *s).is_none() {
            return EIHNDL;
        }
    }
    set_uft(std as u16, h);
    E_OK
}
