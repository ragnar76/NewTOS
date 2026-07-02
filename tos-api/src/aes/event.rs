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

// Event-type flags (`include/aesdefs.h`), selecting which events a wait
// blocks on. Used by `evnt_multi`.
const MU_KEYBD: u16 = 0x0001;
const MU_BUTTON: u16 = 0x0002;
const MU_M1: u16 = 0x0004;
const MU_M2: u16 = 0x0008;
const MU_MESAG: u16 = 0x0010;
const MU_TIMER: u16 = 0x0020;
const MU_TOSVALID: u16 = 0x003F;

/// A mouse rectangle block (`MOBLK`): `flags` selects enter (0) vs leave
/// (1) of the `{x, y, w, h}` rectangle.
struct MoBlk {
    flags: i16,
    x: i16,
    y: i16,
    w: i16,
    h: i16,
}

/// `true` when a `MOBLK`'s condition is satisfied (EmuTOS `in_mrect`:
/// the event fires when `flags != inside`).
fn rect_event(m: &MoBlk) -> bool {
    let (mx, my) = crate::vdi::mouse_position();
    let inside = mx >= m.x && mx < m.x + m.w && my >= m.y && my < m.y + m.h;
    (m.flags != 0) != inside
}

/// Menu-bar height in pixels (matches `menu::BAR_H`).
const MENU_BAR_H: i16 = 19;
/// `MN_SELECTED` message id.
const MN_SELECTED: i16 = 10;

/// AES menu-bar interception: if a menu bar is installed and the cursor is
/// over it, run the drop-down tracker (`menu::track`) — GEM hover menus, no
/// click needed to open. On a selection, write an `MN_SELECTED` message into
/// `pbuff` and return `MU_MESAG`; otherwise (no menu, not in the bar, or
/// cancelled) return 0.
fn menu_intercept(pbuff: u64) -> u16 {
    let tree = super::menu::active();
    if tree.is_null() {
        return 0;
    }
    // Hover model: the drop-down opens whenever the cursor is over the bar, no
    // click required (the tracker selects on a button press over an item).
    let (_mx, my) = crate::vdi::mouse_position();
    if my >= MENU_BAR_H {
        return 0;
    }
    let Some(backend) = host().and_then(|h| h.raster()) else {
        return 0;
    };
    match super::menu::track(tree, backend) {
        Some((title, item)) => {
            if pbuff != 0 {
                // SAFETY: `MME_PBUFF` points at >= MSG_WORDS WORDs.
                unsafe {
                    let p = pbuff as *mut i16;
                    for i in 0..pd::MSG_WORDS {
                        p.add(i).write(0);
                    }
                    p.write(MN_SELECTED); // msg[0]
                    p.add(3).write(title); // msg[3] = menu title object
                    p.add(4).write(item); // msg[4] = menu item object
                }
            }
            MU_MESAG
        }
        None => 0, // cancelled — the click is consumed, no message
    }
}

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
    let mut got = 0i16;
    if let Some(h) = host() {
        let matches = |m: u8, s: u8| crate::vdi::mouse_buttons() & m == s & m;
        // Wait for the button to reach the requested state (the first click).
        while !matches(mask, state) {
            h.pump_input();
        }
        got = 1;
        // Count further press cycles that fall inside the double-click window
        // (EmuTOS reports the number of clicks up to the requested maximum).
        let win = dclick_ms();
        while got < clicks {
            let deadline = h.monotonic_millis().saturating_add(win);
            // The button must leave, then re-reach, the state within `win`.
            if !wait_until(h, deadline, || !matches(mask, state))
                || !wait_until(h, deadline, || matches(mask, state))
            {
                break;
            }
            got += 1;
        }
    }
    fill_rets(pb);
    pb.set_ret(got.max(1));
}

/// Double-click intervals per speed index 0..4 (EmuTOS `gl_dcrates`, ms).
const DCRATES: [u64; 5] = [450, 330, 275, 220, 165];

/// The current double-click window in milliseconds.
fn dclick_ms() -> u64 {
    DCRATES[DCLICK_INDEX.load(Ordering::Relaxed).clamp(0, 4) as usize]
}

/// Poll `cond` until it holds or the deadline passes; returns whether it
/// became true in time.
fn wait_until(h: &dyn crate::host::Host, deadline: u64, cond: impl Fn() -> bool) -> bool {
    while h.monotonic_millis() < deadline {
        if cond() {
            return true;
        }
        h.pump_input();
    }
    cond()
}

/// `evnt_mouse` (22) — block until the mouse enters (`flags` = 0) or
/// leaves (`flags` = 1) the rectangle in `int_in[1..4]`, then fill the
/// return slots and return 1.
///
/// MOBLK: `int_in[0]` = flags, `int_in[1..4]` = x, y, w, h. The event
/// fires when `flags != inside` (EmuTOS `in_mrect`).
pub fn evnt_mouse(pb: &Aespb) {
    let m = MoBlk {
        flags: pb.intin(0),
        x: pb.intin(1),
        y: pb.intin(2),
        w: pb.intin(3),
        h: pb.intin(4),
    };
    if let Some(h) = host() {
        while !rect_event(&m) {
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

/// `evnt_multi` (25) — the central event multiplexer. Block until **any**
/// of the requested events (`MU_*` in `int_in[0]`) fires, then return the
/// bitmask of those that did and report the mouse / keyboard / button
/// state for all of them at once.
///
/// `int_in[0]` = flags, `int_in[1..3]` = button clicks/mask/state,
/// `int_in[4..8]` = MOBLK 1, `int_in[9..13]` = MOBLK 2,
/// `int_in[14..15]` = timer lo/hi ms, `addr_in[0]` = message buffer.
/// `RET_CODE` = fired bitmask; `int_out[1..4]` = x, y, buttons, shift;
/// `int_out[5]` = key (`EV_KRET`); `int_out[6]` = click count (`EV_BRET`).
///
/// Each single `evnt_*` wait is a special case of this loop. `EV_KRET`
/// carries the full `scancode << 8 | ascii` key word (cursor / function
/// keys included). Bring-up limits: single application, single-click
/// button matching.
pub fn evnt_multi(pb: &Aespb) {
    let flags = (pb.intin(0) as u16) & MU_TOSVALID;

    let b_clicks = pb.intin(1);
    let b_mask = pb.intin(2) as u8;
    let b_state = pb.intin(3) as u8;
    let m1 = MoBlk {
        flags: pb.intin(4),
        x: pb.intin(5),
        y: pb.intin(6),
        w: pb.intin(7),
        h: pb.intin(8),
    };
    let m2 = MoBlk {
        flags: pb.intin(9),
        x: pb.intin(10),
        y: pb.intin(11),
        w: pb.intin(12),
        h: pb.intin(13),
    };
    let lo = pb.intin(14) as u16 as u32;
    let hi = pb.intin(15) as u16 as u32;
    let ms = ((hi << 16) | lo) as u64;
    let pbuff = pb.addrin(0);
    let pid = super::appl::current_pid();

    let mut fired = 0u16;
    let mut key = 0i16;
    let mut clicks = 0i16;

    if let Some(h) = host() {
        let deadline = h.monotonic_millis().saturating_add(ms);
        loop {
            // AES intercepts a click in the menu bar or a window frame
            // before the app sees the raw button — it yields an
            // MN_SELECTED / WM_* message.
            if flags & (MU_BUTTON | MU_MESAG) != 0 {
                let m = menu_intercept(pbuff);
                if m != 0 {
                    fired |= m;
                } else if super::window::intercept(pbuff) {
                    fired |= MU_MESAG;
                }
            }
            if flags & MU_KEYBD != 0 {
                if let Some(w) = h.console_try_read_key() {
                    key = w as i16; // scancode << 8 | ascii (EV_KRET)
                    fired |= MU_KEYBD;
                }
            }
            if flags & MU_BUTTON != 0
                && crate::vdi::mouse_buttons() & b_mask == b_state & b_mask
            {
                clicks = b_clicks;
                fired |= MU_BUTTON;
            }
            if flags & MU_M1 != 0 && rect_event(&m1) {
                fired |= MU_M1;
            }
            if flags & MU_M2 != 0 && rect_event(&m2) {
                fired |= MU_M2;
            }
            if flags & MU_MESAG != 0 {
                // SAFETY: single trap path; `by_pid` hands out the live slot.
                if let Some(src) = unsafe { pd::by_pid(pid) } {
                    if src.msg_pending {
                        if pbuff != 0 {
                            // SAFETY: `MME_PBUFF` points at >= 16 bytes.
                            unsafe {
                                let p = pbuff as *mut i16;
                                for i in 0..pd::MSG_WORDS {
                                    p.add(i).write(src.msg[i]);
                                }
                            }
                        }
                        src.msg_pending = false;
                        src.flags |= pd::AP_MESAG;
                        fired |= MU_MESAG;
                    }
                }
            }
            if flags & MU_TIMER != 0 && h.monotonic_millis() >= deadline {
                fired |= MU_TIMER;
            }

            if fired != 0 {
                break;
            }
            h.pump_input();
        }
    }

    fill_rets(pb);
    pb.set_intout(5, key); // EV_KRET
    pb.set_intout(6, clicks); // EV_BRET
    pb.set_ret(fired as i16);
}

/// `evnt_keybd` (20) — block until a key is pressed; return it in
/// `RET_CODE` as `scancode << 8 | ascii`.
///
/// The key word comes from the IKBD key ring (`console_try_read_key`): the
/// low byte is the ASCII character (0 for keys with none), the high byte
/// the Atari scancode, so cursor / function keys reach the application.
/// The GEMDOS console path still reads the ASCII byte only.
pub fn evnt_keybd(pb: &Aespb) {
    let key = match host() {
        Some(h) => loop {
            if let Some(w) = h.console_try_read_key() {
                break w as i16; // scancode << 8 | ascii
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
