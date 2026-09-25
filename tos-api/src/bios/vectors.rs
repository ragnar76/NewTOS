//! `Setexc` (BIOS 0x05) and the software m68k "vector shadow".
//!
//! Spec: [`docs/api/bios/05-setexc.md`]. On m68k, `Setexc(num, vec)` writes
//! the exception/hook vector at address `4 * num`; that flat vector page
//! does not exist on AArch64. NewTOS instead keeps a **software shadow**
//! of the vector table: `Setexc` gets/sets an entry (returning the old
//! value — the complete `Setexc` contract), and NewTOS' own code paths
//! call the logical TOS hooks (`etv_term`/`etv_critic`/`etv_timer`) via
//! [`vector`] when the matching event fires.
//!
//! Native handler addresses are 64-bit, so the shadow stores `i64`. The
//! m68k CPU vectors (`0..0x3F`) are shadowed for a future m68k userland but
//! are not dispatched by the native AArch64 exception handlers.

use crate::trap::TrapArgs;
use core::sync::atomic::{AtomicI64, Ordering};

/// Shadowed vectors: the 1024-byte m68k vector page (256 LONGs) plus the
/// system hook vectors just above it (`etv_*` at `$0400`, GEM hooks to
/// `$047F`) — `0x120` entries total.
const NVEC: usize = 0x120;

// Canonical TOS hook vector numbers (m68k address / 4). (The `05-setexc.md`
// edge-case note lists `etv_critic` at 0x108 in error; the system-variable
// layout puts `etv_timer/critic/term` at `$0400/$0404/$0408`.)

/// `etv_timer` hook — 20 ms system tick (m68k `$0400`).
pub const ETV_TIMER: usize = 0x100;
/// `etv_critic` hook — critical-error handler (m68k `$0404`).
pub const ETV_CRITIC: usize = 0x101;
/// `etv_term` hook — process-terminate hook (m68k `$0408`).
pub const ETV_TERM: usize = 0x102;

/// Non-null boot default for the no-op TOS hooks. EmuTOS points **both**
/// `etv_timer` and `etv_term` at the same `just_rts` stub during
/// `bios_init` (`bios/bios.c:360,362`) — never NULL — so the query idiom
/// `old = Setexc(num, -1)` always yields a valid address to chain to
/// (`05-setexc.md` edge case #5). NewTOS shadows both with this one shared
/// token. The shadow is **not** dispatched — NewTOS invokes the `etv_*`
/// hooks by name (`05-setexc.md` "Gaps") — so the value is a stable,
/// opaque sentinel, not a live code address; the high word carries the
/// m68k `RTS` opcode (`0x4E75`) purely as a self-documenting marker, and
/// the whole value sits well outside any real kernel VA or m68k vector
/// address so it can never be confused with an installed handler.
pub const ETV_DEFAULT_JUST_RTS: i64 = 0x4E75_0000_0000_0000;

/// Non-null boot default for the critical-error hook. EmuTOS uses
/// `default_etv_critic`, a real passthrough that returns the error code
/// (`bios/vectors.S:823`), distinct from `just_rts`. NewTOS shadows it
/// with this distinct opaque sentinel (high word = ASCII `"Crit"`) so a
/// save/restore chain can tell the critical-error default apart from the
/// no-op hooks. Same "not dispatched, stable token" caveat as
/// [`ETV_DEFAULT_JUST_RTS`].
pub const ETV_DEFAULT_CRITIC: i64 = 0x4372_6974_0000_0000;

static VECTORS: [AtomicI64; NVEC] = {
    let mut v = [const { AtomicI64::new(0) }; NVEC];
    // Seed the etv_* hook slots with their non-null boot defaults, exactly
    // as EmuTOS' `bios_init` does before GEMDOS/AES run (F3). All other
    // slots stay 0 (a CPU-exception vector nobody installed).
    v[ETV_TIMER] = AtomicI64::new(ETV_DEFAULT_JUST_RTS);
    v[ETV_CRITIC] = AtomicI64::new(ETV_DEFAULT_CRITIC);
    v[ETV_TERM] = AtomicI64::new(ETV_DEFAULT_JUST_RTS);
    v
};

/// Upper bound (exclusive) on the `Setexc` raw sysvar peek/poke: vectors
/// whose m68k address `4 * num` lands in the reserved low-memory
/// system-variable region `[0x000, 0x800)` (i.e. `num < 0x200`). Between
/// [`NVEC`] and this, `Setexc` reads/writes the calling process's EL0-VA
/// `4 * num` directly, EmuTOS-faithfully — the `Setexc(0x5A0/4 == 0x168, -1)`
/// cookie-jar idiom (TeraDesk `library/utility/cookie.c:56`) lands here.
const SYSVAR_NUM_LIMIT: usize = 0x200;

/// `Setexc` (0x05) — get (when `vector == -1`) or set the handler for
/// exception/hook vector `num`, returning the previous value (EmuTOS
/// `setexc`, `references/emutos/bios/bios.c:1070-1080`).
///
/// Three regimes:
///  * `num < NVEC` — the software m68k **vector shadow** (get/set the entry,
///    return the old value); NewTOS dispatches the `etv_*` hooks by name.
///  * `NVEC <= num < SYSVAR_NUM_LIMIT` — the m68k address `4 * num` lands in
///    the reserved low-memory **sysvar region**: peek/poke the calling
///    process's EL0-VA `*(LONG*)(4 * num)` directly, exactly like EmuTOS'
///    unchecked `setexc`. The kernel runs this svc handler with the app's
///    `TTBR0` active, so an unprivileged word access reaches the app's low
///    memory (where [`crate::bdos::process::loader`]'s `populate_sysvars`
///    wrote `p_cookies`@0x5A0 etc.).
///  * `num >= SYSVAR_NUM_LIMIT` — returns 0. NewTOS deliberately does **not**
///    emulate the full m68k "poke any address" corruption above the sysvar
///    region.
pub fn setexc(args: &TrapArgs) -> i64 {
    let num = args.arg_i16(0) as usize;
    let vector = args.arg_u64(1) as i64;

    // Named/CPU exception-vector shadow.
    if num < NVEC {
        let old = VECTORS[num].load(Ordering::Relaxed);
        if vector != -1 {
            VECTORS[num].store(vector, Ordering::Relaxed);
        }
        return old;
    }

    // Reserved low-memory sysvar region: EmuTOS-faithful raw peek/poke of
    // the calling process's `*(LONG*)(4 * num)` via unprivileged word
    // access (checked as EL0, like `Cconws`'s `ldtrb`), bounded to the
    // sysvar region so it can never corrupt arbitrary memory.
    if num < SYSVAR_NUM_LIMIT {
        let addr = (num * 4) as *mut u8;
        // SAFETY: `4 * num < 0x800` is inside the per-process reserved low
        // region the loader maps and populates; the unprivileged access is
        // permission-checked as EL0 under the active app `TTBR0`.
        let old = unsafe { read_user_long(addr) } as i32 as i64;
        if vector != -1 {
            // SAFETY: as above — writable EL0 sysvar cell.
            unsafe { write_user_long(addr, vector as u32) };
        }
        return old;
    }

    0
}

/// Read a 32-bit `LONG` from a user (EL0) pointer via an unprivileged load
/// (`ldtr`), so the access is permission-checked as EL0 — same contract as
/// `Cconws`'s `ldtrb` (`crate::gemdos::console`).
///
/// # Safety
///
/// `p` must address a readable EL0 4-byte cell under the active `TTBR0`.
#[inline]
unsafe fn read_user_long(p: *const u8) -> u32 {
    let v: u32;
    // SAFETY: caller guarantees `p` readable; unprivileged word load.
    unsafe {
        core::arch::asm!(
            "ldtr {v:w}, [{p}]",
            p = in(reg) p,
            v = out(reg) v,
            options(nostack, preserves_flags, readonly),
        );
    }
    v
}

/// Write a 32-bit `LONG` to a user (EL0) pointer via an unprivileged store
/// (`sttr`).
///
/// # Safety
///
/// `p` must address a writable EL0 4-byte cell under the active `TTBR0`.
#[inline]
unsafe fn write_user_long(p: *mut u8, v: u32) {
    // SAFETY: caller guarantees `p` writable; unprivileged word store.
    unsafe {
        core::arch::asm!(
            "sttr {v:w}, [{p}]",
            p = in(reg) p,
            v = in(reg) v,
            options(nostack, preserves_flags),
        );
    }
}

/// The handler currently installed for hook `num`, or 0 if none. NewTOS'
/// event paths call this to honour a program-installed `etv_*` hook via the
/// EL0-upcall mechanism (cooperative-kernel step 2). Consumed by the Pterm
/// path for **etv_term** (`ETV_TERM`) — the kernel reads the installed
/// handler and, unless it is the non-callable boot sentinel
/// [`ETV_DEFAULT_JUST_RTS`], delivers it as an EL0 upcall before terminating
/// (BIOS finding H1). `etv_critic`/`etv_timer` follow in later steps.
pub fn vector(num: usize) -> i64 {
    if num < NVEC {
        VECTORS[num].load(Ordering::Relaxed)
    } else {
        0
    }
}

// ─── `mousexvec` — the extended-mouse scancode vector ──────────────────────
//
// EmuTOS reserves a BIOS-level vector for the IKBD scancodes that are *not*
// keys but extra mouse buttons and wheel detents: `_mousexvec: .ds.l 1
// // mouse-routine for additional buttons` (`bios/aciavecs.S:127`), declared
// `extern void (*mousexvec)(WORD scancode);` (`include/biosext.h:78`).
// `init_acia_vecs` seeds it with the `just_rts` no-op (`aciavecs.S:149-151`),
// `kbd_int` calls it for the seven extended-mouse scancodes instead of
// decoding them as keys (`bios/ikbd.c:748-760`), and the VDI installs its own
// handler at workstation open: `mousexvec = vdi_mousex_handler`
// (`vdi/vdi_mouse.c:612`).
//
// It is **not** part of the `Kbdvbase()` table: `_mousexvec` sits two LONGs
// *below* `_kbdvecs` (`aciavecs.S:127-131`), so no program can reach it
// through the documented positive offsets. It is a BIOS-internal hand-off
// from the keyboard decoder to the mouse driver, and that is all it is here.
//
// NewTOS' IKBD decoder lives in the kernel crate and the VDI in this one, so
// the cell has to live on the `tos-api` side of the dependency edge; the
// kernel's `ikbd::decode_key` calls [`mousexvec`], the VDI's
// `vdimouse_init` equivalent calls [`set_mousexvec`].

/// The installed extended-mouse handler, as a `fn(u8)` address. `0` is the
/// power-up "nobody installed one" state and is the exact behavioural
/// equivalent of EmuTOS' `just_rts` default (`aciavecs.S:149-151`).
static MOUSEXVEC: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

/// Install the extended-mouse handler — EmuTOS `vdimouse_init`'s
/// `mousexvec = vdi_mousex_handler` (`vdi/vdi_mouse.c:612`). Idempotent.
pub fn set_mousexvec(f: fn(u8)) {
    MOUSEXVEC.store(f as usize, Ordering::Relaxed);
}

/// Call the extended-mouse handler with `scancode` — EmuTOS `kbd_int`'s
/// `mousexvec(scancode)` (`bios/ikbd.c:758`). A no-op while no handler is
/// installed (EmuTOS' `just_rts`).
pub fn mousexvec(scancode: u8) {
    let p = MOUSEXVEC.load(Ordering::Relaxed);
    if p == 0 {
        return;
    }
    // Go through a data pointer rather than an integer so no int-to-fn
    // transmute lint fires (same shape as the kernel-built `G_USERDEF`
    // callout, `aes/object.rs:2741-2746`).
    let code = p as *const ();
    // SAFETY: the cell is written only by `set_mousexvec`, always with a live
    // `fn(u8)` of this binary; `0` (never installed) was rejected above.
    let f: fn(u8) = unsafe { core::mem::transmute(code) };
    f(scancode);
}
