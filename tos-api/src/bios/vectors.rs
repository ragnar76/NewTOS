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
static VECTORS: [AtomicI64; NVEC] = [const { AtomicI64::new(0) }; NVEC];

/// Canonical TOS hook vector numbers (m68k address / 4). (The `05-setexc.md`
/// edge-case note lists `etv_critic` at 0x108 in error; the system-variable
/// layout puts `etv_timer/critic/term` at `$0400/$0404/$0408`.)
pub const ETV_TIMER: usize = 0x100; // $0400 — 20 ms system tick
pub const ETV_CRITIC: usize = 0x101; // $0404 — critical-error handler
pub const ETV_TERM: usize = 0x102; // $0408 — process-terminate hook

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
