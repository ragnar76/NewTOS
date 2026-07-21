//! `desk` — the EmuTOS-conformant NewTOS desktop (ADR-016 ground-up rebuild).
//!
//! This is the new home of the desktop *application* (EmuTOS `desk/`),
//! reimplemented in idiomatic `#![no_std]` Rust on top of the AES/VDI/GEMDOS
//! layer in `tos-api`. It supersedes the legacy `crate::desktop` /
//! `crate::desktop_icons` modules (kept in the tree, out of the build, until
//! feature parity — see `docs/api/desktop/_rebuild-plan.md`).
//!
//! Scope so far: the data model ([`global`]/[`model`]/[`win`]), the boot
//! one-time setup, the window/icon view builder ([`app`]/[`win`], Task #6), the
//! damage-rectangle redraw ([`redraw`], Task #7), and the **interaction layer**
//! ([`main`]/[`menu`], Task #8): the `evnt_multi` loop + message drain, mouse
//! button dispatch (`hndl_button` via `wind_find` — fixes D3), keyboard
//! (`hndl_kbd` — fixes D4), the menu bar + `hndl_menu` + `men_update`
//! (enable/disable/check — fixes D6), and window move/size/top/close wiring.
//! File/INF ops and the drag/rubber-band selection machine are Task #9.
//!
//! This module root hosts the shared **aesbind** surface — the thin GEM-trap
//! wrappers the desktop calls the AES through, exactly as EmuTOS `desk/` goes
//! over `aesbind` — plus the desktop-background painter hook and the boot
//! entry point [`run`].

pub mod act;
pub mod app;
pub mod dir;
pub mod dlg;
pub mod ftype;
pub mod global;
pub mod icons;
pub mod inf;
pub mod insapp;
pub mod main;
pub mod menu;
pub mod model;
pub mod redraw;
pub mod win;

use crate::kprintln;

use global::{Grect, MAX_DEPTH, WINDOW_STYLE};

// ── AES parameter-block client ("aesbind") ─────────────────────────────────
//
// The desktop is a client of the AES: it drives it through the GEM trap
// (`svc #2`, selector 200) with a parameter block of six array pointers,
// exactly like EmuTOS `desk/` calls the AES over `aesbind`. Every AES call the
// desktop makes lives here as a thin `pub(super)` wrapper so `main`/`menu`
// never touch the raw parameter block.

// AES opcodes (`aes/mod.rs` dispatch table).
const OP_EVNT_BUTTON: i16 = 21;
const OP_EVNT_MULTI: i16 = 25;
const OP_MENU_BAR: i16 = 30;
const OP_MENU_TNORMAL: i16 = 33;
const OP_OBJC_DRAW: i16 = 42;
const OP_OBJC_FIND: i16 = 43;
const OP_FORM_DO: i16 = 50;
const OP_FORM_ALERT: i16 = 52;
const OP_GRAF_RUBBERBOX: i16 = 70;
const OP_GRAF_DRAGBOX: i16 = 71;
const OP_GRAF_MOUSE: i16 = 78;
const OP_GRAF_MKSTATE: i16 = 79;
const OP_WIND_CREATE: i16 = 100;
const OP_WIND_OPEN: i16 = 101;
const OP_WIND_CLOSE: i16 = 102;
const OP_WIND_DELETE: i16 = 103;
const OP_WIND_GET: i16 = 104;
const OP_WIND_SET: i16 = 105;
const OP_WIND_FIND: i16 = 106;
const OP_WIND_CALC: i16 = 108;

// wind_get/set `WF_*` fields and wind_calc `WC_*` types.
pub(super) const WF_NAME: i16 = 2;
pub(super) const WF_INFO: i16 = 3;
pub(super) const WF_WXYWH: i16 = 4;
pub(super) const WF_CXYWH: i16 = 5;
pub(super) const WF_PXYWH: i16 = 6;
pub(super) const WF_FXYWH: i16 = 7;
pub(super) const WF_HSLIDE: i16 = 8;
pub(super) const WF_VSLIDE: i16 = 9;
pub(super) const WF_TOP: i16 = 10;
pub(super) const WF_FIRSTXYWH: i16 = 11;
pub(super) const WF_NEXTXYWH: i16 = 12;
const WF_NEWDESK: i16 = 14;
pub(super) const WF_HSLSIZ: i16 = 15;
pub(super) const WF_VSLSIZ: i16 = 16;
const WC_WORK: i16 = 1;

// graf_mouse cursor-control numbers (`aesdefs.h`): hide / show around a paint.
pub(super) const M_OFF: i16 = 256;
pub(super) const M_ON: i16 = 257;

// evnt_multi event-class bits (`aesdefs.h`).
pub(super) const MU_KEYBD: u16 = 0x0001;
pub(super) const MU_BUTTON: u16 = 0x0002;
pub(super) const MU_MESAG: u16 = 0x0010;
pub(super) const MU_TIMER: u16 = 0x0020;

/// Issue the GEM trap (`svc #2`): selector 200 in `x0`, AESPB in `x1`.
fn gem_trap(aespb: u64) {
    // SAFETY: GEM trap; results are written back into the parameter block.
    unsafe {
        core::arch::asm!(
            "svc #2",
            inout("x0") 200u64 => _,
            in("x1") aespb,
            options(nostack),
        );
    }
}

/// A reusable AES parameter block. [`Aes::call`] zeroes `control`, sets the
/// opcode + array lengths, and traps; results land in `intout`/`addrout`.
struct Aes {
    control: [i16; 8],
    global: [i16; 32],
    intin: [i16; 16],
    intout: [i16; 16],
    addrin: [i64; 4],
    addrout: [i64; 4],
}

impl Aes {
    fn new() -> Self {
        Self {
            control: [0; 8],
            global: [0; 32],
            intin: [0; 16],
            intout: [0; 16],
            addrin: [0; 4],
            addrout: [0; 4],
        }
    }

    /// Trap opcode `op` with `n_int` int-ins and `n_addr` addr-ins.
    fn call(&mut self, op: i16, n_int: i16, n_addr: i16) {
        self.control = [0; 8];
        self.control[0] = op;
        self.control[1] = n_int;
        self.control[3] = n_addr;
        let block: [u64; 6] = [
            self.control.as_ptr() as u64,
            self.global.as_ptr() as u64,
            self.intin.as_ptr() as u64,
            self.intout.as_ptr() as u64,
            self.addrin.as_ptr() as u64,
            self.addrout.as_ptr() as u64,
        ];
        gem_trap(block.as_ptr() as u64);
    }
}

// ── aesbind wrappers (window manager) ──────────────────────────────────────

/// `objc_draw` (42) the subtree rooted at `root` of the desktop object arena
/// `tree`, clipped to `clip`. Used by `do_wredraw` to paint a window's item
/// tree **and** the desktop's own surface (`DROOT` background box + icons) — the
/// "two paths, one tree" of EmuTOS `do_wredraw`.
pub(super) fn objc_draw_tree(tree: u64, root: i16, clip: Grect) {
    let mut a = Aes::new();
    a.addrin[0] = tree as i64;
    a.intin[0] = root;
    a.intin[1] = MAX_DEPTH;
    a.intin[2] = clip.x;
    a.intin[3] = clip.y;
    a.intin[4] = clip.w;
    a.intin[5] = clip.h;
    a.call(OP_OBJC_DRAW, 6, 1);
}

/// `objc_find` (43) — deepest object of `tree` under `(mx, my)` within `depth`
/// levels of `start`; [`global::NIL`] if none. Resolves the icon/text item a
/// click landed on (the `hndl_button` hit-test).
pub(super) fn objc_find(tree: u64, start: i16, depth: i16, mx: i16, my: i16) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = tree as i64;
    a.intin[0] = start;
    a.intin[1] = depth;
    a.intin[2] = mx;
    a.intin[3] = my;
    a.call(OP_OBJC_FIND, 4, 1);
    a.intout[0]
}

/// `wind_find` (106) — handle of the window under screen point `(mx, my)`;
/// `0` (= [`win::DESKWH`]) for the desktop. The D3 fix: a click is routed to
/// *whichever* window it fell in, not a single hard-coded one.
pub(super) fn wind_find(mx: i16, my: i16) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = mx;
    a.intin[1] = my;
    a.call(OP_WIND_FIND, 2, 0);
    a.intout[0]
}

/// `wind_get(handle, WF_WXYWH)` — the window's current work-area rectangle.
pub(super) fn wind_get_work(handle: i16) -> Grect {
    wind_get_grect(handle, WF_WXYWH)
}

/// `wind_get(handle, field)` for a rectangle field (`WF_WXYWH`/`WF_CXYWH`/
/// `WF_FXYWH`/`WF_FIRSTXYWH`/`WF_NEXTXYWH`) → the returned `GRECT`.
pub(super) fn wind_get_grect(handle: i16, field: i16) -> Grect {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.intin[1] = field;
    a.call(OP_WIND_GET, 2, 0);
    Grect { x: a.intout[1], y: a.intout[2], w: a.intout[3], h: a.intout[4] }
}

/// `wind_get(handle, WF_FIRSTXYWH | WF_NEXTXYWH)` — one rectangle from the
/// window's AES **owner-rectangle list**: the occlusion-aware visible pieces
/// of its work area (the work rect minus every higher-Z window). `WF_FIRSTXYWH`
/// (re)starts the walk and seeds the first piece; `WF_NEXTXYWH` advances it. A
/// returned rectangle with `w == 0 || h == 0` is the end-of-list sentinel
/// (`desksupp-redraw.md`). Backed by `tos_api::aes::window`'s `compute_rlist`.
pub(super) fn wind_get_rect(handle: i16, field: i16) -> Grect {
    wind_get_grect(handle, field)
}

/// `wind_get(DESKWH, WF_TOP)` — the handle of the top-most open window, or
/// [`win::DESKWH`] when only the desktop is showing.
pub(super) fn wind_get_top() -> i16 {
    let mut a = Aes::new();
    a.intin[0] = win::DESKWH;
    a.intin[1] = WF_TOP;
    a.call(OP_WIND_GET, 2, 0);
    a.intout[1]
}

/// `wind_set(handle, field, x, y, w, h)` — a rectangle field (`WF_CXYWH`).
pub(super) fn wind_set_grect(handle: i16, field: i16, r: Grect) {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.intin[1] = field;
    a.intin[2] = r.x;
    a.intin[3] = r.y;
    a.intin[4] = r.w;
    a.intin[5] = r.h;
    a.call(OP_WIND_SET, 6, 0);
}

/// `wind_set(handle, field, value)` — a scalar field (`WF_TOP` with no value,
/// the slider size/position fields).
pub(super) fn wind_set_int(handle: i16, field: i16, value: i16) {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.intin[1] = field;
    a.intin[2] = value;
    a.call(OP_WIND_SET, 5, 0);
}

/// `wind_set(handle, field, ptr)` — a pointer field (`WF_NAME`/`WF_INFO`).
/// `ptr` must stay valid across the trap.
pub(super) fn wind_set_ptr(handle: i16, field: i16, ptr: u64) {
    let mut a = Aes::new();
    a.addrin[0] = ptr as i64;
    a.intin[0] = handle;
    a.intin[1] = field;
    a.call(OP_WIND_SET, 2, 1);
}

/// `wind_set(DESKWH, WF_NEWDESK, tree, root)` — register (`tree != 0`) or
/// un-register (`tree == 0`) the desktop's background object tree with the AES
/// (EmuTOS `deskmain.c:2049`/`:2163`). Between registration and un-registration
/// the AES's `w_drawdesk` draws `tree` from `root` (the desktop's `DROOT`, its
/// background `G_BOX` + icon children) at `MAX_DEPTH` on every desktop redraw —
/// the desktop no longer paints its own background. The tree pointer travels in
/// `addr_in[0]`, the root in `int_in[2]` (the AES resolves either this native
/// packing or the classic D-094 `int_in[2..4]` form). `tree` must stay valid for
/// as long as it is registered (the desktop's `g_screen` arena is never freed).
pub(super) fn wind_set_newdesk(tree: u64, root: i16) {
    let mut a = Aes::new();
    a.addrin[0] = tree as i64;
    a.intin[0] = win::DESKWH;
    a.intin[1] = WF_NEWDESK;
    a.intin[2] = root;
    a.call(OP_WIND_SET, 3, 1);
}

/// `wind_create(kind, x, y, w, h)` — reserve a window of border `kind` and max
/// extent `r`; returns the handle (`>= 1`) or a negative error.
pub(super) fn wind_create(kind: i16, r: Grect) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = kind;
    a.intin[1] = r.x;
    a.intin[2] = r.y;
    a.intin[3] = r.w;
    a.intin[4] = r.h;
    a.call(OP_WIND_CREATE, 5, 0);
    a.intout[0]
}

/// `wind_open(handle, x, y, w, h)` — display window `handle` at `r`, topmost.
pub(super) fn wind_open(handle: i16, r: Grect) {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.intin[1] = r.x;
    a.intin[2] = r.y;
    a.intin[3] = r.w;
    a.intin[4] = r.h;
    a.call(OP_WIND_OPEN, 5, 0);
}

/// `wind_close(handle)` — hide window `handle` (the handle stays valid).
pub(super) fn wind_close(handle: i16) {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.call(OP_WIND_CLOSE, 1, 0);
}

/// `wind_delete(handle)` — free the AES window handle.
pub(super) fn wind_delete(handle: i16) {
    let mut a = Aes::new();
    a.intin[0] = handle;
    a.call(OP_WIND_DELETE, 1, 0);
}

/// The instantaneous mouse/keyboard state from `graf_mkstate` (79).
pub(super) struct MouseState {
    /// Mouse x.
    pub mx: i16,
    /// Mouse y.
    pub my: i16,
    /// Button mask (bit 0 = left).
    pub buttons: i16,
    /// Keyboard shift/ctrl/alt state (`K_*` bits).
    pub kstate: i16,
}

/// `graf_mkstate` (79) — poll the mouse position, button mask, and keyboard
/// shift state without waiting. `hndl_button` uses it after `act_bsclick` to
/// decide whether the button is still held (a drag) or already released (a
/// plain click) — EmuTOS `deskmain.c:968`.
pub(super) fn graf_mkstate() -> MouseState {
    let mut a = Aes::new();
    a.call(OP_GRAF_MKSTATE, 0, 0);
    MouseState { mx: a.intout[1], my: a.intout[2], buttons: a.intout[3], kstate: a.intout[4] }
}

/// `graf_rubberbox(x, y, minw, minh)` (70) — anchor a rubber-band box at
/// `(x, y)`; the opposite corner tracks the mouse until the button is released.
/// Returns the final `(w, h)` (EmuTOS `act_bdown`'s rubber-band branch).
pub(super) fn graf_rubberbox(x: i16, y: i16, minw: i16, minh: i16) -> (i16, i16) {
    let mut a = Aes::new();
    a.intin[0] = x;
    a.intin[1] = y;
    a.intin[2] = minw;
    a.intin[3] = minh;
    a.call(OP_GRAF_RUBBERBOX, 4, 0);
    (a.intout[1], a.intout[2])
}

/// `graf_dragbox(w, h, sx, sy, bound)` (71) — drag a fixed `w×h` outline box
/// from `(sx, sy)`, clamped to `bound`, until the button is released. Returns
/// the final `(x, y)`. NewTOS's single-outline replacement for EmuTOS's
/// multi-icon XOR drag polygon (`gr_drgplns`) — the *result* (where the drop
/// lands) is what the file-op contract observes, per `deskact.md`.
pub(super) fn graf_dragbox(w: i16, h: i16, sx: i16, sy: i16, bound: Grect) -> (i16, i16) {
    let mut a = Aes::new();
    a.intin[0] = w;
    a.intin[1] = h;
    a.intin[2] = sx;
    a.intin[3] = sy;
    a.intin[4] = bound.x;
    a.intin[5] = bound.y;
    a.intin[6] = bound.w;
    a.intin[7] = bound.h;
    a.call(OP_GRAF_DRAGBOX, 8, 0);
    (a.intout[1], a.intout[2])
}

/// `evnt_button(clicks, mask, state)` (21) — wait until the button reaches
/// `state` (masked). Used to drain the button-up after a double-click open, so
/// the next `evnt_multi` does not re-see the same release (EmuTOS
/// `deskmain.c:1004`).
pub(super) fn evnt_button(clicks: i16, mask: i16, state: i16) {
    let mut a = Aes::new();
    a.intin[0] = clicks;
    a.intin[1] = mask;
    a.intin[2] = state;
    a.call(OP_EVNT_BUTTON, 3, 0);
}

/// `graf_mouse(number)` (78) — hide ([`M_OFF`]) or show ([`M_ON`]) the mouse
/// cursor. `do_wredraw` brackets its paint with these so the cursor's
/// save-under does not corrupt the blit (EmuTOS `desksupp.c:221`/`:231`).
pub(super) fn graf_mouse(number: i16) {
    let mut a = Aes::new();
    a.intin[0] = number;
    a.call(OP_GRAF_MOUSE, 1, 0);
}

/// `wind_calc(WC_WORK, WINDOW_STYLE, desk)` — the work-area width of a
/// maximum-size window, feeding `win_view`'s `g_icols`.
pub(super) fn wind_calc_work_width(desk: Grect) -> i16 {
    let mut a = Aes::new();
    a.intin[0] = WC_WORK;
    a.intin[1] = WINDOW_STYLE as i16;
    a.intin[2] = desk.x;
    a.intin[3] = desk.y;
    a.intin[4] = desk.w;
    a.intin[5] = desk.h;
    a.call(OP_WIND_CALC, 6, 0);
    a.intout[3]
}

// ── aesbind wrappers (menu manager) ────────────────────────────────────────

/// `menu_bar(tree, show)` (30) — install (`show = 1`) or remove the menu bar.
pub(super) fn menu_bar(tree: u64, show: i16) {
    let mut a = Aes::new();
    a.addrin[0] = tree as i64;
    a.intin[0] = show;
    a.call(OP_MENU_BAR, 1, 1);
}

/// `menu_tnormal(tree, title, normal)` (33) — draw `title` normal (`normal =
/// 1`) or highlighted (`0`).
pub(super) fn menu_tnormal(tree: u64, title: i16, normal: bool) {
    let mut a = Aes::new();
    a.addrin[0] = tree as i64;
    a.intin[0] = title;
    a.intin[1] = normal as i16;
    a.call(OP_MENU_TNORMAL, 2, 1);
}

// ── aesbind wrappers (form manager + event manager) ────────────────────────

/// `form_alert(default, text)` (52) — modal alert; `text` is a
/// NUL-terminated `[icon][message][buttons]` string. Returns the 1-based
/// button clicked. Used for the Desk-menu About/Info box.
pub(super) fn form_alert(default: i16, text: *const u8) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = text as i64;
    a.intin[0] = default;
    a.call(OP_FORM_ALERT, 1, 1);
    a.intout[0]
}

/// `form_do(tree, start)` (50) — run a modal dialog on `tree` until an
/// `EXIT`/`TOUCHEXIT` object (or Enter on the default button) is chosen; returns
/// the object index that ended it. Editable `TEDINFO` fields are edited in place
/// (the desktop's New-folder / Rename name boxes). `tree` must stay valid across
/// the call.
pub(super) fn form_do(tree: u64) -> i16 {
    let mut a = Aes::new();
    a.addrin[0] = tree as i64;
    a.intin[0] = 0; // editable-field focus start
    a.call(OP_FORM_DO, 1, 1);
    a.intout[0]
}

/// `objc_draw` (42) the whole tree `tree` from `ROOT` (0) — used to paint a
/// self-built dialog before [`form_do`] runs it.
pub(super) fn objc_draw_dialog(tree: u64, clip: Grect) {
    objc_draw_tree(tree, 0, clip);
}

/// The outputs of an [`evnt_multi`] wait the desktop consumes.
pub(super) struct EvntOut {
    /// Bitmask of the event classes that fired (`RET_CODE`).
    pub which: u16,
    /// Mouse x (`EV_MX`).
    pub mx: i16,
    /// Mouse y (`EV_MY`).
    pub my: i16,
    /// The key word `scancode << 8 | ascii` (`EV_KRET`).
    pub kret: i16,
    /// The click count (`EV_BRET`).
    pub bret: i16,
}

/// `evnt_multi(flags, …)` (25) — block until any requested event fires and
/// return which, plus the mouse / key / click state. The button parameters are
/// the desktop's fixed `bclk = 2` (report up to a double click), `bmask = 1`
/// (left button), `bstate = 1` (down) — the EmuTOS `deskmain.c:2078` values.
/// The MOBLK and timer slots stay zero; a `flags` that includes [`MU_TIMER`]
/// therefore returns immediately once no other event is pending (the
/// message-drain "quick-out"), while the blocking outer loop omits it.
pub(super) fn evnt_multi(flags: u16, msg: &mut [i16; 8]) -> EvntOut {
    let mut a = Aes::new();
    a.addrin[0] = msg.as_mut_ptr() as i64;
    a.intin[0] = flags as i16;
    a.intin[1] = 2; // bclk: report up to a double click
    a.intin[2] = 1; // bmask: left button
    a.intin[3] = 1; // bstate: down
    a.call(OP_EVNT_MULTI, 16, 1);
    EvntOut {
        which: a.intout[0] as u16,
        mx: a.intout[1],
        my: a.intout[2],
        kret: a.intout[5],
        bret: a.intout[6],
    }
}

// ── Boot entry ─────────────────────────────────────────────────────────────

/// The desktop entry point (EmuTOS `deskmain`): one-time setup (metrics, the
/// desktop icon tree, menu bar), open the first directory window, paint the
/// full desktop, then hand off to the interaction event loop ([`main::run`]).
pub fn run() {
    let Some(backend) = tos_api::host::host().and_then(|h| h.raster()) else {
        kprintln!("[desk] no framebuffer — desktop not started");
        return;
    };

    // One-time setup: DeskState (view/sort defaults, window list, object arena).
    global::init(backend.width() as i16, backend.height() as i16);

    // Grid metrics: app_start (icon cell + desktop pitch) → win_view (window
    // pitch; needs the full-window work width from the AES) → app_blddesk
    // (desktop drive/trash/printer icon tree).
    let desk = global::with(|g| g.g_desk).unwrap_or_default();
    let full_work_w = wind_calc_work_width(desk);
    global::with(|g| {
        g.full_work_w = full_work_w;
        g.app_start();
    });

    // Read NEWDESK.INF and apply it (view/sort, g_patcol background, desktop-icon
    // grid positions, window rectangles) — after `app_start` so the grid pitch
    // (`g_icw`/`g_ich`) exists for the icon-position rounding, before `win_view`/
    // `app_blddesk` so the view and positions take effect. A missing file is not
    // an error (the desktop keeps its built-in defaults).
    inf::load_and_apply();

    global::with(|g| {
        g.win_view();
        g.app_blddesk();
    });

    // Register the desktop's own object tree as the AES screen background
    // (EmuTOS `wind_set(DESKWH, WF_NEWDESK, G.g_screen, 1, 0)`, `deskmain.c:2049`):
    // from now on the AES's `w_drawdesk` draws `DROOT` (the background box) + the
    // icon children whenever the desktop area is exposed — the desktop no longer
    // paints its own background. Then install the menu bar (drawn once — it is
    // never covered, windows live below it).
    if let Some(tree) = global::screen_addr() {
        wind_set_newdesk(tree, global::DROOT);
    }
    menu::install_menu_bar();

    // Open the C:\ directory window and build its view, set the menu state to
    // reflect it (men_update — the D6 machinery), then paint everything (desktop
    // bg + icons + frames via redraw_all, each window's content
    // occlusion-clipped via `do_wredraw`).
    main::open_root_window(b'C');
    menu::men_update();
    redraw::full_redraw();

    // Park the mouse pointer at the screen centre so the desktop boots into a
    // clean idle state (GEM positions the pointer at startup). Without this the
    // cursor sits at (0,0) — `tos-api`'s `graf_mouse(M_ON)` re-stores the
    // pre-`ensure_init` position — over the menu bar, whose AES hover-menu would
    // then open the Desk drop-down at boot. Reported to the lead as a `tos-api`
    // cursor-init note; positioning the pointer here is a faithful GEM startup
    // action, not a workaround for a menu-semantics gap.
    tos_api::vdi::mouse::display_at(backend.width() as i16 / 2, backend.height() as i16 / 2, backend);

    kprintln!(
        "[desk] desktop up ({} drives)",
        tos_api::bios::drive::drvbits().count_ones()
    );

    // Hand off to the interaction loop (never returns while the desktop runs).
    main::event_loop();
}
