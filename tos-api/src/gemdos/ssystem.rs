//! `Ssystem` — GEMDOS 0x154 (FreeMiNT extension).
//!
//! Spec: [`docs/api/gemdos/154-ssystem.md`]. Only the cookie subfunctions
//! (`S_GETCOOKIE`/`S_SETCOOKIE`/`S_DELCOOKIE`) are implemented; every other
//! mode returns `EINVFN` (NewTOS has no `ENOSYS`), so the surface stays
//! honest. Cookie storage: [`crate::cookie`].

use tos_types::error::{EINVAL, EINVFN, ENSMEM, ERROR, E_OK};
use tos_types::Long;

use crate::cookie;
use crate::trap::TrapArgs;

/// `Ssystem`'s GEMDOS function id. It is above `MAX_FN`, so the GEMDOS
/// dispatcher special-cases it before the static table lookup.
pub const SSYSTEM: usize = 0x154;

// Cookie subfunction modes (FreeMiNT `sys/mint/ssystem.h`).
const S_GETCOOKIE: u16 = 8;
const S_SETCOOKIE: u16 = 9;
const S_DELCOOKIE: u16 = 26;

/// `Ssystem(WORD mode, LONG arg1, LONG arg2)`: `mode` in `x0`, `arg1` in
/// `x1`, `arg2` in `x2`.
pub fn ssystem(args: &TrapArgs) -> Long {
    // `mode == -1` is the "does this call exist" feature probe. FreeMiNT's
    // `sys_s_system` pre-sets `r = E_OK` (`sys/ssystem.c:66`) and `case -1:`
    // only TRACEs + `break`s (`sys/ssystem.c:72-76`), so it falls through to
    // `return r;` with `r` still 0 — `E_OK` unconditionally, for exactly
    // `-1` (not any negative mode). Without this, a probe hits the wildcard
    // arm, sees `EINVFN`, and wrongly concludes `Ssystem` is absent (D-072).
    if args.arg_i16(0) == -1 {
        return E_OK;
    }
    match args.arg_u16(0) {
        S_GETCOOKIE => s_getcookie(args),
        S_SETCOOKIE => s_setcookie(args),
        S_DELCOOKIE => s_delcookie(args),
        _ => EINVFN,
    }
}

/// `S_GETCOOKIE` (8). `arg1` = tag, `arg2` = pointer to a LONG (or 0).
/// `tag == 0` → jar **capacity** (total slot count, not the occupied
/// count); small `tag` (high word 0) → the 1-based Nth cookie's tag
/// (enumeration); otherwise → the tag's value. With `arg2 == 0` the result
/// is returned in `x0`; otherwise it is written to `*arg2` and `E_OK` is
/// returned. Not-found differs by branch: a missing tag → `ERROR`
/// (FreeMiNT `EERROR`, `sys/cookie.c:508-509`), an out-of-range slot index
/// → `EINVAL` (`sys/cookie.c:458-462`).
fn s_getcookie(args: &TrapArgs) -> Long {
    let tag = args.arg_i32(1) as u32;
    let ret = args.arg_u64(2);

    // Each branch carries the error code to use if it yields `None`, so the
    // slot-index-overflow (`EINVAL`) and tag-miss (`ERROR`) cases stay
    // distinct (D-073). The `tag == 0` branch always returns `Some`.
    let (value, not_found) = if tag == 0 {
        (Some(cookie::capacity()), ERROR)
    } else if tag & 0xFFFF_0000 == 0 {
        (cookie::slot_tag(tag), EINVAL)
    } else {
        (cookie::get(tag), ERROR)
    };

    match value {
        Some(v) if ret != 0 => {
            // SAFETY: `arg2` is a user pointer; the unprivileged store
            // permission-checks it as EL0 and faults the caller (not the
            // kernel) on a bad pointer.
            unsafe { write_user_long(ret as *mut u8, v) };
            E_OK
        }
        Some(v) => v as Long,
        None => not_found,
    }
}

/// `S_SETCOOKIE` (9). `arg1` = tag, `arg2` = value. Add or update. An
/// invalid tag (any zero byte) → `EINVAL` (`sys/cookie.c:538-541`); a full
/// jar → `ENSMEM` (FreeMiNT `ENOMEM`, `sys/cookie.c:574-575`).
fn s_setcookie(args: &TrapArgs) -> Long {
    let tag = args.arg_i32(1) as u32;
    let value = args.arg_i32(2) as u32;
    if !cookie::valid_tag(tag) {
        return EINVAL;
    }
    if cookie::set(tag, value) {
        E_OK
    } else {
        ENSMEM // jar full
    }
}

/// `S_DELCOOKIE` (26). `arg1` = tag. Remove it. A tag that is not present →
/// `EINVAL` (`sys/cookie.c:606-607`) — deliberately distinct from
/// `S_GETCOOKIE`'s `ERROR` for a missing tag.
fn s_delcookie(args: &TrapArgs) -> Long {
    let tag = args.arg_i32(1) as u32;
    if cookie::del(tag) {
        E_OK
    } else {
        EINVAL
    }
}

/// Write a 32-bit value to a user pointer via an unprivileged store
/// (`sttr`) — permission-checked as EL0, like `Cconrs`'s `sttrb`.
///
/// # Safety
/// `p` must be a 4-byte-aligned writable user address.
unsafe fn write_user_long(p: *mut u8, v: u32) {
    // SAFETY: unprivileged word store; caller guarantees `p` is writable.
    unsafe {
        core::arch::asm!(
            "sttr {v:w}, [{p}]",
            p = in(reg) p,
            v = in(reg) v,
            options(nostack, preserves_flags),
        );
    }
}
