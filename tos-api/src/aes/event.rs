//! AES Event Manager (`evnt_*`, opcodes 20–26).
//!
//! Spec: [`docs/api/aes/event.md`]; the `int_in`/`int_out` slot layout is
//! the EmuTOS `include/aesdefs.h` mapping (e.g. `T_LOCOUNT = int_in[0]`,
//! `RET_CODE = int_out[0]`).
//!
//! **Wait model (sleep/wake substrate, cooperative-kernel steps 3a + 3b):**
//! EmuTOS' `ev_block` parks the calling AES process and yields to others until
//! an event fires. Each of the six `evnt_*` waits below re-checks its predicate
//! and, when it has not fired, blocks on [`sched::sleep`] with the matching
//! wake-class key ([`sched::wait_input`]/[`sched::wait_mesg`]/
//! [`sched::wait_timer`]) — the single cooperative block primitive replacing the
//! former per-function `pump_input` spin (`docs/analysis/coop-kernel/`
//! `20-sleepwake-substrate-design.md` §5).
//!
//! `sleep` returns a [`sched::SleepOutcome`]: [`Recheck`](sched::SleepOutcome::
//! Recheck) (no peer — the step-3 single-context idle-fallback, identical to the
//! old spin) or [`Yield`](sched::SleepOutcome::Yield)`(Switch)` (a runnable peer
//! exists — step 3b). On `Yield` the wait **aborts and returns the `Switch`**
//! ([`Option<sched::Switch>`]) up through [`super::dispatch`] to
//! `rust_sync_handler`, which saves this process's frame with the ELR rewound to
//! the `svc` (the S2 restartable block) and switches to the peer; on resume the
//! re-executed `svc` re-enters the wait, which re-checks and fires. Because the
//! block path is read-only until the firing pass (design §4.3), an abort commits
//! nothing. The observable `evnt_*` contract (return values, `MU_*` bits,
//! message dequeue-on-firing) is unchanged.

use core::sync::atomic::{AtomicI16, Ordering};

use super::pd;
use super::Aespb;
use crate::bdos::process::sched;
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

/// AES menu-bar interception (mirroring the EmuTOS Control Manager's
/// `hctl_rect`, `gemctrl.c:318`): if a menu bar is installed and the cursor is
/// over it, run the drop-down tracker (`menu::track`) — GEM hover menus, no
/// click needed to open. On a selection, post an `MN_SELECTED` message into
/// the menu owner's per-app FIFO — the EmuTOS `ct_msgup(MN_SELECTED, gl_mnppd,
/// …)`→`ap_sendmsg`→`aqueue`/`doq` path (`gemctrl.c:353-367`,
/// `gemwmlib.c:851-863`, `gemqueue.c:85-125`) — and return `true`; the caller
/// then delivers it through the regular `MU_MESAG` dequeue, so a menu
/// selection can no longer clobber (or be clobbered by) an already-queued
/// message (AUD-P2-002). Returns `false` when there is no menu, the cursor is
/// not over the bar, or the drop-down was cancelled (no message).
///
/// `pid` is the current AES process — the menu owner (`gl_mnppd`) at
/// single-app — used both as the FIFO target and as the `ap_sendmsg` sender
/// pid `msg[1] = rlr->p_pid` (`gemwmlib.c:855`), replacing the former `0`.
fn menu_intercept(pid: i16) -> bool {
    let tree = super::menu::active();
    if tree.is_null() {
        return false;
    }
    // Hover model: the drop-down opens whenever the cursor is over the bar, no
    // click required (the tracker selects on a button press over an item).
    let (_mx, my) = crate::vdi::mouse_position();
    if my >= MENU_BAR_H {
        return false;
    }
    let Some(backend) = host().and_then(|h| h.raster()) else {
        return false;
    };
    match super::menu::track(tree, backend) {
        Some((title, item)) => {
            let mut msg = [0i16; pd::MSG_WORDS];
            msg[0] = MN_SELECTED;
            msg[1] = pid; // ap_sendmsg: msg[1] = rlr->p_pid (sender pid)
            msg[3] = title; // menu title object
            msg[4] = item; // menu item object
            // SAFETY: single trap path; `post_msg` hands the message to the
            // live slot (EmuTOS `aqueue`/`doq`, `gemqueue.c:85-125`).
            unsafe {
                pd::post_msg(pid, msg);
            }
            true
        }
        None => false, // cancelled — the click is consumed, no message
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
/// requested state, then return the **actual** click count and fill the
/// mouse / keyboard return slots.
///
/// `int_in[0]` = clicks to wait for (`B_CLICKS`), `int_in[1]` = button
/// mask (`B_MASK`), `int_in[2]` = desired masked state (`B_STATE`).
/// `RET_CODE` = clicks reached; `int_out[1..4]` = x, y, buttons, shift.
///
/// Waits for the masked button bits to reach the requested state (the
/// first click), then counts any further presses that fall inside the
/// double-click window (`dclick_ms()`) up to the requested maximum, and
/// returns the *actual* click count (1 for a single click, 2 for a double
/// click within the window). The window counting is factored into the
/// shared [`count_clicks`] helper, which `evnt_multi` reuses.
pub fn evnt_button(pb: &Aespb) -> Option<sched::Switch> {
    let clicks = pb.intin(0);
    let mask = pb.intin(1) as u8;
    let state = pb.intin(2) as u8;
    let mut got = 0i16;
    if let Some(h) = host() {
        let matches = || crate::vdi::mouse_buttons() & mask == state & mask;
        // Wait for the button to reach the requested state (the first click).
        while !matches() {
            if let sched::SleepOutcome::Yield(sw) = sched::sleep(sched::wait_input()) {
                return Some(sw); // yield to a peer (S2); re-checks on resume
            }
        }
        // Count further clicks inside the double-click window. `evnt_button`
        // has no timer source, so counting runs to the full window (no outer
        // deadline: `cap = None`).
        got = count_clicks(h, clicks, None, matches);
    }
    fill_rets(pb);
    pb.set_ret(got.max(1));
    None
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

/// Count mouse-button clicks in the double-click window, given the button has
/// *already* reached the requested state for the first click — so the count
/// starts at 1. Each further click must leave and then re-reach the state (as
/// reported by `matches`) within one `dclick_ms()` window. Counting stops when
/// `max_clicks` is reached, when a window closes without another press, or when
/// `cap` (an optional outer deadline) is reached. Returns the click count
/// (always >= 1).
///
/// This is the shared counter behind both `evnt_button` and `evnt_multi`'s
/// `MU_BUTTON` branch, so a single click and a double click are told apart the
/// same way in either call (`EV_BRET` / `RET_CODE`).
///
/// * `evnt_button` passes `cap = None`: it has no timer source, so counting
///   runs to the full double-click window.
/// * `evnt_multi` passes `cap = Some(deadline)` iff `MU_TIMER` is armed, so the
///   click count never overruns the multi-wait's timer deadline nor stalls the
///   other event sources (keyboard/message) past it. Without a timer it too
///   uses `cap = None`; either way the stall is bounded by one window, and any
///   keystroke/message that arrives meanwhile is buffered and seen on the next
///   loop pass (it is delayed, never lost).
fn count_clicks(
    h: &dyn crate::host::Host,
    max_clicks: i16,
    cap: Option<u64>,
    matches: impl Fn() -> bool,
) -> i16 {
    let win = dclick_ms();
    let mut got = 1i16;
    while got < max_clicks {
        let mut deadline = h.monotonic_millis().saturating_add(win);
        if let Some(cap) = cap {
            deadline = deadline.min(cap);
        }
        // The button must leave, then re-reach, the state within the window.
        if !wait_until(h, deadline, || !matches()) || !wait_until(h, deadline, &matches) {
            break;
        }
        got += 1;
    }
    got
}

/// Resolve a single vs. double left-button click for the Form Manager's modal
/// loop (EmuTOS `fm_do` waits on `ev_multi` with `0x0002ff01L` = up to 2 left
/// clicks). The caller has already detected the press edge, so the left button
/// is down (the first click); count a second click that falls inside the
/// double-click window. Returns 1 or 2. Used by `form::run_form` only for a
/// TOUCHEXIT object, where the count sets the 0x8000 return bit.
pub(super) fn count_form_clicks(h: &dyn crate::host::Host) -> i16 {
    count_clicks(h, 2, None, || crate::vdi::mouse_buttons() & 0x01 == 0x01)
}

/// `evnt_mouse` (22) — block until the mouse enters (`flags` = 0) or
/// leaves (`flags` = 1) the rectangle in `int_in[1..4]`, then fill the
/// return slots and return 1.
///
/// MOBLK: `int_in[0]` = flags, `int_in[1..4]` = x, y, w, h. The event
/// fires when `flags != inside` (EmuTOS `in_mrect`).
pub fn evnt_mouse(pb: &Aespb) -> Option<sched::Switch> {
    let m = MoBlk {
        flags: pb.intin(0),
        x: pb.intin(1),
        y: pb.intin(2),
        w: pb.intin(3),
        h: pb.intin(4),
    };
    if host().is_some() {
        while !rect_event(&m) {
            if let sched::SleepOutcome::Yield(sw) = sched::sleep(sched::wait_input()) {
                return Some(sw); // yield to a peer (S2); re-checks on resume
            }
        }
    }
    fill_rets(pb);
    pb.set_ret(1);
    None
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
/// keys included). The `MU_BUTTON` click count (`EV_BRET`) is the real
/// count from [`count_clicks`] (1 = single, 2 = double click within the
/// double-click window), so callers can tell a Select from an Open.
/// Bring-up limit: single AES application.
pub fn evnt_multi(pb: &Aespb) -> Option<sched::Switch> {
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
        // Persist the timer deadline per-slot for S2-restart idempotency
        // (design §4.3 OPEN): the first entry stores `now + ms`, an S2 restart
        // (this whole loop re-executes from the top on resume) reuses it, and
        // any completion below clears it. Only meaningful when `MU_TIMER` is
        // armed; without a timer the value is unused (`cap = None`, no MU_TIMER
        // check), so it is computed but not persisted.
        let deadline = if flags & MU_TIMER != 0 {
            sched::arm_or_get_deadline(h.monotonic_millis(), ms)
        } else {
            h.monotonic_millis().saturating_add(ms)
        };
        loop {
            // AES intercepts a click in the menu bar or a window frame before
            // the app sees the raw button, mirroring the EmuTOS Control Manager
            // process (`gemctrl.c:427`): it owns the mouse over the menu/frame
            // and posts the resulting MN_SELECTED / WM_* message into the
            // target's per-app FIFO via `ap_sendmsg` (`gemctrl.c:81`,
            // `gemwmlib.c:851`). The message reaches the app through the regular
            // MU_MESAG dequeue below — no direct `pbuff` write, no bypass. Done
            // only when the app actually requested MU_MESAG: EmuTOS sets
            // AP_MESAG (and processes the frame click) solely for MU_MESAG
            // (`gemsuper.c:147`); an app waiting on MU_BUTTON alone is not woken
            // by a frame click and must not receive a spurious MU_MESAG
            // (AUD-P2-003).
            if flags & MU_MESAG != 0 && !menu_intercept(pid) {
                super::window::intercept();
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
                // Resolve the *real* click count (single vs. double) inside the
                // double-click window — the same counter `evnt_button` uses, so
                // the desktop can tell Select (1) from Open (2). Cap counting at
                // the multi-wait deadline when a timer is armed so it never
                // overruns MU_TIMER (a 0-ms poll timer collapses to a count of
                // 1, matching the "poll now" contract).
                let cap = (flags & MU_TIMER != 0).then_some(deadline);
                clicks = count_clicks(h, b_clicks, cap, || {
                    crate::vdi::mouse_buttons() & b_mask == b_state & b_mask
                });
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
                    // Pop the FRONT of the FIFO (EmuTOS `ap_rdwr(MU_RDMSG)`
                    // →`doq` dequeue, `gemqueue.c:76-81`); a single pending
                    // message is just the 1-element case.
                    if let Some(msg) = src.queue.pop() {
                        if pbuff != 0 {
                            // SAFETY: `MME_PBUFF` points at >= 16 bytes.
                            unsafe {
                                let p = pbuff as *mut i16;
                                for (i, &w) in msg.iter().enumerate() {
                                    p.add(i).write(w);
                                }
                            }
                        }
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
            // Not fired: block until the cooperative idle re-check (design §5).
            // `WAIT_INPUT` is the multiplexer's general re-check trigger — every
            // armed source (keyboard/button/mouse, and the menu/window frame
            // interception that produces `MU_MESAG`) is input-driven, and the
            // `MU_TIMER` deadline is checked by this same loop above. The union
            // of message/timer classes becomes load-bearing at step 4 (§4.1);
            // in step 3a the caller re-checks the full predicate each pass.
            //
            // The loop above the block is read-only (a message is popped only on
            // the `fired |= MU_MESAG` firing pass), so on a peer yield (S2) it is
            // safe to abort and re-execute from the top on resume (§4.3).
            if let sched::SleepOutcome::Yield(sw) = sched::sleep(sched::wait_input()) {
                return Some(sw);
            }
        }
    }

    // Completed (an event fired, or no host): clear the persisted timer deadline
    // so a later timer wait recomputes fresh — any firing path reaches here,
    // whereas a peer yield above returned early and left the deadline armed for
    // the restart (S2-restart idempotency, §4.3).
    sched::clear_deadline();
    fill_rets(pb);
    pb.set_intout(5, key); // EV_KRET
    pb.set_intout(6, clicks); // EV_BRET
    pb.set_ret(fired as i16);
    None
}

/// `evnt_keybd` (20) — block until a key is pressed; return it in
/// `RET_CODE` as `scancode << 8 | ascii`.
///
/// The key word comes from the IKBD key ring (`console_try_read_key`): the
/// low byte is the ASCII character (0 for keys with none), the high byte
/// the Atari scancode, so cursor / function keys reach the application.
/// The GEMDOS console path still reads the ASCII byte only.
pub fn evnt_keybd(pb: &Aespb) -> Option<sched::Switch> {
    let key = match host() {
        Some(h) => loop {
            if let Some(w) = h.console_try_read_key() {
                break w as i16; // scancode << 8 | ascii
            }
            if let sched::SleepOutcome::Yield(sw) = sched::sleep(sched::wait_input()) {
                return Some(sw); // yield to a peer (S2); re-checks on resume
            }
        },
        None => 0,
    };
    pb.set_ret(key);
    None
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
pub fn evnt_mesag(pb: &Aespb) -> Option<sched::Switch> {
    let dest = pb.addrin(0);
    let pid = super::appl::current_pid();
    if host().is_none() {
        pb.set_ret(1);
        return None;
    }
    loop {
        // SAFETY: single trap path; `by_pid` hands out the live slot.
        let Some(src) = (unsafe { pd::by_pid(pid) }) else {
            // No descriptor (caller never ran appl_init): nothing to
            // wait on; report the message event and bail.
            pb.set_ret(1);
            return None;
        };
        // Pop the FRONT of the FIFO (EmuTOS `ap_rdwr(MU_RDMSG)`→`doq` dequeue,
        // `gemqueue.c:76-81`).
        if let Some(msg) = src.queue.pop() {
            if dest != 0 {
                // SAFETY: `ME_PBUFF` points at >= 16 bytes of storage.
                unsafe {
                    let p = dest as *mut i16;
                    for (i, &w) in msg.iter().enumerate() {
                        p.add(i).write(w);
                    }
                }
            }
            src.flags |= pd::AP_MESAG;
            pb.set_ret(1);
            return None;
        }
        // No message yet: block until one is posted to this pid's FIFO
        // (`pd::post_msg` wakes `WAIT_MESG(pid)`, design §5.2). The pop above
        // runs only when a message is present, so aborting to a peer (S2) and
        // re-executing on resume never double-pops (§4.3).
        if let sched::SleepOutcome::Yield(sw) = sched::sleep(sched::wait_mesg(pid)) {
            return Some(sw);
        }
    }
}

/// `evnt_timer` (24) — block for `ms` milliseconds, then return.
///
/// `int_in[0]` = low WORD of the millisecond count, `int_in[1]` = high
/// WORD (`T_LOCOUNT`/`T_HICOUNT`). `RET_CODE` is 1 (the timer event).
pub fn evnt_timer(pb: &Aespb) -> Option<sched::Switch> {
    let lo = pb.intin(0) as u16 as u32;
    let hi = pb.intin(1) as u16 as u32;
    let ms = ((hi << 16) | lo) as u64;

    if let Some(sw) = wait_ms(ms) {
        return Some(sw); // yield to a peer (S2); reuses the deadline on resume
    }
    // Completed (deadline reached, or `ms == 0`): clear the persisted deadline
    // so a later timer wait recomputes fresh (S2-restart idempotency, §4.3).
    sched::clear_deadline();
    pb.set_ret(1);
    None
}

/// Block for `ms` milliseconds against the host's monotonic clock. A
/// zero count returns immediately (EmuTOS `ev_timer(0)` semantics). With
/// no host installed the clock stands still, so this returns at once.
fn wait_ms(ms: u64) -> Option<sched::Switch> {
    if ms == 0 {
        return None;
    }
    if let Some(h) = host() {
        let now = h.monotonic_millis();
        // Persist the absolute deadline per-slot so an S2 restart REUSES it
        // instead of recomputing `now + ms` — which, re-entered from the top on
        // every restart with a live peer, would creep forward by `ms` and never
        // converge (design §4.3 OPEN). First entry stores; a restart reuses;
        // `evnt_timer` clears it on completion. `monotonic_millis` is monotonic
        // so a plain comparison is safe (no wraparound within any realistic
        // uptime).
        let deadline = sched::arm_or_get_deadline(now, ms);
        // Block until the deadline. The idle wake-pass wakes `WAIT_TIMER` once
        // `monotonic_millis >= deadline` (design §5.2); the caller's own
        // deadline check drives the loop in the single-context step-3a case.
        while h.monotonic_millis() < deadline {
            if let sched::SleepOutcome::Yield(sw) = sched::sleep(sched::wait_timer(deadline)) {
                return Some(sw); // yield to a peer (S2); reuses the deadline on resume
            }
        }
    }
    None
}
