//! `Ssystem` — GEMDOS 0x154 (FreeMiNT extension).
//!
//! Spec: [`docs/api/gemdos/154-ssystem.md`]. Only the cookie subfunctions
//! (`S_GETCOOKIE`/`S_SETCOOKIE`/`S_DELCOOKIE`) are implemented; every other
//! mode returns `EINVFN` (NewTOS has no `ENOSYS`), so the surface stays
//! honest. Cookie storage: [`crate::cookie`].

use tos_types::error::{ERROR, EINVFN, ENSMEM, E_OK};
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
    match args.arg_u16(0) {
        S_GETCOOKIE => s_getcookie(args),
        S_SETCOOKIE => s_setcookie(args),
        S_DELCOOKIE => s_delcookie(args),
        _ => EINVFN,
    }
}

/// `S_GETCOOKIE` (8). `arg1` = tag, `arg2` = pointer to a LONG (or 0).
/// `tag == 0` → cookie count; small `tag` (high word 0) → the 1-based
/// Nth cookie's tag (enumeration); otherwise → the tag's value. With
/// `arg2 == 0` the result is returned in `x0`; otherwise it is written to
/// `*arg2` and `E_OK` is returned. Not-found / out-of-range → `ERROR`.
fn s_getcookie(args: &TrapArgs) -> Long {
    let tag = args.arg_i32(1) as u32;
    let ret = args.arg_u64(2);

    let value = if tag == 0 {
        Some(cookie::count())
    } else if tag & 0xFFFF_0000 == 0 {
        cookie::slot_tag(tag)
    } else {
        cookie::get(tag)
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
        None => ERROR,
    }
}

/// `S_SETCOOKIE` (9). `arg1` = tag, `arg2` = value. Add or update.
fn s_setcookie(args: &TrapArgs) -> Long {
    let tag = args.arg_i32(1) as u32;
    let value = args.arg_i32(2) as u32;
    if !cookie::valid_tag(tag) {
        return EINVFN;
    }
    if cookie::set(tag, value) {
        E_OK
    } else {
        ENSMEM // jar full
    }
}

/// `S_DELCOOKIE` (26). `arg1` = tag. Remove it.
fn s_delcookie(args: &TrapArgs) -> Long {
    let tag = args.arg_i32(1) as u32;
    if cookie::del(tag) {
        E_OK
    } else {
        ERROR
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
