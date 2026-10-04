//! `Ssystem` — GEMDOS 0x154 (FreeMiNT extension).
//!
//! Spec: [`docs/api/gemdos/154-ssystem.md`]. Of FreeMiNT's modes only the
//! cookie subfunctions (`S_GETCOOKIE`/`S_SETCOOKIE`/`S_DELCOOKIE`) are
//! implemented; every other FreeMiNT mode returns `EINVFN` (NewTOS has no
//! `ENOSYS`), so the surface stays honest. Cookie storage: [`crate::cookie`].
//!
//! On top of those sit NewTOS's **own** subfunctions from 2000 upwards
//! ([`S_NEWTOS_CPUCLOCK`], [`S_NEWTOS_CPUTEMP`]) — machine facts an EL0
//! program cannot otherwise reach, because the hardware that knows them (the
//! BCM2711 mailbox) is kernel-only — and [`S_NEWTOS_KBDVEC`] (2002), the
//! kernel entry of the per-process default `kbdvec` stub (KEYS02,
//! `docs/api/xbios/22-kbdvbase.md` B3/D6). 2000 is free: FreeMiNT's own header
//! reserves 0–29, 100, `0x54f8`, 900–999 and 1000–**1999**
//! (`references/freemint/sys/mint/ssystem.h`, the two `reserved` comments),
//! and the highest number its dispatcher answers is the build-gated test mode
//! 3000 (`sys/ssystem.c:631`). The gap 2000…2999 belongs to nobody.

use tos_types::error::{EINVAL, EINVFN, ENSMEM, ERROR, E_OK};
use tos_types::Long;

use crate::cookie;
use crate::host;
use crate::trap::TrapArgs;

/// `Ssystem`'s GEMDOS function id. It is above `MAX_FN`, so the GEMDOS
/// dispatcher special-cases it before the static table lookup.
pub const SSYSTEM: usize = 0x154;

// Cookie subfunction modes (FreeMiNT `sys/mint/ssystem.h`).
const S_GETCOOKIE: u16 = 8;
const S_SETCOOKIE: u16 = 9;
const S_DELCOOKIE: u16 = 26;

/// NewTOS-own subfunction: the CPU core clock in **Hz**.
pub const S_NEWTOS_CPUCLOCK: u16 = 2000;
/// NewTOS-own subfunction: the CPU/SoC temperature in **thousandths of a
/// degree Celsius**.
pub const S_NEWTOS_CPUTEMP: u16 = 2001;
/// NewTOS-own subfunction: run the default `kbdvec` (EmuTOS `kbd_int`) for
/// one IKBD key byte — what the `kbdvec_default` stub in every process's page
/// 0 calls (KEYS02). Defined next to the stub in [`crate::xbios::kbdvecs`].
pub const S_NEWTOS_KBDVEC: u16 = crate::xbios::kbdvecs::S_NEWTOS_KBDVEC;

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
        S_NEWTOS_CPUCLOCK => {
            newtos_scalar(args, || host::host()?.cpu_clock_hz().map(|hz| hz as Long))
        }
        S_NEWTOS_CPUTEMP => {
            newtos_scalar(args, || host::host()?.cpu_temperature_mdeg())
        }
        S_NEWTOS_KBDVEC => s_newtos_kbdvec(args),
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

/// The shared body of the NewTOS-own scalar subfunctions (2000 upwards).
///
/// Argument convention — FreeMiNT's own, where `arg1` is the caller's buffer
/// and `arg2` its length (`S_GETBOOTLOG` 29, `S_KNAME` 900,
/// `sys/mint/ssystem.h`), narrowed to a single LONG:
///
/// * `arg1` — a user pointer to a LONG that receives the value, or **0** to
///   have the value returned in the call's own LONG. The pointer form is the
///   unambiguous one: it carries all 32 bits, while a returned LONG cannot
///   tell a value of `0xFFFF_FFFF` from `ERROR`. (For a Pi 4 neither reading
///   can reach that far — its ARM clock tops out at 2.0 GHz and its die
///   temperature at some tens of degrees — but the contract, not the current
///   board, decides which form a caller should use.)
/// * `arg2` — reserved for a future selector (a second clock, a second
///   sensor); it must be **0** today. Anything else is `EINVAL` rather than
///   silently ignored, so a program written against a later, wider version
///   of this call is told it is talking to an older kernel instead of
///   receiving the wrong quantity.
///
/// `read` answers `None` when this board cannot measure the value — then the
/// call is `ERROR` and nothing is written through `arg1`. That is the whole
/// point of routing through [`crate::host::Host`]: a board with no sensor
/// says so.
fn newtos_scalar(args: &TrapArgs, read: impl FnOnce() -> Option<Long>) -> Long {
    if args.arg_u64(2) != 0 {
        return EINVAL;
    }
    let Some(value) = read() else {
        return ERROR;
    };
    let out = args.arg_u64(1);
    if out == 0 {
        return value;
    }
    // SAFETY: `arg1` is a user pointer; the unprivileged store
    // permission-checks it as EL0 and faults the caller (not the kernel) on a
    // bad pointer — the same rule as `s_getcookie`'s `arg2`.
    unsafe { write_user_long(out as *mut u8, value as u32) };
    E_OK
}

/// `S_NEWTOS_KBDVEC` (2002): `arg1` = the IKBD key byte (bit 7 = break, as
/// `kbdvec` receives it in `x0`), `arg2` reserved, must be 0. Runs the default
/// `kbdvec` — EmuTOS `_kbdvec` → `kbd_int(scancode)` (`bios/aciavecs.S:
/// 372-376`, `bios/ikbd.c:725-910`) — through the host's key decoder:
/// modifier state, the key word into the console queue, the key repeat.
///
/// * `E_OK` — the byte was decoded;
/// * `EINVAL` — `arg1` is not a byte (`> 0xFF`) or `arg2 != 0`; nothing ran;
/// * `ERROR` — this host has no keyboard decoder (a hostless build).
///
/// Any process may call it; the effect is exactly that of calling the default
/// `kbdvec` it can already read from its own table, as on TOS.
fn s_newtos_kbdvec(args: &TrapArgs) -> Long {
    let byte = args.arg_u64(1);
    if byte > 0xFF || args.arg_u64(2) != 0 {
        return EINVAL;
    }
    match host::host() {
        Some(h) if h.kbd_int(byte as u8) => E_OK,
        _ => ERROR,
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
