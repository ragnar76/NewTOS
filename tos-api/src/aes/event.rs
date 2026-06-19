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

use core::sync::atomic::{AtomicI16, Ordering};

use super::pd;
use super::Aespb;
use crate::host::host;

/// Fill the shared event return slots from the current input state:
/// `EV_MX`/`EV_MY` (mouse x/y), `EV_MB` (button bitmask), `EV_KS`
/// (keyboard shift state) — the `ev_rets` block at `int_out[1..4]`.
fn fill_rets(pb: &Aespb) {
    let (mx, my) = crate::vdi::mouse_position();
    pb.set_intout(1, mx); // EV_MX
    pb.set_intout(2, my); // EV_MY
    pb.set_intout(3, crate::vdi::mouse_buttons() as i16); // EV_MB
    pb.set_intout(4, shift_state()); // EV_KS
}

/// Current keyboard shift-state bitmask (`Kbshift` encoding), or 0 with
/// no host.
fn shift_state() -> i16 {
    host().map(|h| h.keyboard_shift_state()).unwrap_or(0) as i16
}

/// `evnt_button` (21) — block until the mouse buttons match the
/// requested state, then return the click count and fill the mouse /
/// keyboard return slots.
///
/// `int_in[0]` = clicks to wait for (`B_CLICKS`), `int_in[1]` = button
/// mask (`B_MASK`), `int_in[2]` = desired masked state (`B_STATE`).
/// `RET_CODE` = clicks; `int_out[1..4]` = x, y, buttons, shift.
///
/// Bring-up: waits for the masked button bits to reach the requested
/// state and reports the requested click count. Multi-click
/// (double-click) timing — the EmuTOS `gl_dclick`/`gl_bclick` machinery
/// — is deferred; single clicks are exact.
pub fn evnt_button(pb: &Aespb) {
    let clicks = pb.intin(0);
    let mask = pb.intin(1) as u8;
    let state = pb.intin(2) as u8;
    if let Some(h) = host() {
        while crate::vdi::mouse_buttons() & mask != state & mask {
            h.pump_input();
        }
    }
    fill_rets(pb);
    pb.set_ret(clicks);
}

/// `evnt_mouse` (22) — block until the mouse enters (`flags` = 0) or
/// leaves (`flags` = 1) the rectangle in `int_in[1..4]`, then fill the
/// return slots and return 1.
///
/// MOBLK: `int_in[0]` = flags, `int_in[1..4]` = x, y, w, h. The event
/// fires when `flags != inside` (EmuTOS `in_mrect`).
pub fn evnt_mouse(pb: &Aespb) {
    let flags = pb.intin(0);
    let rx = pb.intin(1);
    let ry = pb.intin(2);
    let rw = pb.intin(3);
    let rh = pb.intin(4);
    if let Some(h) = host() {
        loop {
            let (mx, my) = crate::vdi::mouse_position();
            let inside = mx >= rx && mx < rx + rw && my >= ry && my < ry + rh;
            if (flags != 0) != inside {
                break;
            }
            h.pump_input();
        }
    }
    fill_rets(pb);
    pb.set_ret(1);
}

/// AES double-click speed index (0..4); EmuTOS startup default is 3
/// (`geminit.c:708`, `ev_dclick(3, TRUE)`).
static DCLICK_INDEX: AtomicI16 = AtomicI16::new(3);

/// `evnt_dclick` (26) — get (`int_in[1]` = 0) or set (`int_in[1]` != 0)
/// the double-click speed index `int_in[0]`. Returns the current index.
pub fn evnt_dclick(pb: &Aespb) {
    let rate = pb.intin(0); // EV_DCRATE
    let setit = pb.intin(1); // EV_DCSETIT
    if setit != 0 {
        DCLICK_INDEX.store(rate, Ordering::Relaxed);
    }
    pb.set_ret(DCLICK_INDEX.load(Ordering::Relaxed));
}

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
