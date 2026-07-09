//! Path resolution (`findit`). Mirrors the role of EmuTOS `bdos/fsdir.c`
//! `findit` in reduced form: split a path into a drive, a parent
//! directory (addressed by its first cluster, 0 = root) and a final 8.3
//! component, descending through intermediate sub-directories.
//!
//! Current-directory-relative resolution is rooted at the volume root for
//! now; `Dsetpath` makes the per-drive current directory real.

use core::cell::UnsafeCell;

use super::dir;
use super::drive::current_drive;
use super::fat::Volume;
use tos_types::error::{EDRIVE, EPTHNF};
use tos_types::Long;

const FA_SUBDIR: u8 = 0x10;

/// Maximum stored current-directory path length (per drive), including
/// the leading `\` and NUL terminator.
const PATH_MAX: usize = 64;

struct CurDirs(UnsafeCell<[[u8; PATH_MAX]; 16]>);
// SAFETY: single-CPU, IRQ-off trap path.
unsafe impl Sync for CurDirs {}
/// Per-drive current directory, stored as an absolute path without the
/// drive letter (e.g. `b"\\SUB"`); empty = root. Stand-in for the
/// per-process `p_curdir` until process integration.
static CURDIR: CurDirs = CurDirs(UnsafeCell::new([[0u8; PATH_MAX]; 16]));

/// The current-directory string for `drive` (absolute, no drive letter;
/// empty at root).
pub fn curdir_str(drive: u16) -> &'static str {
    if drive as usize >= 16 {
        return "";
    }
    // SAFETY: single trap path; buffers are valid UTF-8 ASCII paths.
    let buf = unsafe { &(*CURDIR.0.get())[drive as usize] };
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    core::str::from_utf8(&buf[..end]).unwrap_or("")
}

/// Store the current-directory string for `drive`. `false` if it would
/// overflow `PATH_MAX`.
pub fn set_curdir(drive: u16, path: &str) -> bool {
    if drive as usize >= 16 || path.len() >= PATH_MAX {
        return false;
    }
    // SAFETY: single trap path.
    let buf = unsafe { &mut (*CURDIR.0.get())[drive as usize] };
    *buf = [0u8; PATH_MAX];
    buf[..path.len()].copy_from_slice(path.as_bytes());
    true
}

/// Resolve a drive's current directory to its first cluster (0 = root).
fn current_dir_cluster(vol: &Volume, drive: u16) -> u32 {
    let cur = curdir_str(drive);
    let mut start = 0u32;
    for comp in cur.split(['/', '\\']).filter(|c| !c.is_empty()) {
        match dir::fcb_name(comp).and_then(|n| dir::find_in_dir(vol, start, &n)) {
            Some(e) if e.attrib & FA_SUBDIR != 0 => start = e.cluster,
            _ => return 0, // stale current dir → fall back to root
        }
    }
    start
}

/// Split an optional `X:` drive prefix from a path. Returns the drive
/// index (current drive if absent) and the remaining path.
pub fn split_drive(path: &str) -> (u16, &str) {
    let b = path.as_bytes();
    if b.len() >= 2 && b[1] == b':' {
        (
            (b[0].to_ascii_uppercase().wrapping_sub(b'A')) as u16,
            &path[2..],
        )
    } else {
        (current_drive(), path)
    }
}

/// Resolve a path to `(volume, parent-directory-cluster, final-component)`.
/// Descends through intermediate sub-directories; the final component is
/// returned **as a string** (so `Fsfirst` can keep its wildcards). An
/// intermediate component that is missing or is not a directory yields
/// `EPTHNF`.
pub fn resolve_parent(path: &str) -> Result<(Volume, u32, &str), Long> {
    let (drive, rest) = split_drive(path);
    let vol = Volume::mount(drive).ok_or(EDRIVE)?;

    // A leading separator means absolute (from root); otherwise the path
    // is relative to the drive's current directory.
    let absolute = matches!(rest.as_bytes().first(), Some(b'/') | Some(b'\\'));
    let mut start = if absolute {
        0
    } else {
        current_dir_cluster(&vol, drive)
    };
    let s = if absolute { &rest[1..] } else { rest };

    let mut comps = s.split(['/', '\\']).filter(|c| !c.is_empty());
    let mut comp = comps.next().ok_or(EPTHNF)?;
    for next in comps {
        // `comp` is an intermediate component → descend into it.
        match comp {
            "." => {} // stay in the current directory
            ".." => {
                let dotdot = fcb_dotdot();
                start = match dir::find_in_dir(&vol, start, &dotdot) {
                    Some(e) => e.cluster,
                    None => 0,
                };
            }
            _ => {
                let name = dir::fcb_name(comp).ok_or(EPTHNF)?;
                let entry = dir::find_in_dir(&vol, start, &name).ok_or(EPTHNF)?;
                if entry.attrib & FA_SUBDIR == 0 {
                    return Err(EPTHNF);
                }
                start = entry.cluster;
            }
        }
        comp = next;
    }
    // `comp` is now the final component.
    Ok((vol, start, comp))
}

/// Resolve a path to `(volume, parent-directory-cluster, final-name)` as
/// an 8.3 FAT name. The final component is **not** required to exist.
pub fn findit(path: &str) -> Result<(Volume, u32, [u8; 11]), Long> {
    let (vol, parent, comp) = resolve_parent(path)?;
    let name = dir::fcb_name(comp).ok_or(EPTHNF)?;
    Ok((vol, parent, name))
}

/// Collapse a path-resolution failure caused by an **unmountable drive**
/// (`Volume::mount` → `EDRIVE`) to a syscall-specific "not found" code,
/// exactly as EmuTOS' `dcrack`→`ckdrv(<0)`→`findit` NULL does: `dcrack`
/// throws away the `ckdrv` error and returns NULL, so the caller's
/// `findit`-NULL branch, not `EDRIVE`, decides the observable code. Only the
/// drive error is remapped; a missing directory component keeps its own
/// `EPTHNF`. Non-drive errors pass through unchanged.
///
/// EmuTOS per-caller mapping (verified against `bdos/` @ `1a7484ab`):
///   * `Fopen` (`ixopen`) / `Fdelete` (`ixunlink`) → `EFILNF`
///   * `Fcreate` (`ixcreat`), `Fattrib` (`xchmod`), `Frename` (`xrename`),
///     `Fsfirst` (`ixsfirst`), `Dcreate` (`xmkdir`), `Ddelete` (`xrmdir`),
///     `Dsetpath` (`xchdir`) → `EPTHNF`
pub fn drive_notfound(e: Long, notfound: Long) -> Long {
    if e == EDRIVE {
        notfound
    } else {
        e
    }
}

/// EmuTOS' `ILLEGAL_FNAME_CHARACTERS` set (`bdos/fsopnclo.c:64`): a space
/// and the ten characters `*,:;<=>?[]|`. `true` if `name` contains any of
/// them. Checked by `Fcreate` / `Dcreate` (`ixcreat`, `fsopnclo.c:121`) and
/// `Frename` (`xrename`, `fsdir.c:985`) against the **user-supplied path's
/// final component** — before it is folded to an 8.3 FCB name, so the
/// rejected characters are still visible — and yields `EACCDN`. Wildcard
/// searches (`Fsfirst`) deliberately do **not** run this check, so `*`/`?`
/// stay legal there.
pub fn contains_illegal_characters(name: &str) -> bool {
    const ILLEGAL: &[u8] = b" *,:;<=>?[]|";
    name.bytes().any(|c| ILLEGAL.contains(&c))
}

/// The 11-byte FAT name for the `".."` entry (`b".."` + spaces).
fn fcb_dotdot() -> [u8; 11] {
    let mut n = [b' '; 11];
    n[0] = b'.';
    n[1] = b'.';
    n
}
