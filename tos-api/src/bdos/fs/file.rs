//! GEMDOS read-side file API: `Fopen` / `Fread` / `Fseek` / `Fclose`.
//!
//! Spec: `docs/api/gemdos/file/{3d-fopen,3f-fread,42-fseek,3e-fclose}.md`.
//! Mirrors the EmuTOS `bdos/fsopnclo.c` + `bdos/fsio.c` model in reduced
//! form: a global open-file table (the SFT, `f_ofd`/`f_own`/`f_use`) whose
//! entries point at [`Ofd`]s — one OFD may be shared (`Fdup`). Handles
//! `>= NUMSTD` (6) index this table; standard handles `0..5` map through the
//! running PD's `p_uft[]` (`Fforce`, inherited by `Pexec`, closed by `Pterm`;
//! `docs/api/gemdos/_state.md` §2, TASK/PROC02).
//!
//! Read-only / root-directory only for this deliverable: paths resolve a
//! drive letter and a single 8.3 component in the volume root (sub-
//! directory `findit` walking lands with `Dsetpath`).

use core::cell::UnsafeCell;

use super::dir;
use super::fat::Volume;
use super::fsio::{self, Fail, Stream};
use super::path;
use crate::trap::TrapArgs;
use tos_types::error::{
    EACCDN, EFILNF, EIHNDL, EINTRN, EINVFN, ENHNDL, ENSAME, EPTHNF, E_OK,
};
use tos_types::Long;

/// Number of reserved standard handles (0..5); first real handle is 6.
const NUMSTD: u16 = 6;
/// Open-file table size — EmuTOS `OPNFILES` (`bdos/fs.h:43`), system-wide
/// and unconditional: handles `NUMSTD..NUMHANDLES` = 6..80. A full table
/// answers `ENHNDL` from every allocator ([`claim_sft`]). The largest handle
/// (80) still fits the `i8` `p_uft[]` entries (TASK/PROC04 point 4).
const OPEN_FILES: usize = 75;
/// `FA_RO` read-only attribute bit.
const FA_RO: u8 = 0x01;

/// Open File Descriptor — reduced EmuTOS `OFD`.
#[derive(Clone, Copy)]
struct Ofd {
    /// Volume, start cluster, length, `O_DIRTY` and the position fields
    /// `o_bytnum`/`o_curcl`/`o_currec`/`o_curbyt` (FS08, [`fsio`]).
    s: Stream,
    /// Drive the file lives on (recorded for future open-file tracking).
    #[allow(dead_code)]
    drive: u16,
    /// First cluster of the directory holding this file's FCB
    /// (0 = root), and the raw slot within it (for size/cluster
    /// write-back on close).
    dir_start: u32,
    dir_slot: u32,
    /// Open mode bits (`mod & 0x77`). Sharing bits are recorded, not
    /// enforced, per the TOS contract.
    #[allow(dead_code)]
    mode: u8,
    /// The OFD is on its directory's open-file list (`o_dnode->d_files`):
    /// put there when the file is opened (`makopn`, `bdos/fsopnclo.c:294-295`),
    /// taken off by the first full `ixclose` (`:552-566`). A later `ixclose`
    /// of the same OFD — a handle shared through `Fforce`, `Fdup` or
    /// inheritance — finds it gone and answers `EINTRN`.
    listed: bool,
}

/// What an `sft[]` entry points at — EmuTOS `f_ofd`: no OFD (`NULL`), a BIOS
/// character device (a negative handle stored in the pointer), or an OFD
/// (here an index into `Tables::ofds`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum FOfd {
    Null,
    Bios(i16),
    File(usize),
}

impl FOfd {
    /// `(OFD *)(long)h` for a standard handle's BIOS mapping `h <= 0`
    /// (`bdos/fshand.c:118`): `0` is the `NULL` pointer.
    fn bios(h: i16) -> FOfd {
        if h < 0 { FOfd::Bios(h) } else { FOfd::Null }
    }
}

/// One `sft[]` entry — EmuTOS `FTAB` (`bdos/fs.h:307-312`).
#[derive(Clone, Copy)]
struct Ftab {
    /// `f_ofd`.
    ofd: FOfd,
    /// `f_own`: the PD (user address) that opened / duplicated the handle;
    /// `None` is EmuTOS's `NULL` — the entry is free (`!sft[i].f_own`).
    own: Option<u64>,
    /// `f_use`: the use count (`Fforce` and inheritance add one,
    /// `bdos/fshand.c:73`).
    use_: i16,
}

const FREE: Ftab = Ftab { ofd: FOfd::Null, own: None, use_: 0 };

/// EmuTOS `NUMHANDLES` with this table's size: standard + non-standard.
const NUMHANDLES: usize = NUMSTD as usize + OPEN_FILES;

struct Tables {
    /// `sft[OPNFILES]` — handle `h >= NUMSTD` is `sft[h - NUMSTD]`.
    sft: [Ftab; OPEN_FILES],
    /// The OFDs the `sft[]` entries point at; one OFD may be shared by several
    /// entries (`Fdup`), and lives while any entry points at it (`sftdel`).
    ofds: [Option<Ofd>; OPEN_FILES],
}

struct FileTable(UnsafeCell<Tables>);
// SAFETY: accessed only from the single-CPU, IRQ-off trap path.
unsafe impl Sync for FileTable {}
static FILES: FileTable =
    FileTable(UnsafeCell::new(Tables { sft: [FREE; OPEN_FILES], ofds: [None; OPEN_FILES] }));

fn sft() -> &'static mut [Ftab; OPEN_FILES] {
    // SAFETY: single trap path; never concurrently borrowed.
    unsafe { &mut (*FILES.0.get()).sft }
}

fn ofds() -> &'static mut [Option<Ofd>; OPEN_FILES] {
    // SAFETY: single trap path; never concurrently borrowed.
    unsafe { &mut (*FILES.0.get()).ofds }
}

/// EmuTOS `run` as an owner: the running PD, or — kernel code with no
/// running process — the stand-in PD that is `run` there (TASK/PPARENT; `0`
/// before the stand-ins exist).
fn run_pd() -> u64 {
    let r = crate::bdos::process::current();
    if r.is_null() {
        crate::bdos::process::standin::kernel_run() as u64
    } else {
        r as u64
    }
}

/// Claim a free `sft[]` entry for `run` with `f_use = 1` and the given
/// `f_ofd` (`opnfil`, `bdos/fsopnclo.c:341-349`; `xdup`, `bdos/fshand.c:
/// 100-120`). Returns its index, or `None` when every entry is owned.
fn claim_sft(ofd: FOfd) -> Option<usize> {
    let i = sft().iter().position(|e| e.own.is_none())?;
    sft()[i] = Ftab { ofd, own: Some(run_pd()), use_: 1 };
    Some(i)
}

/// Open `ofd` on a new handle: a fresh OFD (listed on its directory) and a
/// claimed `sft[]` entry. `None` when the table is full (`ENHNDL`).
fn open_handle(ofd: Ofd) -> Option<Long> {
    // Every OFD is pointed at by at least one entry, so a free entry means a
    // free OFD slot.
    let j = ofds().iter().position(Option::is_none)?;
    let i = claim_sft(FOfd::File(j))?;
    ofds()[j] = Some(ofd);
    Some((i as u16 + NUMSTD) as Long)
}

/// The calling process's UFT entry for standard handle `std` (0..NUMSTD):
/// negative = a BIOS device, `>= NUMSTD` = a redirected non-standard handle.
/// No process → the console.
fn uft(std: u16) -> i16 {
    let bp = crate::bdos::process::current_kva();
    if bp.is_null() {
        return -1;
    }
    // SAFETY: `current()` returns a valid basepage pointer or null. The UFT
    // entries are bytes (handles 0..NUMHANDLES, devices -1..-4 all fit).
    unsafe { (*bp).p_uft.get(std as usize).copied().unwrap_or(-1) as i16 }
}

/// Set the calling process's UFT entry (`Fclose` of a standard handle).
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

/// `sft[h - NUMSTD].f_ofd` for a value `h` taken from a `p_uft[]` entry
/// `> 0` — `NULL` for a value that names no entry (`1..NUMSTD`, which
/// `Fforce` never stores, or one past the table).
fn sft_ofd(h: i16) -> FOfd {
    if h < NUMSTD as i16 {
        return FOfd::Null;
    }
    sft().get((h - NUMSTD as i16) as usize).map_or(FOfd::Null, |e| e.ofd)
}

/// EmuTOS `getofd` (`bdos/fsmain.c:474-486`) via `syshnd` (`:457-466`): a
/// non-standard handle names its entry; a standard handle its `p_uft[]`
/// value if that is `> 0`, else it has no OFD. Answers the entry's `f_ofd`.
fn getofd(h: i16) -> FOfd {
    if h < 0 || h as usize >= NUMHANDLES {
        return FOfd::Null;
    }
    let n = if h >= NUMSTD as i16 {
        h - NUMSTD as i16
    } else {
        let t = uft(h as u16);
        if t > 0 { t - NUMSTD as i16 } else { t }
    };
    if n < 0 {
        return FOfd::Null;
    }
    sft().get(n as usize).map_or(FOfd::Null, |e| e.ofd)
}

/// The final I/O target a handle resolves to.
enum Target {
    /// A BDOS character-device handle (`-1` CON, `-2` AUX, `-3` PRN); its BIOS
    /// device (`HXFORM`) is served by the device table
    /// ([`crate::bios::chardev`], FS11).
    Device(i16),
    /// The OFD (index into `Tables::ofds`) the handle's entry points at.
    File(usize),
    /// An invalid handle.
    None,
}

/// Lowest BDOS character-device handle (`H_Print`, `bdos/console.h:23-25`).
const H_PRINT: i16 = -3;

/// Resolve a handle to its I/O target as the dispatcher does for the
/// handle-based functions `Fread`/`Fwrite`/`Fseek` (`bdos/bdosmain.c:
/// 571-602`). `handle` is the signed handle WORD: a non-standard handle
/// takes its entry's `f_ofd`; a standard handle its `p_uft[]` value — `> 0`
/// the entry it was forced to, else the BIOS handle itself (`0`: none); a
/// negative handle is the device itself. A device below `-3` is invalid
/// (`:601-602`). A handle past the table is invalid (spec 42-fseek.md, the
/// `>= NUMHANDLES` contract row).
fn resolve(handle: i16) -> Target {
    let numl = if handle >= NUMSTD as i16 {
        sft().get((handle - NUMSTD as i16) as usize).map_or(FOfd::Null, |e| e.ofd)
    } else if handle >= 0 {
        let h = uft(handle as u16);
        if h > 0 { sft_ofd(h) } else { FOfd::bios(h) }
    } else {
        FOfd::Bios(handle)
    };
    match numl {
        FOfd::Null => Target::None,
        FOfd::Bios(n) if n < H_PRINT => Target::None,
        FOfd::Bios(n) => Target::Device(n),
        FOfd::File(j) => Target::File(j),
    }
}

/// Resolve a handle to its open `Ofd` (a file), or `None` for a device /
/// an invalid handle. Used by the file-only ops (`Fseek`, `Fdatime`, …).
fn ofd_of(handle: i16) -> Option<&'static mut Ofd> {
    match resolve(handle) {
        Target::File(j) => ofds().get_mut(j)?.as_mut(),
        _ => None,
    }
}

/// `Fopen` (0x3D) — open an existing file for reading/writing.
pub fn fopen(args: &TrapArgs) -> Long {
    // Path argument(s) in the caller's space (ARCH03 3a, `fs::path_arg`).
    let mut path_buf: super::PathBuf = [0; super::PATH_ARG_MAX];
    let path = match super::path_arg(args, 0, &mut path_buf) {
        Ok(Some(p)) => p,
        Ok(None) => return EFILNF,
        Err(_) => return crate::uaccess::FAULTED,
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

    open_handle(Ofd {
        s: Stream::new(vol, entry.cluster, entry.size),
        drive,
        dir_start: parent,
        dir_slot,
        mode,
        listed: true,
    })
    .unwrap_or(ENHNDL)
}

/// `Fcreate` (0x3C) — create (or truncate) a file and open it for
/// writing.
pub fn fcreate(args: &TrapArgs) -> Long {
    // Path argument(s) in the caller's space (ARCH03 3a, `fs::path_arg`).
    let mut path_buf: super::PathBuf = [0; super::PATH_ARG_MAX];
    let path = match super::path_arg(args, 0, &mut path_buf) {
        Ok(Some(p)) => p,
        Ok(None) => return EPTHNF,
        Err(_) => return crate::uaccess::FAULTED,
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

    open_handle(Ofd {
        s: Stream::new(vol, 0, 0),
        drive,
        dir_start: parent,
        dir_slot,
        mode: 0x01, // write access
        listed: true,
    })
    .unwrap_or(ENHNDL)
}

/// `Fread` (0x3F) — read `len` bytes from a handle into a buffer.
pub fn fread(args: &TrapArgs) -> Long {
    // The handle is a signed WORD: `-1..-3` name a character device.
    let handle = args.arg_i16(0);
    let len = args.arg_i32(1);
    let ubuf = args.arg_u64(2);

    // Route standard handles / redirects: a handle that ends on a character
    // device reads that device (`osif`, `bdos/bdosmain.c:608-620`: HUGE guard,
    // `conin`, `cgets`), a file handle reads the file (below).
    match resolve(handle) {
        Target::Device(numl) => {
            return crate::gemdos::console::device_read(numl, args.space, len, ubuf)
        }
        Target::None => return EIHNDL,
        Target::File(_) => {}
    }
    let ofd = match ofd_of(handle) {
        Some(o) => o,
        None => return EIHNDL,
    };
    match ixread(ofd, len, args.space, ubuf) {
        Ok(v) | Err(v) => v,
    }
}

/// EmuTOS `ixread` (`bdos/fsio.c:498-514`) on an open file: `len` bytes from
/// the file position into `ubuf` in `space`. `Ok` is the call's ordinary
/// result (the bytes moved); `Err` is a result that ends the GEMDOS call
/// wherever it came from — a device error (EmuTOS unwinds `osif` with the
/// BIOS code, `longjmp_rwabs`, `bdos/fs.h:60`) or a buffer the caller cannot
/// write ([`crate::uaccess::FAULTED`]).
fn ixread(ofd: &mut Ofd, len: i32, space: crate::uaccess::Space, ubuf: u64) -> Result<Long, Long> {
    // Clamp to the bytes before EOF; zero or negative requests move nothing.
    if len <= 0 {
        return Ok(0);
    }
    let want = (len as u32).min(ofd.s.fileln.saturating_sub(ofd.s.bytnum));
    if want == 0 {
        return Ok(0);
    }
    // `xrw(0, …)` (FS08, `docs/api/gemdos/_state.md` §3a): one FAT step per
    // new cluster from `o_curcl`, whole records straight into the caller's
    // buffer (in its space, ARCH03 3a), partial ones through the cache.
    let r = fsio::xrw(&mut ofd.s, false, want, space, ubuf);
    // A read that stopped on a device error reports the error, not a short
    // count (`docs/api/gemdos/file/3f-fread.md` §"Error conditions": an
    // underlying disk read failure surfaces as the BIOS code, `EREADF`); a
    // chain that ends early stays a short count.
    let pending = super::cache::take_error();
    match r {
        Err(Fail::Fault) => Err(crate::uaccess::FAULTED),
        Err(Fail::Device) => Err(pending.unwrap_or(EINTRN)),
        Ok(done) => match pending {
            Some(code) => Err(code),
            None => Ok(done as Long),
        },
    }
}

/// `Fwrite` (0x40) — write `len` bytes from a buffer to a handle,
/// extending the file (allocating clusters) as needed.
pub fn fwrite(args: &TrapArgs) -> Long {
    // The handle is a signed WORD: `-1..-3` name a character device.
    let handle = args.arg_i16(0);
    let len = args.arg_i32(1);
    let ubuf = args.arg_u64(2);

    // Route standard handles / redirects: a handle that ends on a character
    // device writes that device (`osif`, `bdos/bdosmain.c:622-640`: CON like
    // `Cconws`, others through `Bconout` with a short count on 0), a file
    // handle writes the file (below).
    match resolve(handle) {
        Target::Device(numl) => {
            return crate::gemdos::console::device_write(numl, args.space, len, ubuf)
        }
        Target::None => return EIHNDL,
        Target::File(_) => {}
    }
    let ofd = match ofd_of(handle) {
        Some(o) => o,
        None => return EIHNDL,
    };
    match ixwrite(ofd, len, args.space, ubuf) {
        Ok(v) | Err(v) => v,
    }
}

/// EmuTOS `ixwrite` (`bdos/fsio.c:554-557`) on an open file: `len` bytes from
/// `ubuf` in `space` at the file position. `Ok` / `Err` as in [`ixread`]:
/// `Err` is a device error or a fault, which ends the GEMDOS call.
fn ixwrite(ofd: &mut Ofd, len: i32, space: crate::uaccess::Space, ubuf: u64) -> Result<Long, Long> {
    if len <= 0 {
        return Ok(0);
    }
    if !ofd.s.vol.is_writable() {
        return Ok(EACCDN);
    }
    // `ixwrite` → `xrw(1, …)` (FS08, `docs/api/gemdos/_state.md` §3a):
    // clusters allocated by `nextcl` as the write runs past the chain, whole
    // records straight from the caller's buffer, partial ones
    // read-modify-write through the cache. A disk that fills up ends the
    // transfer with a short count.
    let r = fsio::xrw(&mut ofd.s, true, len as u32, space, ubuf);
    if r == Err(Fail::Fault) {
        return Err(crate::uaccess::FAULTED);
    }
    // Write-back of everything this call dirtied. Its code wins over the
    // byte count: EmuTOS never returns a short count for a hard error, it
    // unwinds with `EWRITF` (`bdos/fs.h:60`).
    super::cache::flush()?;
    // ...and a transfer that stopped because the *device* failed is the
    // same hard error, not the "disk full" short count
    // (`docs/api/gemdos/file/40-fwrite.md` §"Error conditions": disk-full is
    // a short count, a media fault is a BIOS code).
    if let Some(code) = super::cache::take_error() {
        return Err(code);
    }
    match r {
        Ok(done) => Ok(done as Long),
        Err(_) => Err(EINTRN),
    }
}

// ---------------------------------------------------------------------------
// Standard handles for the character functions (TASK/PROC04 point 1).
// ---------------------------------------------------------------------------

/// `run->p_uft[std]` as the GEMDOS character functions read it
/// (`bdos/bdosmain.c:500`, `HXFORM(run->p_uft[n])` in `bdos/console.c`):
/// the running process's entry, or — kernel code with no running process —
/// the entry of the stand-in PD that is EmuTOS's `run` there
/// ([`crate::bdos::process::standin::kernel_run_uft`], always
/// `default_handle`; the same source `init_pd_files` inherits from). So
/// kernel code (desktop, EmuCON, boot) keeps CON/CON/AUX/PRN. `std` past
/// `NUMSTD` answers the console.
///
/// Deliberately separate from [`uft`], which the handle-based file functions
/// use and which answers `-1` for every standard handle without a process
/// (TASK/PROC04 point 1 leaves that path as it was; for `std` 0/1/4/5 the two
/// agree, for 2/3 the file path has always said CON).
pub(crate) fn std_handle(std: u16) -> i16 {
    let bp = crate::bdos::process::current_kva();
    let uft = if bp.is_null() {
        crate::bdos::process::standin::kernel_run_uft()
    } else {
        // SAFETY: `current_kva` is the running process's live basepage.
        unsafe { (*bp).p_uft }
    };
    uft.get(std as usize).copied().unwrap_or(-1) as i16
}

/// EmuTOS `xread(h, len, buf)` (`bdos/fsio.c:440-454`) for the character
/// functions redirected to a file (`osif`, `bdos/bdosmain.c:509-553`): `h`
/// through `getofd`, then [`ixread`]; a handle with no file behind it is
/// `Ok(EIHNDL)`. `Err` ends the GEMDOS call (device error, fault).
pub(crate) fn xread(h: i16, len: i32, space: crate::uaccess::Space, ubuf: u64) -> Result<Long, Long> {
    match getofd(h) {
        FOfd::File(j) => match ofds()[j].as_mut() {
            Some(ofd) => ixread(ofd, len, space, ubuf),
            None => Ok(EIHNDL),
        },
        _ => Ok(EIHNDL),
    }
}

/// EmuTOS `xwrite(h, len, buf)` (`bdos/fsio.c:528-548`) for the character
/// functions redirected to a file: [`ixwrite`] on `getofd(h)`, `Ok(EIHNDL)`
/// for a handle with no file behind it.
pub(crate) fn xwrite(h: i16, len: i32, space: crate::uaccess::Space, ubuf: u64) -> Result<Long, Long> {
    match getofd(h) {
        FOfd::File(j) => match ofds()[j].as_mut() {
            Some(ofd) => ixwrite(ofd, len, space, ubuf),
            None => Ok(EIHNDL),
        },
        _ => Ok(EIHNDL),
    }
}

/// EmuTOS `eof(h)` (`bdos/fsio.c:294-306`): `1` when the position of `h`'s
/// file is at or past its length, `0` before it, `EIHNDL` for a handle with no
/// file behind it.
pub(crate) fn eof(h: i16) -> Long {
    match getofd(h) {
        FOfd::File(j) => match ofds()[j].as_ref() {
            Some(ofd) if ofd.s.bytnum >= ofd.s.fileln => 1,
            Some(_) => 0,
            None => EIHNDL,
        },
        _ => EIHNDL,
    }
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
    for j in 0..OPEN_FILES {
        let hit = matches!(ofds()[j], Some(o) if o.dir_start == dir_start && o.dir_slot == dir_slot);
        if !hit {
            continue;
        }
        ofds()[j] = None;
        for e in sft().iter_mut() {
            if e.ofd == FOfd::File(j) {
                *e = FREE;
            }
        }
    }
}

/// Flush an open file's directory entry (size + start cluster).
fn flush_ofd(ofd: &Ofd) {
    let mut e = [0u8; 32];
    if dir::read_slot(&ofd.s.vol, ofd.dir_start, ofd.dir_slot, &mut e) {
        e[0x1A..0x1C].copy_from_slice(&((ofd.s.strtcl & 0xFFFF) as u16).to_le_bytes());
        e[0x1C..0x20].copy_from_slice(&ofd.s.fileln.to_le_bytes());
        dir::write_slot(&ofd.s.vol, ofd.dir_start, ofd.dir_slot, &e);
    }
}

/// `Fdelete` (0x41) — delete a file by path.
pub fn fdelete(args: &TrapArgs) -> Long {
    // Path argument(s) in the caller's space (ARCH03 3a, `fs::path_arg`).
    let mut path_buf: super::PathBuf = [0; super::PATH_ARG_MAX];
    let path = match super::path_arg(args, 0, &mut path_buf) {
        Ok(Some(p)) => p,
        Ok(None) => return EFILNF,
        Err(_) => return crate::uaccess::FAULTED,
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
    // Path argument(s) in the caller's space (ARCH03 3a, `fs::path_arg`).
    let mut pathstr_buf: super::PathBuf = [0; super::PATH_ARG_MAX];
    let pathstr = match super::path_arg(args, 0, &mut pathstr_buf) {
        Ok(Some(p)) => p,
        Ok(None) => return EPTHNF,
        Err(_) => return crate::uaccess::FAULTED,
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
    // Path argument(s) in the caller's space (ARCH03 3a, `fs::path_arg`).
    let mut src_buf: super::PathBuf = [0; super::PATH_ARG_MAX];
    let src = match super::path_arg(args, 1, &mut src_buf) {
        Ok(Some(p)) => p,
        Ok(None) => return EPTHNF,
        Err(_) => return crate::uaccess::FAULTED,
    };
    let mut dst_buf: super::PathBuf = [0; super::PATH_ARG_MAX];
    let dst = match super::path_arg(args, 2, &mut dst_buf) {
        Ok(Some(p)) => p,
        Ok(None) => return EPTHNF,
        Err(_) => return crate::uaccess::FAULTED,
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
    let handle = args.arg_i16(1);
    let wrt = args.arg_u16(2);

    let ofd = match ofd_of(handle) {
        Some(o) => o,
        None => return EIHNDL,
    };
    let mut e = [0u8; 32];
    if !dir::read_slot(&ofd.s.vol, ofd.dir_start, ofd.dir_slot, &mut e) {
        return EIHNDL;
    }
    // The 4-byte DOSTIME buffer is in the caller's space (ARCH03 3a).
    if wrt == 0 {
        // Get: copy the entry's time (0x16) + date (0x18) to the buffer.
        if args.space.copy_to(buf, &e[0x16..0x1A]).is_err() {
            return crate::uaccess::FAULTED;
        }
    } else {
        // Set: copy time/date from the buffer into the entry.
        if args.space.copy_from(&mut e[0x16..0x1A], buf).is_err() {
            return crate::uaccess::FAULTED;
        }
        dir::write_slot(&ofd.s.vol, ofd.dir_start, ofd.dir_slot, &e);
        if let Err(code) = super::cache::flush() {
            return code;
        }
    }
    E_OK
}

/// `Fseek` (0x42) — reposition a handle's file pointer.
///
/// The handle (a signed WORD) is resolved first, as EmuTOS `osif` does for
/// `Fseek` (`bdos/bdosmain.c:571-642`; spec `42-fseek.md` §"Character-device
/// and standard handles"): one that ends on a character device answers `0`
/// whatever `n` and `flg` are — `xlseek` never runs, so an invalid mode is
/// not `EINVFN` there; an unmapped handle or a device below `-3` is
/// `EIHNDL`; a file seeks.
pub fn fseek(args: &TrapArgs) -> Long {
    let n = args.arg_i32(0) as i64;
    let handle = args.arg_i16(1);
    let flg = args.arg_u16(2);

    let ofd = match resolve(handle) {
        Target::Device(_) => return 0,
        Target::None => return EIHNDL,
        Target::File(j) => match ofds().get_mut(j).and_then(Option::as_mut) {
            Some(o) => o,
            None => return EIHNDL,
        },
    };
    // `xlseek` (`bdos/fsio.c:321-337`) folds the mode into an absolute
    // target; `ixlseek` positions the cluster fields (FS08, `fsio`).
    let base = match flg {
        0 => 0,                       // SEEK_SET
        1 => ofd.s.bytnum as i64,     // SEEK_CUR
        2 => ofd.s.fileln as i64,     // SEEK_END
        _ => return EINVFN,
    };
    fsio::ixlseek(&mut ofd.s, base + n)
}

/// `Fclose` (0x3E) — close a handle (EmuTOS `xclose`,
/// `bdos/fsopnclo.c:426-496`; spec `docs/api/gemdos/file/3e-fclose.md`).
pub fn fclose(args: &TrapArgs) -> Long {
    xclose(args.arg_i16(0))
}

/// EmuTOS `xclose`: a negative handle is a good close of a character device;
/// a standard handle reverts to its `default_handle` and the handle it was
/// forced to is closed; a non-standard entry mapped to a BIOS device only
/// loses a use; anything else is a real close (`ixclose`) that loses a use,
/// and the entry goes at zero (`sftdel`).
fn xclose(h: i16) -> Long {
    if h < 0 {
        return E_OK; // always a good close on a character device
    }
    if h as usize >= NUMHANDLES {
        return EIHNDL;
    }
    let mut h = h;
    if h < NUMSTD as i16 {
        let h0 = uft(h as u16); // remember old mapping
        let default = crate::bdos::process::standin::DEFAULT_HANDLE[h as usize];
        set_uft(h as u16, default as i16); // revert to default
        h = h0;
        if h < 0 {
            return E_OK;
        }
        if h < NUMSTD as i16 {
            return EIHNDL; // "can't happen" (bug in Fforce()?)
        }
    } else {
        let e = &mut sft()[(h - NUMSTD as i16) as usize];
        if let FOfd::Bios(_) = e.ofd {
            e.use_ -= 1;
            if e.use_ == 0 {
                *e = FREE;
            }
            return E_OK;
        }
    }

    let FOfd::File(j) = getofd(h) else {
        return EIHNDL;
    };
    let rc = ixclose(j);

    let n = (h - NUMSTD as i16) as usize;
    sft()[n].use_ -= 1;
    if sft()[n].use_ == 0 {
        sftdel(n);
    }
    rc
}

/// EmuTOS `ixclose(fd, 0)` (`bdos/fsopnclo.c:511-581`): write a dirty
/// directory entry back, take the OFD off its directory's open-file list —
/// `EINTRN` if it is no longer there (an earlier close of a handle sharing it
/// took it off), returned before the buffer flush — then write back every
/// dirty buffer. A write-back failure is the result: the file on the card is
/// not the file the application wrote (EmuTOS unwinds with the BIOS code,
/// `bdos/fs.h:60`); the caller releases the handle either way.
fn ixclose(j: usize) -> Long {
    let Some(ofd) = ofds()[j].as_mut() else {
        return EIHNDL;
    };
    if ofd.s.dirty {
        flush_ofd(ofd);
        ofd.s.dirty = false;
    }
    if !ofd.listed {
        return EINTRN; // some kind of internal error
    }
    ofd.listed = false;
    match super::cache::flush() {
        Ok(()) => E_OK,
        Err(code) => code,
    }
}

/// EmuTOS `sftdel` (`bdos/fsopnclo.c:380-410`): clear the entry, and free its
/// OFD unless another entry still points at it (`sftofdsrch`).
fn sftdel(n: usize) {
    let ofd = sft()[n].ofd;
    sft()[n] = FREE;
    if let FOfd::File(j) = ofd {
        if !sft().iter().any(|e| e.ofd == ofd) {
            ofds()[j] = None;
        }
    }
}

/// The file half of EmuTOS `ixterm` (`bdos/proc.c:116-123`), for the
/// process `r` that terminates (`Pterm`, `Pterm0`, `Ptermres`): close every
/// standard handle it redirected to a file (`p_uft[i] > 0`), then every
/// `sft[]` entry it owns.
///
/// # Safety
///
/// `r` is the user address of a live basepage (its kernel alias readable).
pub unsafe fn close_process_files(r: *mut tos_types::BasePage) {
    if r.is_null() {
        return;
    }
    let kva = crate::bdos::process::kalias(r as u64) as *const tos_types::BasePage;
    // SAFETY: caller contract.
    let uft = unsafe { (*kva).p_uft };
    for h in uft {
        if h > 0 {
            let _ = xclose(h as i16);
        }
    }
    for i in 0..OPEN_FILES {
        if sft()[i].own == Some(r as u64) {
            let _ = xclose((i + NUMSTD as usize) as i16);
        }
    }
}

/// `Fdup` (0x45) — duplicate standard handle `h` (0..NUMSTD) into a fresh
/// non-standard handle (EmuTOS `xdup`, `bdos/fshand.c:93-123`): the first
/// entry without an owner becomes `run`'s with `f_use = 1` and the current
/// mapping — the OFD of the entry `h` is forced to (shared, no use taken on
/// that entry), or `h`'s BIOS handle. Returns the new handle, `EIHNDL` for a
/// non-standard `h`, `ENHNDL` when every entry is owned.
pub fn fdup(args: &TrapArgs) -> Long {
    let h = args.arg_i16(0);
    if h < 0 || h >= NUMSTD as i16 {
        return EIHNDL; // only dup standard
    }
    let t = uft(h as u16);
    let ofd = if t > 0 { sft_ofd(t) } else { FOfd::bios(t) };
    match claim_sft(ofd) {
        Some(i) => (i as u16 + NUMSTD) as Long,
        None => ENHNDL,
    }
}

/// `Fforce` (0x46) — redirect standard handle `std` of `run` to `h`
/// (EmuTOS `xforce` = `ixforce(std, h, run)`, `bdos/fshand.c:32-35`).
pub fn fforce(args: &TrapArgs) -> Long {
    let bp = crate::bdos::process::current_kva();
    // SAFETY: `current_kva` is the running process's live basepage or null.
    unsafe { ixforce(args.arg_i16(0), args.arg_i16(1), bp) }
}

/// EmuTOS `ixforce` (`bdos/fshand.c:46-77`): validate the standard handle
/// `std`; a BIOS handle `h < 0` is stored as is; a non-standard `h` must have
/// an OFD — if its entry is mapped to a BIOS device that handle is stored,
/// otherwise `h` itself and the entry gains a use. Whatever `p_uft[std]`
/// mapped before is not released. Also the launcher's hook
/// ([`crate::bdos::process::loader::init_pd_files`]).
///
/// `p` is the kernel alias of the PD to change; null (kernel code with no
/// running process — the stand-ins are read-only) validates only: nothing is
/// stored and no use is taken.
///
/// # Safety
///
/// `p` is null or a live basepage.
pub unsafe fn ixforce(std: i16, h: i16, p: *mut tos_types::BasePage) -> Long {
    if std < 0 || std >= NUMSTD as i16 {
        return EIHNDL; // validate standard handle
    }
    let store = |v: i16| {
        if !p.is_null() {
            // SAFETY: caller contract; `std` is in 0..NUMSTD.
            unsafe { (*p).p_uft[std as usize] = v as i8 };
        }
    };
    if h < 0 {
        store(h); // a BIOS handle: just store it as-is
        return E_OK;
    }
    if h < NUMSTD as i16 {
        return EIHNDL; // validate the non-std handle
    }
    match getofd(h) {
        FOfd::Null => EIHNDL, // the non-std handle does not exist
        FOfd::Bios(fh) => {
            store(fh);
            E_OK
        }
        FOfd::File(_) => {
            if !p.is_null() {
                store(h);
                sft()[(h - NUMSTD as i16) as usize].use_ += 1;
            }
            E_OK
        }
    }
}
