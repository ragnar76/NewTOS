//! `desk::dosbind` — the desktop's **GEMDOS trap binding**.
//!
//! The desktop is a client of the operating system, not a part of it: every
//! system service it uses it must reach the way an application reaches it —
//! through the trap. [`super`]'s *aesbind* block does that for the AES
//! (`svc #2`, selector 200, EmuTOS `desk/gembind.c`); this module is its
//! counterpart for GEMDOS (`svc #1`), i.e. for EmuTOS's `include/gemdos.h` —
//! the `dos_*` inline wrappers the desktop actually calls
//! (`desk/deskdir.c:402,431,454,463,555-558`, `desk/deskinf.c`,
//! `desk/deskfpd.c`, `desk/deskapp.c:714`) — layered over
//! `include/bdosbind.h`'s `trap1()` (`util/miscasm.S`).
//!
//! Naming follows EmuTOS's `gemdos.h`: [`dos_open`], [`dos_read`],
//! [`dos_sfirst`], … . [`dos_getcookie`] has no EmuTOS counterpart in
//! `gemdos.h`: it is the NewTOS route to the one piece of system state EmuTOS's
//! desktop reads with an XBIOS call, the `_IDT` cookie — see its own comment
//! for why that is a GEMDOS call here and not `Supexec`.
//!
//! The desktop uses **no BIOS and no XBIOS function**, so there is neither an
//! `svc #13` nor an `svc #14` binding here. EmuTOS's desktop includes
//! `biosbind.h`, but the two calls it takes from it are `Drvmap()` — which the
//! desktop itself never uses, because it reads the drive map as
//! `dos_sdrv(dos_gdrv())` (`deskapp.c:714`, `deskins.c:297`,
//! `desksupp.c:1575,1819`) — and `Dosound()` for the About-box Easter egg
//! (`CONF_WITH_EASTER_EGG`), which NewTOS does not have. From `xbiosbind.h` it
//! takes `deskrez.c`'s resolution change, which NewTOS does not have either
//! (the screen mode is fixed by the firmware), and `Supexec` in `do_wredraw`
//! (`desksupp.c:219`) — that one it *does* need, and [`dos_getcookie`] is where
//! it goes instead.
//!
//! ## ABI
//!
//! `docs/ARCHITECTURE.md`, "Native AArch64 callers": function ID in `x8`
//! (low 16 bits), arguments in `x0..x7` **in their TOS order**, result a
//! sign-extended `Long` in `x0`. The argument order is therefore exactly the
//! `trap1(...)` argument list of `bdosbind.h:22-53` — note the two calls that
//! carry a dead leading word, `Frename(0, old, new)` (`:53`) and
//! `Mshrink`-style `Fdatime(timeptr, handle, wflag)` (`:54`).
//!
//! ## What the trap changes, and what it does not
//!
//! Before this module the desktop called the `tos_api::bdos::fs` leaf
//! functions directly with a hand-built `TrapArgs`. Two properties of the real
//! trap path had to be established before the rewiring, because they are the
//! only two places where "same arguments" could still mean "different
//! behaviour":
//!
//! * **The DTA.** `Fsetdta`/`Fsfirst`/`Fsnext` keep the current DTA in the
//!   *running process's* basepage (`p_xdta`, EmuTOS `bdos/fsmain.c:396-405`).
//!   The desktop runs in the kernel's boot context, where
//!   `tos_api::bdos::process::current()` is the null basepage, and
//!   `tos-api/src/bdos/fs/find.rs:52-105` routes exactly that caller to its
//!   `KERNEL_DTA` cell instead. That is the same cell — and therefore the same
//!   44-byte buffer on the desktop's stack — the direct call used, because the
//!   direct call entered `find::fsetdta` at the identical point. No change.
//! * **Pointer arguments.** A trap from EL0 hands the kernel a *user* VA;
//!   the desktop hands it a kernel high-half VA. The GEMDOS file surface reads
//!   its pointers raw (`tos-api/src/bdos/fs/mod.rs:23-33` `cstr`,
//!   `bdos/fs/file.rs:297,373` `ubuf as *const u8`) and performs **no**
//!   address translation — `usermem::resolve_kva` is used only by the AES's
//!   string reader and by XBIOS `Setscreen`, neither of which is on this path.
//!   A kernel VA therefore arrives and is dereferenced unchanged. No buffer
//!   has to move.

use tos_types::Long;

// ── GEMDOS function ids (`bdosbind.h:22-54`) ────────────────────────────────
const DSETDRV: u16 = 0x0E;
const DGETDRV: u16 = 0x19;
const FSETDTA: u16 = 0x1A;
const DCREATE: u16 = 0x39;
const DDELETE: u16 = 0x3A;
const FCREATE: u16 = 0x3C;
const FOPEN: u16 = 0x3D;
const FCLOSE: u16 = 0x3E;
const FREAD: u16 = 0x3F;
const FWRITE: u16 = 0x40;
const FDELETE: u16 = 0x41;
const FSFIRST: u16 = 0x4E;
const FSNEXT: u16 = 0x4F;
const FRENAME: u16 = 0x56;
const FDATIME: u16 = 0x57;
/// `Mxalloc` (`bdosbind.h:56`) — used only through [`dos_avail_stram`] /
/// [`dos_avail_altram`], EmuTOS's `util/gemdos.c:101-112`.
const MXALLOC: u16 = 0x44;
/// `Ssystem` (GEMDOS 0x154, a FreeMiNT extension NewTOS implements —
/// `docs/api/gemdos/154-ssystem.md`). Used only by [`dos_getcookie`].
const SSYSTEM: u16 = 0x154;

/// `S_GETCOOKIE` (FreeMiNT `sys/mint/ssystem.h`) — `Ssystem` subfunction 8.
const S_GETCOOKIE: i16 = 8;

// ── `Mxalloc` modes (`include/bdosdefs.h:18-19`) ────────────────────────────
/// `MX_STRAM` — allocate/measure ST-RAM only.
const MX_STRAM: i16 = 0;
/// `MX_TTRAM` — allocate/measure alternate (TT) RAM only.
const MX_TTRAM: i16 = 1;

/// Issue the GEMDOS trap (`svc #1`): function id in `x8`, arguments in
/// `x0..x2`. EmuTOS's `trap1()` (`util/miscasm.S`).
///
/// `x3..x6` are zeroed rather than left with whatever the compiler had in
/// them: the GEMDOS dispatcher's tracer records the *frame*
/// (`tos-api/src/gemdos/trace.rs:743` logs `x0..x6`), and a trap frame
/// carrying leftovers would put noise into it. No GEMDOS function the desktop
/// calls takes more than three arguments (`bdosbind.h:22-54`).
///
/// `nostack`: an exception taken at EL1 with `SPSel = 0` runs the handler on
/// `SP_EL1` (the emergency stack, `link-rpi4.ld:100-102`), so the caller's
/// stack is untouched — the same option the AES binding's `gem_trap` uses.
/// Memory is deliberately *not* declared clean: `Fread` and `Fsfirst` write
/// through the pointers they are given.
#[inline]
fn dos_trap(func: u16, a0: u64, a1: u64, a2: u64) -> Long {
    let ret: i64;
    // SAFETY: GEMDOS trap. The handler restores every register except `x0`,
    // which it overwrites with the sign-extended `Long` result.
    unsafe {
        core::arch::asm!(
            "svc #1",
            inlateout("x0") a0 => ret,
            in("x1") a1,
            in("x2") a2,
            in("x3") 0u64,
            in("x4") 0u64,
            in("x5") 0u64,
            in("x6") 0u64,
            in("x8") func as u64,
            options(nostack),
        );
    }
    ret as Long
}

/// Widen a signed TOS argument into its register slot.
///
/// The m68k ABI pushes a WORD as a WORD and a LONG as a LONG; on AArch64 the
/// whole 64-bit register carries it, and the dispatcher reads back the width
/// the function declares (`TrapArgs::arg_i16`/`arg_i32`). The *upper* bits are
/// therefore unobservable to the callee — but they are not unobservable to the
/// dispatcher's tracer, which logs the raw register. Sign-extending keeps the
/// frame bit-identical to the hand-built `TrapArgs` this binding replaced.
#[inline]
const fn sx(v: i64) -> u64 {
    v as u64
}

/// A NUL-terminated GEMDOS path buffer built from `s`.
///
/// 130 bytes = EmuTOS `MAXPATHLEN` (`sysconf.h:51-55`, `LEN_ZPATH` 114 +
/// `LEN_ZFNAME` 13 + 1 + rounding); a longer `s` is truncated, which GEMDOS
/// then rejects with `EPTHNF` exactly as it would reject the untruncated name.
fn cpath(s: &str) -> [u8; 130] {
    let mut b = [0u8; 130];
    let src = s.as_bytes();
    let n = src.len().min(b.len() - 1);
    b[..n].copy_from_slice(&src[..n]);
    b
}

// ── GEMDOS bindings (EmuTOS `include/gemdos.h`) ─────────────────────────────

/// `dos_sdta` — `Fsetdta(buf)` (`gemdos.h:50-53`). Points the current search
/// at `dta`, a writable 44-byte buffer.
pub(super) fn dos_sdta(dta: *mut u8) {
    dos_trap(FSETDTA, dta as u64, 0, 0);
}

/// `dos_sfirst` — `Fsfirst(pspec, attr)` (`gemdos.h:60-63`). Starts a search;
/// `E_OK` on a match, `EFILNF`/`ENMFIL`/`EPTHNF` otherwise.
pub(super) fn dos_sfirst(spec: &str, attr: u8) -> Long {
    let p = cpath(spec);
    dos_trap(FSFIRST, p.as_ptr() as u64, attr as u64, 0)
}

/// `dos_snext` — `Fsnext()` (`gemdos.h:65-68`). Continues the search whose
/// state lives in the current DTA.
pub(super) fn dos_snext() -> Long {
    dos_trap(FSNEXT, 0, 0, 0)
}

/// `dos_open` — `Fopen(pname, access)` (`gemdos.h:70-73`). Returns a handle
/// (≥ 0) or a negative GEMDOS error.
pub(super) fn dos_open(path: &str, access: u16) -> Long {
    let p = cpath(path);
    dos_trap(FOPEN, p.as_ptr() as u64, access as u64, 0)
}

/// `dos_create` — `Fcreate(name, attr)` (`gemdos.h:110-113`). Creates or
/// truncates; returns a handle or a negative GEMDOS error.
pub(super) fn dos_create(path: &str, attr: u16) -> Long {
    let p = cpath(path);
    dos_trap(FCREATE, p.as_ptr() as u64, attr as u64, 0)
}

/// `dos_close` — `Fclose(handle)` (`gemdos.h:75-78`).
pub(super) fn dos_close(handle: i16) -> Long {
    dos_trap(FCLOSE, sx(handle as i64), 0, 0)
}

/// `dos_read` — `Fread(handle, cnt, pbuffer)` (`gemdos.h:80-83`). Returns the
/// number of bytes read, or a negative GEMDOS error.
pub(super) fn dos_read(handle: i16, cnt: i32, buf: *mut u8) -> Long {
    dos_trap(FREAD, sx(handle as i64), sx(cnt as i64), buf as u64)
}

/// `dos_write` — `Fwrite(handle, cnt, pbuffer)` (`gemdos.h:85-88`). Returns
/// the number of bytes written, or a negative GEMDOS error.
pub(super) fn dos_write(handle: i16, cnt: i32, buf: *const u8) -> Long {
    dos_trap(FWRITE, sx(handle as i64), sx(cnt as i64), buf as u64)
}

/// `dos_delete` — `Fdelete(name)` (`gemdos.h:125-128`).
pub(super) fn dos_delete(path: &str) -> Long {
    let p = cpath(path);
    dos_trap(FDELETE, p.as_ptr() as u64, 0, 0)
}

/// `dos_rename` — `Frename(p1, p2)` (`gemdos.h:130-133`), which expands to
/// `trap1(0x56, 0, oldname, newname)` (`bdosbind.h:53`): the leading word is
/// the historical unused argument, so the names sit in `x1`/`x2`.
pub(super) fn dos_rename(old: &str, new: &str) -> Long {
    let o = cpath(old);
    let n = cpath(new);
    dos_trap(FRENAME, 0, o.as_ptr() as u64, n.as_ptr() as u64)
}

/// `dos_mkdir` — `Dcreate(path)` (`gemdos.h:115-118`).
pub(super) fn dos_mkdir(path: &str) -> Long {
    let p = cpath(path);
    dos_trap(DCREATE, p.as_ptr() as u64, 0, 0)
}

/// `dos_rmdir` — `Ddelete(path)` (`gemdos.h:135-138`).
pub(super) fn dos_rmdir(path: &str) -> Long {
    let p = cpath(path);
    dos_trap(DDELETE, p.as_ptr() as u64, 0, 0)
}

/// `dos_setdt(h, time, date)` — `Fdatime(buf, h, TRUE)` over a two-word
/// buffer holding `time` then `date` (EmuTOS `util/gemdos.c:49-56`, declared
/// `gemdos.h:17`). Stamps an open handle with the source entry's packed FAT
/// time/date, which is how `deskdir.c:457` preserves a copied file's
/// timestamp.
///
/// EmuTOS's `UWORD buf[2]` is big-endian m68k; NewTOS writes the two words
/// little-endian, the layout `tos-api/src/bdos/fs/file.rs:838-869` reads back.
/// The buffer's byte order is not part of the TOS contract — the *field* order
/// (time, then date) is.
pub(super) fn dos_setdt(handle: i16, time: u16, date: u16) -> Long {
    let t = time.to_le_bytes();
    let d = date.to_le_bytes();
    let buf = [t[0], t[1], d[0], d[1]];
    dos_trap(FDATIME, buf.as_ptr() as u64, sx(handle as i64), 1)
}

/// `dos_gdrv` — `Dgetdrv()` (`gemdos.h:45-48`, `bdosbind.h:30`). The calling
/// process's current drive, 0-based (`0` = `A:`).
pub(super) fn dos_gdrv() -> i16 {
    dos_trap(DGETDRV, 0, 0, 0) as i16
}

/// `dos_sdrv` — `Dsetdrv(newdrv)` (`gemdos.h:105-108`, `bdosbind.h:26`). Sets
/// the current drive (0-based; an out-of-range index is silently ignored) and
/// **returns the drive bitmap**, bit `n` = drive `n` is present
/// (`docs/api/gemdos/drive/0e-dsetdrv.md`). The bitmap comes back regardless of
/// whether the drive actually changed, which is what makes the idiom below
/// work.
///
/// The desktop never calls this to *change* drives: it calls
/// `dos_sdrv(dos_gdrv())` — set the current drive to the one it already is —
/// purely to harvest the return value. That is EmuTOS's own way of reading the
/// drive map (`deskapp.c:714` `drivemask = dos_sdrv(dos_gdrv());`,
/// `deskins.c:297`, `desksupp.c:1575,1819` `valid_drive`), and it is a GEMDOS
/// call, not the BIOS `Drvmap()` that yields the same bits one layer lower:
/// going to the BIOS would step past the GEMDOS drive layer that owns the
/// current-drive state.
pub(super) fn dos_sdrv(newdrv: i16) -> Long {
    dos_trap(DSETDRV, sx(newdrv as i64), 0, 0)
}

/// `dos_avail_stram` — `Mxalloc(-1L, MX_STRAM)` (`util/gemdos.c:100-104`): the
/// **largest free block** in ST-RAM, in bytes. `amount == -1` is `Mxalloc`'s
/// probe form — it measures and allocates nothing
/// (`docs/api/gemdos/memory/44-mxalloc.md`).
pub(super) fn dos_avail_stram() -> Long {
    dos_trap(MXALLOC, sx(-1), sx(MX_STRAM as i64), 0)
}

/// `dos_avail_altram` — `Mxalloc(-1L, MX_TTRAM)` (`util/gemdos.c:106-113`,
/// `CONF_WITH_ALT_RAM`, which defaults to 1, `include/config.h:882-884`): the
/// largest free block in alternate RAM, `0` when the board has none.
///
/// EmuTOS gates the *display* of this figure on `has_alt_ram`
/// (`deskinf.c:1120-1123`); NewTOS gates it on the same thing one step later —
/// a zero return means the TT-RAM pool is empty or absent
/// (`tos-api/src/bdos/mem/traps.rs:274-281`, `pmdalt` null ⇒ `0`).
pub(super) fn dos_avail_altram() -> Long {
    dos_trap(MXALLOC, sx(-1), sx(MX_TTRAM as i64), 0)
}

/// Read one cookie **by value** — `Ssystem(S_GETCOOKIE, tag, 0)`
/// (`docs/api/gemdos/154-ssystem.md`, `tos-api/src/gemdos/ssystem.rs:42-77`).
/// [`None`] if the tag is not in the jar (the call returns `ERROR`).
///
/// ## Why this and not `Supexec`
///
/// EmuTOS's desktop reads `_IDT` with `Supexec((LONG)get_idt_cookie)`
/// (`desksupp.c:219`) — not because the *cookie* needs privilege, but because
/// on m68k the jar is reached through the pointer at `$5A0`, in supervisor-only
/// low memory. That is an m68k mechanism, and `docs/COMPATIBILITY.md` puts
/// mechanisms on the free side of the line.
///
/// On NewTOS there is nothing to elevate for and nothing to elevate *with*:
/// XBIOS `Supexec` (0x26) is an open ADR (`docs/api/xbios/26-supexec.md`,
/// "NewTOS status: decision deferred") and the cookie-jar spec already names
/// this call as its replacement — "There is no `Supexec` in the kernel … the
/// low-memory area an app wants is already in its own address space"
/// (`docs/api/bios/_cookie-jar.md` §Status 2026-09-14). `Ssystem` is a GEMDOS
/// function, so the desktop reaches it through the same `svc #1` it uses for
/// every other system service (DESK10) — the third argument is 0, so the value
/// comes back in `x0` and no pointer crosses the trap.
///
/// **The value form cannot express a cookie whose value has bit 31 set** — it
/// would be indistinguishable from the negative error return. That is
/// `Ssystem`'s own shape, and the alternative (`arg2` = a pointer to write into)
/// is closed to the desktop: `s_getcookie` stores through an *unprivileged*
/// `sttr` (`ssystem.rs:110-121`), which faults on a kernel address. The one
/// cookie read here, `_IDT`, is a 16-bit word (`0x112F`), so the limitation is
/// nominal; a future caller reading a pointer-valued cookie must not use this.
pub(super) fn dos_getcookie(tag: u32) -> Option<u32> {
    let rc = dos_trap(SSYSTEM, sx(S_GETCOOKIE as i64), sx(tag as i32 as i64), 0);
    (rc >= 0).then_some(rc as u32)
}
