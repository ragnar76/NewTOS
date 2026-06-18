//! GEMDOS directory create / remove: `Dcreate` / `Ddelete`. Mirrors
//! EmuTOS `bdos/fsdir.c` (`xmkdir` / `xrmdir`) in reduced form.
//!
//! Spec: `docs/api/gemdos/directory/{39-dcreate,3a-ddelete}.md`.

use super::dir;
use super::fat::Volume;
use super::path;
use crate::gemdos::time::dos_now;
use crate::trap::TrapArgs;
use tos_types::error::{EACCDN, EPTHNF, E_OK};
use tos_types::Long;

const FA_SUBDIR: u8 = 0x10;
const SECTOR: usize = 512;

/// The 11-byte FAT name for `"."` / `".."` (dot-padded).
fn dots(n: usize) -> [u8; 11] {
    let mut name = [b' '; 11];
    for b in name.iter_mut().take(n) {
        *b = b'.';
    }
    name
}

/// `Dcreate` (0x39) — create a sub-directory.
pub fn dcreate(args: &TrapArgs) -> Long {
    // SAFETY: caller-supplied path pointer (kernel pointer at EL1).
    let pathstr = match unsafe { super::cstr(args.arg_u64(0)) } {
        Some(p) => p,
        None => return EPTHNF,
    };
    let (vol, parent, name11) = match path::findit(pathstr) {
        Ok(x) => x,
        Err(e) => return e,
    };
    if !vol.is_writable() {
        return EACCDN;
    }
    if dir::find_in_dir(&vol, parent, &name11).is_some() {
        return EACCDN; // name already exists
    }

    // Allocate and initialise the new directory cluster.
    let cl = match vol.alloc_cluster() {
        Some(c) => c,
        None => return EACCDN, // disk full
    };
    let (time, date) = dos_now();
    let mut sec0 = [0u8; SECTOR];
    // "." points to the new directory; ".." to the parent (0 = root).
    sec0[0..32].copy_from_slice(&dir::build_fcb(&dots(1), FA_SUBDIR, cl, 0, time, date));
    sec0[32..64].copy_from_slice(&dir::build_fcb(&dots(2), FA_SUBDIR, parent, 0, time, date));
    if !vol.write_cluster_sector(cl, 0, &sec0) {
        vol.free_chain(cl);
        return EACCDN;
    }
    // Zero the rest of the cluster so the end-of-directory marker holds.
    let zero = [0u8; SECTOR];
    for s in 1..vol.clsiz {
        vol.write_cluster_sector(cl, s, &zero);
    }

    // Link the new directory into the parent.
    let slot = match dir::find_free_slot(&vol, parent) {
        Some(s) => s,
        None => {
            vol.free_chain(cl);
            return EACCDN; // parent directory full
        }
    };
    let fcb = dir::build_fcb(&name11, FA_SUBDIR, cl, 0, time, date);
    if !dir::write_slot(&vol, parent, slot, &fcb) {
        vol.free_chain(cl);
        return EACCDN;
    }
    super::cache::flush();
    E_OK
}

/// Whether a sub-directory contains only its `"."` / `".."` entries.
fn dir_is_empty(vol: &Volume, cluster: u32) -> bool {
    for e in dir::DirIter::new(vol, cluster) {
        let dot = e.name[0] == b'.'
            && (e.name[1] == b' ' || (e.name[1] == b'.' && e.name[2] == b' '));
        if !dot {
            return false;
        }
    }
    true
}

/// `Ddelete` (0x3A) — remove an empty sub-directory.
pub fn ddelete(args: &TrapArgs) -> Long {
    // SAFETY: caller-supplied path pointer (kernel pointer at EL1).
    let pathstr = match unsafe { super::cstr(args.arg_u64(0)) } {
        Some(p) => p,
        None => return EPTHNF,
    };
    let (vol, parent, name11) = match path::findit(pathstr) {
        Ok(x) => x,
        Err(e) => return e,
    };
    let entry = match dir::find_in_dir(&vol, parent, &name11) {
        Some(e) => e,
        None => return EPTHNF, // folder not found
    };
    if entry.attrib & FA_SUBDIR == 0 {
        return EACCDN; // not a directory
    }
    if !dir_is_empty(&vol, entry.cluster) {
        return EACCDN; // directory not empty
    }
    let slot = match dir::slot_of(&vol, parent, &name11) {
        Some(s) => s,
        None => return EPTHNF,
    };

    vol.free_chain(entry.cluster);
    let mut e = [0u8; 32];
    if dir::read_slot(&vol, parent, slot, &mut e) {
        e[0] = 0xE5; // erased marker
        dir::write_slot(&vol, parent, slot, &e);
    }
    super::cache::flush();
    E_OK
}
