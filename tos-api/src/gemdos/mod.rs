//! GEMDOS (Trap #1) dispatcher and function table.
//!
//! Spec: `docs/api/gemdos/_dispatcher.md`,
//! `docs/api/gemdos/console/*.md`.
//!
//! NewTOS encodes the function ID in `x8` (AAPCS64 reserved slot) and
//! takes up to seven argument registers (`x0..x6`); see
//! [`crate::trap::TrapArgs`]. The return value is a sign-extended
//! `Long` (`i32` in `x0`).
//!
//! PHASE-02 implements the twelve "leaf" console functions that do
//! not depend on Phase 06 (process management) for Ctrl-C handling.
//! The remaining three input-with-Ctrl-C functions (Cconin 0x01,
//! Cnecin 0x08, Cconrs 0x0A) are gated and return [`EINVFN`] until
//! Phase 06 lands.

use crate::host::host;
use crate::trap::TrapArgs;
use tos_types::error::{EINVFN, EUNDEV};
use tos_types::Long;

pub mod console;
pub mod memory;
pub mod time;
pub mod version;

/// Highest GEMDOS function ID dispatched by NewTOS. Matches EmuTOS
/// `MAX_FNCALL` (= `bdos/bdosmain.c`).
pub const MAX_FN: usize = 0x57;

/// Per-function entry. `fp` is the implementation; a `None` slot
/// means "unimplemented" and yields `EINVFN`.
type GemdosFn = fn(&TrapArgs) -> Long;

/// Function table indexed by function ID. Built at compile time; only
/// non-`None` slots dispatch.
const TABLE: [Option<GemdosFn>; MAX_FN + 1] = build_table();

const fn build_table() -> [Option<GemdosFn>; MAX_FN + 1] {
    let mut t: [Option<GemdosFn>; MAX_FN + 1] = [None; MAX_FN + 1];
    // --- Console group (PHASE-02 D3; input side unblocked PHASE-06) -
    t[0x01] = Some(console::cconin);
    t[0x02] = Some(console::cconout);
    t[0x03] = Some(console::cauxin);
    t[0x04] = Some(console::cauxout);
    t[0x05] = Some(console::cprnout);
    t[0x06] = Some(console::crawio);
    t[0x07] = Some(console::crawcin);
    t[0x08] = Some(console::cnecin);
    t[0x09] = Some(console::cconws);
    t[0x0A] = Some(console::cconrs);
    t[0x0B] = Some(console::cconis);
    t[0x10] = Some(console::cconos);
    t[0x11] = Some(console::cprnos);
    t[0x12] = Some(console::cauxis);
    t[0x13] = Some(console::cauxos);
    // 0x0A Cconrs (line editor) — wired separately; Ctrl-C in the
    // blocking input functions records a deferred terminate consumed
    // by the kernel SVC handler.

    // --- Memory group (PHASE-04 D7..D11) --------------------------
    t[0x14] = Some(memory::maddalt);
    t[0x44] = Some(memory::mxalloc);
    t[0x48] = Some(memory::malloc);
    t[0x49] = Some(memory::mfree);
    t[0x4A] = Some(memory::mshrink);

    // --- Time / version (PHASE-07 D2 + D5) ------------------------
    t[0x2A] = Some(time::tgetdate);
    t[0x2B] = Some(time::tsetdate);
    t[0x2C] = Some(time::tgettime);
    t[0x2D] = Some(time::tsettime);
    t[0x30] = Some(version::sversion);
    t
}

/// Dispatch a GEMDOS trap. Called from the kernel's synchronous-EL0
/// exception handler with the captured register state.
///
/// Returns the value to load into `x0` on `eret`.
pub fn dispatch(args: &TrapArgs) -> Long {
    if host().is_none() {
        // Misconfiguration: panic-shaped but encoded as a TOS error so
        // the trap path remains reentrant.
        return EUNDEV;
    }
    let id = args.func_id() as usize;
    if id > MAX_FN {
        return EINVFN;
    }
    match TABLE[id] {
        Some(f) => f(args),
        None => EINVFN,
    }
}
