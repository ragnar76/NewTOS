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

use super::pd;
use super::Aespb;
use crate::host::host;

/// `evnt_keybd` (20) — block until a key is pressed; return it in
/// `RET_CODE` as `scancode << 8 | ascii`.
///
/// The console input source is the serial/IKBD byte ring, which carries
/// ASCII only, so the scancode high byte is 0 — the same convention as
/// the GEMDOS console input ([`docs/api/gemdos/console`]). When a real
/// IKBD scancode path lands, the high byte fills in here.
pub fn evnt_keybd(pb: &Aespb) {
    let key = match host() {
        Some(h) => loop {
            if let Some(b) = h.console_try_read() {
                break b as i16;
            }
            h.pump_input();
        },
        None => 0,
    };
    pb.set_ret(key);
}

/// `evnt_mesag` (23) — block until a 16-byte message is in the calling
/// application's pipe, copy it to `addr_in[0]` (`ME_PBUFF`), set the
/// `AP_MESAG` flag, and return 1.
///
/// Bring-up note: with a single AES application and no window/menu
/// managers posting messages yet, the only writer is `appl_write`; the
/// wait returns as soon as a message is pending. A lone application
/// waiting on a message that no one will post blocks by design — it
/// asked to wait. Multi-application posting lands with the window/menu
/// managers (later D-deliverables).
pub fn evnt_mesag(pb: &Aespb) {
    let dest = pb.addrin(0);
    let pid = super::appl::current_pid();
    let Some(h) = host() else {
        pb.set_ret(1);
        return;
    };
    loop {
        // SAFETY: single trap path; `by_pid` hands out the live slot.
        let Some(src) = (unsafe { pd::by_pid(pid) }) else {
            // No descriptor (caller never ran appl_init): nothing to
            // wait on; report the message event and bail.
            pb.set_ret(1);
            return;
        };
        if src.msg_pending {
            if dest != 0 {
                // SAFETY: `ME_PBUFF` points at >= 16 bytes of storage.
                unsafe {
                    let p = dest as *mut i16;
                    for i in 0..pd::MSG_WORDS {
                        p.add(i).write(src.msg[i]);
                    }
                }
            }
            src.msg_pending = false;
            src.flags |= pd::AP_MESAG;
            pb.set_ret(1);
            return;
        }
        h.pump_input();
    }
}

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
