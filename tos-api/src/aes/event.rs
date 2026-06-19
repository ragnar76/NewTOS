//! AES Event Manager (`evnt_*`, opcodes 20–26).
//!
//! Spec: [`docs/api/aes/event.md`]; the `int_in`/`int_out` slot layout is
//! the EmuTOS `include/aesdefs.h` mapping (e.g. `T_LOCOUNT = int_in[0]`,
//! `RET_CODE = int_out[0]`).
//!
//! **Bring-up model (single application, no AES scheduler):** EmuTOS'
//! `ev_block` parks the calling AES process and yields to others until an
//! event fires. NewTOS has one AES application for now, so each wait is a
//! cooperative poll-loop over the event sources — the same shape as the
//! kernel's blocking console read. Multitasking (`appl_yield` between
//! several AES processes) lands later; the API surface is unchanged.

use super::Aespb;
use crate::host::host;

/// `evnt_timer` (24) — block for `ms` milliseconds, then return.
///
/// `int_in[0]` = low WORD of the millisecond count, `int_in[1]` = high
/// WORD (`T_LOCOUNT`/`T_HICOUNT`). `RET_CODE` is 1 (the timer event).
pub fn evnt_timer(pb: &Aespb) {
    let lo = pb.intin(0) as u16 as u32;
    let hi = pb.intin(1) as u16 as u32;
    let ms = ((hi << 16) | lo) as u64;

    wait_ms(ms);
    pb.set_ret(1);
}

/// Block for `ms` milliseconds against the host's monotonic clock. A
/// zero count returns immediately (EmuTOS `ev_timer(0)` semantics). With
/// no host installed the clock stands still, so this returns at once.
fn wait_ms(ms: u64) {
    if ms == 0 {
        return;
    }
    if let Some(h) = host() {
        let start = h.monotonic_millis();
        // Saturating target; `monotonic_millis` is monotonic so a plain
        // comparison is safe (no wraparound within any realistic uptime).
        let deadline = start.saturating_add(ms);
        while h.monotonic_millis() < deadline {
            core::hint::spin_loop();
        }
    }
}
