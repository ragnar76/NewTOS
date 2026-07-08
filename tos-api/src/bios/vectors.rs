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

/// `Setexc` (0x05) — get (when `vector == -1`) or set the handler for
/// exception/hook vector `num`, returning the previous value (EmuTOS
/// `setexc`). Out-of-range `num` is ignored (returns 0) rather than
/// corrupting low memory the way m68k TOS does.
pub fn setexc(args: &TrapArgs) -> i64 {
    let num = args.arg_i16(0) as usize;
    let vector = args.arg_u64(1) as i64;
    if num >= NVEC {
        return 0;
    }
    let old = VECTORS[num].load(Ordering::Relaxed);
    if vector != -1 {
        VECTORS[num].store(vector, Ordering::Relaxed);
    }
    old
}

/// The handler currently installed for hook `num`, or 0 if none. NewTOS'
/// event paths call this to honour a program-installed `etv_*` hook once
/// the EL0-callback mechanism that invokes such handlers lands.
#[allow(dead_code)] // consumed as the etv_* hook dispatch is wired
pub fn vector(num: usize) -> i64 {
    if num < NVEC {
        VECTORS[num].load(Ordering::Relaxed)
    } else {
        0
    }
}
