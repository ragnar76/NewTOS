//! DTA-based directory search: `Fsfirst` / `Fsnext`, plus `Fsetdta` /
//! `Fgetdta`. Mirrors EmuTOS `bdos/fsdir.c` (`xsfirst`/`xsnext`) and
//! `bdos/fsmain.c` (`xsetdta`/`xgetdta`).
//!
//! Spec: `docs/api/gemdos/file/{4e-fsfirst,4f-fsnext}.md`,
//! `docs/api/gemdos/dta/{1a-fsetdta,2f-fgetdta}.md`,
//! `docs/api/structures/dta.md`.
//!
//! Searches resolve through sub-directories (`findit`). The 21-byte DTA
//! reserved area carries our own resume state — its internal layout is
//! explicitly **not** part of the ABI contract, so we are free to define
//! it:
//!
//! ```text
//!  [0..4]  next raw directory slot to resume at (u32 LE); !0 = exhausted
//!  [4..8]  parent directory first cluster (u32 LE; 0 = root)
//!  [8]     drive number
//!  [9]     effective attribute mask
//!  [10..21] 11-byte FAT search pattern ('?' = wildcard)
//! ```
//!
//! The current-DTA pointer is a global for now (single foreground
//! caller), matching the global-SFT bring-up choice; it moves into the
//! per-process `p_xdta` with full process integration.

use core::sync::atomic::{AtomicU64, Ordering};

use super::dir::DirEntry;
use super::fat::Volume;
use crate::trap::TrapArgs;
use tos_types::error::{EFILNF, ENMFIL, EPTHNF, E_OK};
use tos_types::Long;

/// FAT attribute bits (`bdosdefs.h`).
const FA_RO: u8 = 0x01;
const FA_HIDDEN: u8 = 0x02;
const FA_SYSTEM: u8 = 0x04;
const FA_VOL: u8 = 0x08;
const FA_SUBDIR: u8 = 0x10;
const FA_ARCHIVE: u8 = 0x20;
const FA_LFN: u8 = 0x0F;

const ERASED: u8 = 0xE5;
const END: u8 = 0x00;
const SLOTS_PER_SECTOR: u32 = 16;
/// Sentinel for "DTA not initialised / search exhausted".
const EXHAUSTED: u32 = u32::MAX;

/// Global current-DTA pointer (`p_xdta` stand-in).
static CUR_DTA: AtomicU64 = AtomicU64::new(0);

/// `Fsetdta` (0x1A) — set the current Disk Transfer Address.
pub fn fsetdta(args: &TrapArgs) -> Long {
    CUR_DTA.store(args.arg_u64(0), Ordering::Relaxed);
    E_OK
}

/// `Fgetdta` (0x2F) — return the current DTA pointer.
///
/// Note: the GEMDOS dispatch truncates to `Long` (i32); a user-space DTA
/// in low VA round-trips correctly, but a high-half kernel DTA does not
/// (a pre-existing native-ABI limitation shared with `Malloc`).
pub fn fgetdta(_args: &TrapArgs) -> Long {
    CUR_DTA.load(Ordering::Relaxed) as Long
}

/// Expand a `name.ext` pattern (with `*`/`?`) into the 11-byte FAT
/// short-name form where every wildcard position is `?`.
fn fcb_pattern(name: &str) -> [u8; 11] {
    let mut out = [b' '; 11];
    let b = name.as_bytes();
    let mut i = 0;

    // 8-char name field.
    let mut p = 0;
    while i < b.len() && b[i] != b'.' {
        let c = b[i].to_ascii_uppercase();
        if c == b'*' {
            while p < 8 {
                out[p] = b'?';
                p += 1;
            }
            while i < b.len() && b[i] != b'.' {
                i += 1;
            }
            break;
        }
        if p < 8 {
            out[p] = c;
            p += 1;
        }
        i += 1;
    }

    // 3-char extension field.
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let mut x = 8;
        while i < b.len() {
            let c = b[i].to_ascii_uppercase();
            if c == b'*' {
                while x < 11 {
                    out[x] = b'?';
                    x += 1;
                }
                break;
            }
            if x < 11 {
                out[x] = c;
                x += 1;
            }
            i += 1;
        }
    }
    out
}

/// Match an 11-byte FAT name against a pattern (`?` = any character).
fn pattern_match(pat: &[u8; 11], name: &[u8; 11]) -> bool {
    pat.iter().zip(name).all(|(&p, &n)| p == b'?' || p == n)
}

/// Whether an entry's attributes pass the search mask, per the EmuTOS
/// `scan` rules: `att == FA_VOL` matches labels exclusively; otherwise
/// volume labels are skipped and hidden/system/sub-dir entries appear
/// only if their bit is requested (FA_RO / FA_ARCHIVE never restrict).
fn attr_ok(att: u8, entry_attr: u8) -> bool {
    if att == FA_VOL {
        return entry_attr & FA_VOL != 0;
    }
    if entry_attr & FA_VOL != 0 {
        return false;
    }
    let restricted = FA_HIDDEN | FA_SYSTEM | FA_SUBDIR;
    entry_attr & restricted & !att == 0
}

/// Scan directory `dir` (first cluster; 0 = root) from raw slot `start`,
/// returning the next entry matching `pattern`/`att` together with the
/// slot index to resume at. `None` at the end-of-directory marker or once
/// the directory's sectors are exhausted.
fn scan(
    vol: &Volume,
    dir: u32,
    start: u32,
    pattern: &[u8; 11],
    att: u8,
) -> Option<(DirEntry, u32)> {
    let mut slot = start;
    let mut sbuf = [0u8; 512];
    let mut loaded = u32::MAX;

    loop {
        let sec = slot / SLOTS_PER_SECTOR;
        if sec != loaded {
            let rel = vol.dir_nth_sector(dir, sec)?;
            if !vol.read_rel_sector(rel, &mut sbuf) {
                return None;
            }
            loaded = sec;
        }
        let off = (slot % SLOTS_PER_SECTOR) as usize * 32;
        let e = &sbuf[off..off + 32];
        slot += 1;

        match e[0] {
            END => return None,
            ERASED => continue,
            _ => {}
        }
        if e[0x0B] == FA_LFN {
            continue;
        }
        let entry = DirEntry::parse(e);
        if attr_ok(att, entry.attrib) && pattern_match(pattern, &entry.name) {
            return Some((entry, slot));
        }
    }
}

/// Write the user-visible portion of the DTA from a found entry
/// (EmuTOS `makbuf`): attribute, time, date, size, `8.3\0` name. Values
/// are stored in native byte order (little-endian on AArch64).
///
/// # Safety
/// `dta` must point to a writable 44-byte DTA buffer.
unsafe fn makbuf(dta: u64, e: &DirEntry) {
    let p = dta as *mut u8;
    unsafe {
        *p.add(21) = e.attrib;
        p.add(22).copy_from(e.time.to_le_bytes().as_ptr(), 2);
        p.add(24).copy_from(e.date.to_le_bytes().as_ptr(), 2);
        p.add(26).copy_from(e.size.to_le_bytes().as_ptr(), 4);

        // d_fname: trim-padded "NAME.EXT\0" (offset 30, 14 bytes).
        let mut name = [0u8; 14];
        let mut n = 0;
        for &c in &e.name[0..8] {
            if c == b' ' {
                break;
            }
            name[n] = c;
            n += 1;
        }
        if e.name[8] != b' ' {
            name[n] = b'.';
            n += 1;
            for &c in &e.name[8..11] {
                if c == b' ' {
                    break;
                }
                name[n] = c;
                n += 1;
            }
        }
        // remaining bytes already zero (NUL terminator + pad)
        p.add(30).copy_from(name.as_ptr(), 14);
    }
}

/// Store our resume state into the DTA reserved area.
///
/// # Safety
/// `dta` must point to a writable 44-byte DTA buffer.
unsafe fn put_state(dta: u64, next_slot: u32, dir: u32, drive: u8, att: u8, pattern: &[u8; 11]) {
    let p = dta as *mut u8;
    unsafe {
        p.copy_from(next_slot.to_le_bytes().as_ptr(), 4);
        p.add(4).copy_from(dir.to_le_bytes().as_ptr(), 4);
        *p.add(8) = drive;
        *p.add(9) = att;
        p.add(10).copy_from(pattern.as_ptr(), 11);
    }
}

/// Load our resume state from the DTA reserved area.
///
/// # Safety
/// `dta` must point to a readable 44-byte DTA buffer previously written
/// by [`put_state`].
unsafe fn get_state(dta: u64) -> (u32, u32, u8, u8, [u8; 11]) {
    let p = dta as *const u8;
    unsafe {
        let next = u32::from_le_bytes([*p, *p.add(1), *p.add(2), *p.add(3)]);
        let dir = u32::from_le_bytes([*p.add(4), *p.add(5), *p.add(6), *p.add(7)]);
        let drive = *p.add(8);
        let att = *p.add(9);
        let mut pat = [0u8; 11];
        core::ptr::copy_nonoverlapping(p.add(10), pat.as_mut_ptr(), 11);
        (next, dir, drive, att, pat)
    }
}

/// `Fsfirst` (0x4E) — begin a directory search, filling the current DTA.
pub fn fsfirst(args: &TrapArgs) -> Long {
    let dta = CUR_DTA.load(Ordering::Relaxed);
    if dta == 0 {
        return EFILNF;
    }
    // SAFETY: caller-supplied path pointer (kernel pointer at EL1).
    let path = match unsafe { crate::bdos::fs::cstr(args.arg_u64(0)) } {
        Some(p) => p,
        None => return EPTHNF,
    };
    let att = args.arg_u16(1) as u8;
    let eff_att = if att == FA_VOL {
        att
    } else {
        att | FA_ARCHIVE | FA_RO
    };

    // Resolve the parent directory; the final component is the (possibly
    // wildcarded) search pattern.
    let (vol, parent, comp) = match super::path::resolve_parent(path) {
        Ok(x) => x,
        Err(e) => return e,
    };
    let drive = super::path::split_drive(path).0 as u8;
    let pattern = fcb_pattern(comp);

    // Mark uninitialised, then search from slot 0 of the parent directory.
    // SAFETY: `dta` is the caller's 44-byte DTA buffer.
    unsafe { put_state(dta, EXHAUSTED, parent, drive, eff_att, &pattern) };
    match scan(&vol, parent, 0, &pattern, eff_att) {
        Some((entry, next)) => {
            // SAFETY: as above.
            unsafe {
                put_state(dta, next, parent, drive, eff_att, &pattern);
                makbuf(dta, &entry);
            }
            E_OK
        }
        None => EFILNF,
    }
}

/// `Fsnext` (0x4F) — return the next match into the current DTA.
pub fn fsnext(_args: &TrapArgs) -> Long {
    let dta = CUR_DTA.load(Ordering::Relaxed);
    if dta == 0 {
        return ENMFIL;
    }
    // SAFETY: `dta` is the caller's 44-byte DTA buffer.
    let (next, parent, drive, att, pattern) = unsafe { get_state(dta) };
    if next == EXHAUSTED {
        return ENMFIL;
    }
    let vol = match Volume::mount(drive as u16) {
        Some(v) => v,
        None => return ENMFIL,
    };
    match scan(&vol, parent, next, &pattern, att) {
        Some((entry, after)) => {
            // SAFETY: as above.
            unsafe {
                put_state(dta, after, parent, drive, att, &pattern);
                makbuf(dta, &entry);
            }
            E_OK
        }
        None => {
            // SAFETY: as above — mark exhausted.
            unsafe { put_state(dta, EXHAUSTED, parent, drive, att, &pattern) };
            ENMFIL
        }
    }
}
