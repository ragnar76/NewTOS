//! Path resolution (`findit`). Mirrors the role of EmuTOS `bdos/fsdir.c`
//! `findit` in reduced form: split a path into a drive, a parent
//! directory (addressed by its first cluster, 0 = root) and a final 8.3
//! component, descending through intermediate sub-directories.
//!
//! Current-directory-relative resolution is rooted at the volume root for
//! now; `Dsetpath` makes the per-drive current directory real.

use super::dir;
use super::drive::current_drive;
use super::fat::Volume;
use tos_types::error::{EDRIVE, EPTHNF};
use tos_types::Long;

const FA_SUBDIR: u8 = 0x10;

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

    // Absolute (leading separator) or relative — both start at the root
    // for now (current directory is the root until Dsetpath).
    let mut start = 0u32;
    let mut s = rest;
    if let Some(first) = s.as_bytes().first() {
        if *first == b'/' || *first == b'\\' {
            s = &s[1..];
        }
    }

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

/// The 11-byte FAT name for the `".."` entry (`b".."` + spaces).
fn fcb_dotdot() -> [u8; 11] {
    let mut n = [b' '; 11];
    n[0] = b'.';
    n[1] = b'.';
    n
}
