//! GEMDOS directory create / remove: `Dcreate` / `Ddelete`. Mirrors
//! EmuTOS `bdos/fsdir.c` (`xmkdir` / `xrmdir`) in reduced form.
//!
//! Spec: `docs/api/gemdos/directory/{39-dcreate,3a-ddelete}.md`.

use super::dir;
use super::drive::current_drive;
use super::fat::Volume;
use super::path;
use crate::gemdos::time::dos_now;
use crate::trap::TrapArgs;
use tos_types::error::{EACCDN, EDRIVE, EPTHNF, E_OK};
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
    // Path argument(s) in the caller's space (ARCH03 3a, `fs::path_arg`).
    let mut pathstr_buf: super::PathBuf = [0; super::PATH_ARG_MAX];
    let pathstr = match super::path_arg(args, 0, &mut pathstr_buf) {
        Ok(Some(p)) => p,
        Ok(None) => return EPTHNF,
        Err(_) => return crate::uaccess::FAULTED,
    };
    // Resolve the parent directory but keep the *raw* final component, so the
    // illegal-character check runs against the user's folder name before it
    // is folded to an 8.3 FCB name.
    let (vol, parent, comp) = match path::resolve_parent(pathstr) {
        Ok(x) => x,
        // `Dcreate` resolves its directory portion (an unmountable drive
        // included) to `EPTHNF` (EmuTOS `xmkdir`→`ixcreat` `findit` NULL →
        // `EPTHNF`, `fsopnclo.c:114`).
        Err(e) => return path::drive_notfound(e, EPTHNF),
    };
    // `Dcreate` is `xmkdir`, which is `ixcreat(s, FA_SUBDIR)`
    // (`bdos/fsdir.c:211`), so it inherits `ixcreat`'s guard against a path
    // with no final component or one starting with a dot
    // (`bdos/fsopnclo.c:117-118`: `if (!*s || (*s == '.')) return EPTHNF;`).
    if comp.is_empty() || comp.starts_with('.') {
        return EPTHNF;
    }
    // F7: reject illegal characters in the new folder name (EmuTOS
    // `xmkdir`→`ixcreat` `contains_illegal_characters(s)` → `EACCDN`,
    // `fsopnclo.c:121-122`).
    if path::contains_illegal_characters(comp) {
        return EACCDN;
    }
    let name11 = match dir::fcb_name(comp) {
        Some(n) => n,
        None => return EPTHNF,
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

    // Link the new directory into the parent. F5: a full non-root parent
    // grows by a cluster; only the fixed-size root still fails here.
    let slot = match dir::find_free_slot_grow(&vol, parent) {
        Some(s) => s,
        None => {
            vol.free_chain(cl);
            return EACCDN; // parent directory full (root is fixed-size)
        }
    };
    let fcb = dir::build_fcb(&name11, FA_SUBDIR, cl, 0, time, date);
    if !dir::write_slot(&vol, parent, slot, &fcb) {
        vol.free_chain(cl);
        return EACCDN;
    }
    if let Err(code) = super::cache::flush() {
        return code;
    }
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
    // Path argument(s) in the caller's space (ARCH03 3a, `fs::path_arg`).
    let mut pathstr_buf: super::PathBuf = [0; super::PATH_ARG_MAX];
    let pathstr = match super::path_arg(args, 0, &mut pathstr_buf) {
        Ok(Some(p)) => p,
        Ok(None) => return EPTHNF,
        Err(_) => return crate::uaccess::FAULTED,
    };
    let (vol, parent, name11) = match path::findit(pathstr) {
        Ok(x) => x,
        // A path that does not resolve to a folder — an unmountable drive
        // included — returns `EPTHNF` (EmuTOS `xrmdir` `findit` mode-1 NULL →
        // `EPTHNF`, `fsdir.c:307`).
        Err(e) => return path::drive_notfound(e, EPTHNF),
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

    // EmuTOS `xrmdir` ends in `ixdel(d1, fcb, pos)` (`bdos/fsdir.c:377`), the
    // same routine as `Fdelete`: owner check on the OFDs at that position,
    // free the chain, erase the entry — here with FS01's long-name slots in
    // front of it (a folder created by a foreign system carries them just
    // like a file, and leaving them behind orphans the chain).
    if let Err(code) = super::file::ixdel(&vol, parent, slot) {
        return code;
    }
    if let Err(code) = super::cache::flush() {
        return code;
    }
    E_OK
}

/// `Dsetpath` (0x3B) — set the current directory for the path's drive in
/// `run`'s PD (`xchdir`, `bdos/fsdir.c:1113-1148`: `run->p_curdir[dlog]`,
/// TASK/PROC04 point 2, [`super::curdir`]). Resolves `.`/`..` and stores the
/// canonical absolute path (no drive letter). The target must be an existing
/// directory; on any failure `run`'s PD is unchanged.
pub fn dsetpath(args: &TrapArgs) -> Long {
    // Path argument(s) in the caller's space (ARCH03 3a, `fs::path_arg`).
    let mut p_buf: super::PathBuf = [0; super::PATH_ARG_MAX];
    let p = match super::path_arg(args, 0, &mut p_buf) {
        Ok(Some(p)) => p,
        Ok(None) => return EPTHNF,
        Err(_) => return crate::uaccess::FAULTED,
    };
    if p.bytes().any(|c| c == b'*' || c == b'?') {
        return EPTHNF;
    }
    let (drive, rest) = path::split_drive(p);
    // An invalid or unmounted drive prefix surfaces as `EPTHNF`, not `EDRIVE`:
    // EmuTOS resolves the whole path through `findit`→`dcrack`→`ckdrv`, whose
    // failure collapses to `xchdir`'s `findit` NULL → `EPTHNF`
    // (`fsdir.c:1135`; spec `3b-dsetpath.md` error table).
    let vol = match Volume::mount(drive) {
        Some(v) => v,
        None => return EPTHNF,
    };

    // Build the component stack: seed from the current directory unless
    // the path is absolute, then apply the path resolving "." / "..".
    let mut comps: [&str; 32] = [""; 32];
    let mut depth = 0usize;
    let absolute = matches!(rest.as_bytes().first(), Some(b'/') | Some(b'\\'));
    let cur = super::curdir::curdir(drive);
    if !absolute {
        for c in cur.as_str().split(['/', '\\']).filter(|c| !c.is_empty()) {
            if depth >= comps.len() {
                return EPTHNF;
            }
            comps[depth] = c;
            depth += 1;
        }
    }
    let body = if absolute { &rest[1..] } else { rest };
    for c in body.split(['/', '\\']).filter(|c| !c.is_empty()) {
        match c {
            "." => {}
            ".." => depth = depth.saturating_sub(1),
            _ => {
                if depth >= comps.len() {
                    return EPTHNF;
                }
                comps[depth] = c;
                depth += 1;
            }
        }
    }

    // Validate: descend from root; every component must be a directory.
    let mut start = 0u32;
    for c in &comps[..depth] {
        let name = match dir::fcb_name(c) {
            Some(n) => n,
            None => return EPTHNF,
        };
        match dir::find_in_dir(&vol, start, &name) {
            Some(e) if e.attrib & FA_SUBDIR != 0 => start = e.cluster,
            _ => return EPTHNF,
        }
    }

    // Build the canonical "\A\B" string (uppercased).
    let mut buf = [0u8; 64];
    let mut len = 0;
    for c in &comps[..depth] {
        if len + 1 + c.len() >= buf.len() {
            return EPTHNF;
        }
        buf[len] = b'\\';
        len += 1;
        for &b in c.as_bytes() {
            buf[len] = b.to_ascii_uppercase();
            len += 1;
        }
    }
    let s = core::str::from_utf8(&buf[..len]).unwrap_or("");
    // `incr_curdir_usage` + `run->p_curdir[dlog] = newdir` +
    // `decr_curdir_usage(olddir)` (`fsdir.c:1140-1151`); a full table is
    // `EINTRN`, answered as `EPTHNF` (`:1143-1144`).
    if !super::curdir::set_curdir(drive, s) {
        return EPTHNF;
    }
    E_OK
}

/// `Dgetpath` (0x47) — write `run`'s current directory of `drv` (0 = current,
/// 1 = A:, …) into `buf` as an absolute path without the drive letter
/// (`xgetdir`, `bdos/fsdir.c:1229-1251`: `dirtbl[run->p_curdir[drv]]`).
pub fn dgetpath(args: &TrapArgs) -> Long {
    let buf = args.arg_u64(0);
    let drv = args.arg_u16(1);
    let drive = if drv == 0 { current_drive() } else { drv - 1 };

    // `buf` is the caller's path buffer (>= LEN_ZPATH bytes), in the
    // caller's space (ARCH03 3a, `crate::uaccess`).
    if Volume::mount(drive).is_none() {
        if args.space.put_u8(buf, 0).is_err() {
            return crate::uaccess::FAULTED;
        }
        return EDRIVE;
    }
    let cur = super::curdir::curdir(drive);
    let s = cur.as_str();
    // The path bytes plus a NUL terminator.
    if args.space.copy_to(buf, s.as_bytes()).is_err()
        || args.space.put_u8(buf + s.len() as u64, 0).is_err()
    {
        return crate::uaccess::FAULTED;
    }
    E_OK
}
