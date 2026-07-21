//! AES Window Manager (`wind_*`, opcodes 100–109) — Batch 1.
//!
//! Spec: [`docs/api/aes/window-graphics.md`]. Source: EmuTOS @ `1a7484ab`
//! `aes/gemwmlib.c` (`wm_*`, `wm_calc`, `w_getsize`). A window has a
//! **border** rectangle (the full frame) and a **work** rectangle (the
//! client area inside the gadgets); `wm_calc` converts between them from
//! the `kind` widget bitmask. The frame is drawn **procedurally** at our
//! altitude (each gadget rect computed from the border rect + `kind`),
//! rather than via EmuTOS's 19-object `W_ACTIVE` tree — same visible
//! frame and hit-testing without porting the tree machinery.
//!
//! **Batch-1 scope:** the window table + lifecycle (`create`/`open`/
//! `close`/`delete`), geometry (`calc`/`get`/`set`/`find`), and frame
//! drawing (border, title + name, closer/fuller/sizer, scroll arrows +
//! slider tracks). The `WM_*` message protocol (redraw/topped/moved/…),
//! the rectangle list (visible pieces), interactive drag/resize, and
//! `graf_*` are Batch 2+. No save/restore under windows yet (see the
//! Menu-Manager note); closing erases to the desktop background.

use super::Aespb;
use crate::host::host;
use crate::vdi::backend::{ClipRect, Pel, PatternFill, RasterBackend, Rect, WriteMode};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering};

// ── kind (border widget) bits, aesdefs.h:519-531 ──
const NAME: u16 = 0x0001;
const CLOSER: u16 = 0x0002;
const FULLER: u16 = 0x0004;
const MOVER: u16 = 0x0008;
const INFO: u16 = 0x0010;
const SIZER: u16 = 0x0020;
const UPARROW: u16 = 0x0040;
const DNARROW: u16 = 0x0080;
const VSLIDE: u16 = 0x0100;
const LFARROW: u16 = 0x0200;
const RTARROW: u16 = 0x0400;
const HSLIDE: u16 = 0x0800;

const VGADGETS: u16 = UPARROW | DNARROW | VSLIDE;
const HGADGETS: u16 = LFARROW | RTARROW | HSLIDE;

// ── WF_* fields, aesdefs.h:534-552 ──
const WF_KIND: i16 = 1;
const WF_NAME: i16 = 2;
const WF_INFO: i16 = 3;
const WF_WXYWH: i16 = 4;
const WF_CXYWH: i16 = 5;
const WF_HSLIDE: i16 = 8;
const WF_VSLIDE: i16 = 9;
const WF_HSLSIZ: i16 = 15;
const WF_VSLSIZ: i16 = 16;
const WF_PXYWH: i16 = 6;
const WF_FXYWH: i16 = 7;
const WF_TOP: i16 = 10;
const WF_FIRSTXYWH: i16 = 11;
const WF_NEXTXYWH: i16 = 12;
const WF_SCREEN: i16 = 17;

// ── wind_calc types ──
const WC_BORDER: i16 = 0;
const WC_WORK: i16 = 1;

// ── gadget glyphs (ST system font 0x01-0x07) ──
const G_UP: u16 = 0x01;
const G_DN: u16 = 0x02;
const G_RT: u16 = 0x03;
const G_LF: u16 = 0x04;
const G_CLOSE: u16 = 0x05;
const G_SIZE: u16 = 0x06;
const G_FULL: u16 = 0x07;

/// Gadget cell size (`gl_hbox` = char height + 3; square gadgets). This is
/// also EmuTOS `gl_wbox`/`gl_hbox`: for the 8×16 system font both are 19
/// (`gl_hbox = gl_hchar+3 = 19`, `gl_wbox = max(gl_wchar+4, gl_hbox·aspect) =
/// 19`, `gemgraf.c:412-415`), so `graf::graf_handle` returns this same unit as
/// its box metric instead of a second magic 19.
pub(super) const GAD: i16 = 19;

const WHITE: Pel = 0;
const BLACK: Pel = 1;
const GRAY_LT: Pel = 8; // 3D face (EmuTOS LWHITE)
const GRAY_DK: Pel = 9; // 3D shadow (EmuTOS LBLACK)

/// The classic GEM "move-bar" dither on an **active** window's name area
/// (EmuTOS `TOPPED_COLOR` = `IP_2PATT`, a light 25 % black stipple). The
/// inactive window's name bar is left plain — the presence of the stipple is
/// the active/inactive cue, matching EmuTOS.
const TITLE_STIPPLE: [u16; 4] = [0x0000, 0x5555, 0x0000, 0x5555];

const MAX_WINDOWS: usize = 8;
const NAME_CAP: usize = 80;

/// The desktop pseudo-window's AES handle (EmuTOS `DESKWH`) — the bottom of the
/// window z-order. It has no table slot; its rectangle list is the desk area
/// (the screen minus the menu bar) minus every open window's border rect.
const DESKWH: i16 = 0;

/// A rectangle (GEM `GRECT`).
#[derive(Clone, Copy, Default, PartialEq)]
struct Grect {
    x: i16,
    y: i16,
    w: i16,
    h: i16,
}

/// One window slot.
#[derive(Clone, Copy)]
struct Window {
    in_use: bool,
    open: bool,
    kind: u16,
    z: i16,     // stacking order (higher = nearer the top)
    owner: i16, // owning process id (WM_* message target)
    full: Grect, // maximum extent (WF_FXYWH)
    curr: Grect, // current border rect (WF_CXYWH)
    prev: Grect, // previous border rect (WF_PXYWH)
    name: [u8; NAME_CAP],
    info: [u8; NAME_CAP],
    /// Vertical slider position (`WF_VSLIDE`, 1..1000 per-mille of travel)
    /// and thumb size (`WF_VSLSIZ`, 1..1000 per-mille of the track, or **−1**
    /// = "use default size", one gadget cell). `wind_create` inits the size to
    /// −1 (EmuTOS `w_vslsiz`, gemwmlib.c:232); the getter round-trips −1.
    vslide: i16,
    vslsize: i16,
    /// Horizontal slider position (`WF_HSLIDE`) and thumb size (`WF_HSLSIZ`),
    /// same per-mille encoding as the vertical pair (mirror axis); size −1 =
    /// default.
    hslide: i16,
    hslsize: i16,
}

impl Window {
    const EMPTY: Window = Window {
        in_use: false,
        open: false,
        kind: 0,
        z: 0,
        owner: 0,
        full: Grect { x: 0, y: 0, w: 0, h: 0 },
        curr: Grect { x: 0, y: 0, w: 0, h: 0 },
        prev: Grect { x: 0, y: 0, w: 0, h: 0 },
        name: [0; NAME_CAP],
        info: [0; NAME_CAP],
        vslide: 0,
        vslsize: -1,
        hslide: 0,
        hslsize: -1,
    };
}

/// The window table. Single boot CPU only → `UnsafeCell` with a hand-rolled
/// `Sync`, mirroring the cookie jar and AES process table.
struct WinTable {
    slots: UnsafeCell<[Window; MAX_WINDOWS]>,
    next_z: UnsafeCell<i16>, // monotonically increasing stacking counter
}
// SAFETY: only the single boot CPU touches the AES; no concurrent access.
unsafe impl Sync for WinTable {}

static TABLE: WinTable = WinTable {
    slots: UnsafeCell::new([Window::EMPTY; MAX_WINDOWS]),
    next_z: UnsafeCell::new(1),
};

/// Saved copy of the desktop's window set for the single-tasking **screen
/// handoff** ([`save_windows`] / [`restore_windows`]). Disjoint from [`TABLE`]:
/// [`save_windows`] snapshots the live table here and clears the live one so a
/// launched GEM app gets a clean window list; [`restore_windows`] copies it back
/// on the child's `Pterm`. `active` guards a restore with no prior save.
struct WinSave {
    slots: UnsafeCell<[Window; MAX_WINDOWS]>,
    next_z: UnsafeCell<i16>,
    active: UnsafeCell<bool>,
}
// SAFETY: only the single boot CPU touches the AES; no concurrent access.
unsafe impl Sync for WinSave {}

static SAVE: WinSave = WinSave {
    slots: UnsafeCell::new([Window::EMPTY; MAX_WINDOWS]),
    next_z: UnsafeCell::new(1),
    active: UnsafeCell::new(false),
};

#[allow(clippy::mut_from_ref)]
fn slots() -> &'static mut [Window; MAX_WINDOWS] {
    // SAFETY: single-CPU AES; exclusive logical owner.
    unsafe { &mut *TABLE.slots.get() }
}

/// Next stacking value (raises a window to the top of the z-order).
fn bump_z() -> i16 {
    // SAFETY: single-CPU AES.
    unsafe {
        let p = TABLE.next_z.get();
        let z = *p;
        *p = z + 1;
        z
    }
}

/// Handle of the top-most open window (highest `z`), or 0 (desktop).
fn get_top() -> i16 {
    let mut best = 0i16;
    let mut best_z = i16::MIN;
    for (i, w) in slots().iter().enumerate() {
        if w.in_use && w.open && w.z >= best_z {
            best_z = w.z;
            best = (i + 1) as i16;
        }
    }
    best
}

/// Raise `handle` to the top of the stack and repaint only the union of the
/// newly-topped window and the previously-top window (which loses its
/// active-title highlight) — not the whole screen (the topping-flicker fix,
/// same damage-scope as move/open/close).
fn top_window(handle: i16) {
    let old_top = get_top(); // current top before the bump (may equal handle)
    let old_rect = win(old_top).map(|w| w.curr);
    if let Some(w) = win(handle) {
        w.z = bump_z();
    }
    let new_rect = win(handle).map(|w| w.curr);
    let damage = match (new_rect, old_rect) {
        (Some(n), Some(o)) => Some(union_grect(n, o)),
        (Some(n), None) => Some(n),
        _ => None,
    };
    if let Some(d) = damage {
        redraw_clip(d.x, d.y, d.w, d.h);
    }
}

/// Optional desktop-background painter. A GEM application that owns the
/// desktop (e.g. the NewTOS desktop) registers a pure drawing function
/// here; the window manager calls it to repaint the background underneath
/// the windows (the EmuTOS DESKWH/DROOT idea), restricted to a clip
/// rectangle. `0` = none → plain white erase of the clip.
///
/// The painter runs **inside** `redraw_all` and the solid window drag, i.e.
/// inside the AES SVC handler, so it must draw straight through the backend
/// and issue **no further AES trap** (no re-entrant `svc`).
static DESK_PAINTER: AtomicUsize = AtomicUsize::new(0);

/// Register the desktop-background painter (see [`DESK_PAINTER`]).
pub fn set_desktop_painter(f: fn(&dyn RasterBackend, ClipRect)) {
    DESK_PAINTER.store(f as usize, Ordering::Release);
}

/// Neutralise the desktop-background painter (the inverse of
/// [`set_desktop_painter`]): the next [`paint_bg_clip`] does the plain white
/// erase — literally EmuTOS's `CLEAR_SCREEN` — instead of the desktop's
/// green background + icons.
///
/// Used by the single-tasking **screen handoff**: when the desktop launches a
/// GEM app it relinquishes the shared AES screen state, so the child composites
/// on a clean white screen rather than over the desktop's own background. The
/// desktop re-registers its painter via [`set_desktop_painter`] on the child's
/// `Pterm` (see [`save_windows`] / [`restore_windows`]).
pub fn clear_desktop_painter() {
    DESK_PAINTER.store(0, Ordering::Release);
}

/// Screen handoff — **relinquish** the desktop's windows: snapshot the whole
/// shared window table into [`SAVE`] and clear the live table (every desktop
/// window is hidden, its z-order counter reset). The saved state is *preserved*,
/// not destroyed, so [`restore_windows`] can rebuild it exactly; the launched
/// GEM app then gets a fresh window list (all handles free), composites over no
/// desktop windows, and its own `wind_*` calls run against the empty table.
///
/// This mirrors EmuTOS tearing the desktop's windows (`wm_init`) when a child is
/// launched. It is called from the desktop's launch path (kernel) together with
/// [`clear_desktop_painter`] and the menu-bar removal; the caller then clears the
/// screen (a full composite over the empty table + neutralised painter is the
/// white `CLEAR_SCREEN`).
pub fn save_windows() {
    // SAFETY: single-CPU AES; `SAVE` and `TABLE` are disjoint statics and
    // `[Window; MAX_WINDOWS]` is `Copy`, so these are plain array copies.
    unsafe {
        *SAVE.slots.get() = *TABLE.slots.get();
        *SAVE.next_z.get() = *TABLE.next_z.get();
        *SAVE.active.get() = true;
        // Hide every desktop window by resetting the live table (its state is
        // held in `SAVE`); reset the z counter for the child.
        *TABLE.slots.get() = [Window::EMPTY; MAX_WINDOWS];
        *TABLE.next_z.get() = 1;
    }
}

/// Screen handoff — **resume** the desktop's windows: restore the set saved by
/// [`save_windows`] back into the shared window table, discarding whatever the
/// child left there. A no-op if no set is currently saved. Called from the
/// desktop's launch path on the child's `Pterm`, alongside re-registering the
/// desktop painter ([`set_desktop_painter`]) and a repaint.
pub fn restore_windows() {
    // SAFETY: single-CPU AES; disjoint `Copy` statics (see [`save_windows`]).
    unsafe {
        if !*SAVE.active.get() {
            return;
        }
        *TABLE.slots.get() = *SAVE.slots.get();
        *TABLE.next_z.get() = *SAVE.next_z.get();
        *SAVE.active.get() = false;
    }
}

/// Paint the desktop background within `clip`: the registered painter if any
/// (clipped), else a plain white erase of the clip.
fn paint_bg_clip(backend: &dyn RasterBackend, clip: ClipRect) {
    let p = DESK_PAINTER.load(Ordering::Acquire);
    if p != 0 {
        // SAFETY: only ever set via `set_desktop_painter` from that same
        // `fn(&dyn RasterBackend, ClipRect)` pointer.
        let f: fn(&dyn RasterBackend, ClipRect) = unsafe { core::mem::transmute(p) };
        f(backend, clip);
    } else if let Some((x0, y0, x1, y1)) = clip.clamp_to(backend.width(), backend.height()) {
        backend.fill_rect(
            x0 as i16, y0 as i16, (x1 - x0 + 1) as i16, (y1 - y0 + 1) as i16, WHITE, clip,
        );
    }
}

/// A `ClipRect` for the rectangle `[x, x+w) × [y, y+h)`.
fn rect_clip(x: i16, y: i16, w: i16, h: i16) -> ClipRect {
    ClipRect { xmn: x, ymn: y, xmx: x + w - 1, ymx: y + h - 1 }
}

/// Repaint the desktop and every open window back-to-front (painter's
/// algorithm — front windows overwrite the ones they cover). The simple
/// stand-in for per-window rectangle-list clipping when redrawing frames;
/// the rectangle list (`WF_FIRSTXYWH`) still drives what the *app* redraws.
fn redraw_all(backend: &dyn RasterBackend) {
    compose(backend, ClipRect::full(backend.width(), backend.height()));
}

/// The desktop-background + window-frame composite, confined to `clip`. Paints
/// the desktop background (painter hook) then every open window's frame
/// back-to-front (painter's algorithm — front windows overwrite the ones they
/// cover), each clipped to `clip`. [`redraw_all`] passes the full-device clip;
/// [`redraw_clip`] passes a damage rectangle so a move/size settle repaints only
/// the affected area instead of the whole screen. The window **content** (item
/// objects) is re-laid by the owner afterwards, exactly as in the full path.
fn compose(backend: &dyn RasterBackend, clip: ClipRect) {
    paint_bg_clip(backend, clip);
    let top = get_top();
    // Selection sort by ascending z, drawing each open window once.
    let mut drawn = [false; MAX_WINDOWS];
    loop {
        let mut pick = usize::MAX;
        let mut pick_z = i16::MAX;
        for (i, w) in slots().iter().enumerate() {
            if w.in_use && w.open && !drawn[i] && w.z <= pick_z {
                // pick the lowest z not yet drawn; tie-break stable
                if pick == usize::MAX || w.z < pick_z {
                    pick = i;
                    pick_z = w.z;
                }
            }
        }
        if pick == usize::MAX {
            break;
        }
        drawn[pick] = true;
        let istop = (pick as i16 + 1) == top;
        draw_frame(&slots()[pick], istop, backend, clip);
    }
}

/// Damage-scoped repaint of the desktop background + every window frame, clipped
/// to `clip` (a border-rect union). The public entry the desktop drives after a
/// solid window move/resize (WM_MOVED/WM_SIZED) and the programmatic WF_CXYWH
/// path use, in place of [`redraw`] (whole-screen `redraw_all`) — only the union
/// of the window's old and new border rects is repainted, which removes the
/// full-screen flicker. The window content is re-laid by the desktop
/// (`do_wredraw`) / by the `send_redraw` WM_REDRAW that follows.
pub fn redraw_clip(x: i16, y: i16, w: i16, h: i16) {
    if w <= 0 || h <= 0 {
        return;
    }
    if let Some(backend) = host().and_then(|h| h.raster()) {
        let cr = ClipRect { xmn: x, ymn: y, xmx: x + w - 1, ymx: y + h - 1 };
        compose(backend, cr);
    }
}

/// The bounding rectangle enclosing both `a` and `b` — the damage region of a
/// window move/resize (old border rect ∪ new border rect).
fn union_grect(a: Grect, b: Grect) -> Grect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = (a.x + a.w).max(b.x + b.w);
    let y1 = (a.y + a.h).max(b.y + b.h);
    Grect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
}

// ── rectangle list (visible pieces) ──────────────────────────────────────

const RCAP: usize = 16;
const WM_REDRAW: i16 = 20;

/// Iterator state for `WF_FIRSTXYWH`/`WF_NEXTXYWH` — the visible pieces of
/// the window most recently queried. Single-CPU, like the window table.
struct RList {
    rects: UnsafeCell<[Grect; RCAP]>,
    count: UnsafeCell<usize>,
    cursor: UnsafeCell<usize>,
}
unsafe impl Sync for RList {}
static RLIST: RList = RList {
    rects: UnsafeCell::new([Grect { x: 0, y: 0, w: 0, h: 0 }; RCAP]),
    count: UnsafeCell::new(0),
    cursor: UnsafeCell::new(0),
};

fn put(out: &mut [Grect; RCAP], m: &mut usize, r: Grect) {
    if r.w > 0 && r.h > 0 && *m < RCAP {
        out[*m] = r;
        *m += 1;
    }
}

/// Subtract covering rect `c` from each rect in `list[0..n]`, producing the
/// non-overlapping remainder. Returns the new count.
fn subtract(list: &mut [Grect; RCAP], n: usize, c: Grect) -> usize {
    let mut out = [Grect::default(); RCAP];
    let mut m = 0usize;
    for r in list.iter().take(n).copied() {
        // No intersection → keep whole.
        if c.x >= r.x + r.w || c.x + c.w <= r.x || c.y >= r.y + r.h || c.y + c.h <= r.y {
            put(&mut out, &mut m, r);
            continue;
        }
        if r.y < c.y {
            put(&mut out, &mut m, Grect { x: r.x, y: r.y, w: r.w, h: c.y - r.y });
        }
        if r.y + r.h > c.y + c.h {
            put(&mut out, &mut m, Grect { x: r.x, y: c.y + c.h, w: r.w, h: (r.y + r.h) - (c.y + c.h) });
        }
        let oy = r.y.max(c.y);
        let oh = (r.y + r.h).min(c.y + c.h) - oy;
        if r.x < c.x {
            put(&mut out, &mut m, Grect { x: r.x, y: oy, w: c.x - r.x, h: oh });
        }
        if r.x + r.w > c.x + c.w {
            put(&mut out, &mut m, Grect { x: c.x + c.w, y: oy, w: (r.x + r.w) - (c.x + c.w), h: oh });
        }
    }
    *list = out;
    m
}

/// Compute `handle`'s visible work-area pieces (work rect minus every
/// higher-z open window) into `RLIST`, cursor reset.
///
/// [`DESKWH`] (the desktop pseudo-window) is treated as the **bottom** of the
/// z-order: its work area is the desk rectangle ([`desktop_work`], the screen
/// minus the menu bar) and *every* open window is above it, so the subtraction
/// loop removes each open window's border rect. This is the occlusion-aware
/// rectangle list EmuTOS's `do_wredraw(DESKWH, …)` walks uniformly with real
/// windows (`desksupp.c:223`), so the desktop background never reports — and
/// therefore never repaints over — a piece a window covers.
fn compute_rlist(handle: i16) {
    // SAFETY: single-CPU AES.
    let (rects, count, cursor) = unsafe {
        (&mut *RLIST.rects.get(), &mut *RLIST.count.get(), &mut *RLIST.cursor.get())
    };
    *count = 0;
    *cursor = 0;
    let (work, myz) = if handle == DESKWH {
        // Desktop: desk area, z below every window (i16::MIN → all subtracted).
        (desktop_work(), i16::MIN)
    } else {
        match win(handle) {
            Some(w) if w.open => (calc(WC_WORK, w.kind, w.curr), w.z),
            _ => return,
        }
    };
    let mut list = [Grect::default(); RCAP];
    list[0] = work;
    let mut n = 1;
    for ow in slots().iter() {
        if ow.in_use && ow.open && ow.z > myz {
            n = subtract(&mut list, n, ow.curr);
        }
    }
    rects[..n].copy_from_slice(&list[..n]);
    *count = n;
}

/// Next piece from `RLIST` (empty `Grect` when exhausted).
fn rlist_next() -> Grect {
    // SAFETY: single-CPU AES.
    unsafe {
        let count = *RLIST.count.get();
        let cursor = &mut *RLIST.cursor.get();
        if *cursor < count {
            let r = (*RLIST.rects.get())[*cursor];
            *cursor += 1;
            r
        } else {
            Grect::default()
        }
    }
}

/// Post a `WM_REDRAW` for `rect` to `handle`'s owner process.
fn send_redraw(handle: i16, rect: Grect) {
    let owner = match win(handle) {
        Some(w) => w.owner,
        None => return,
    };
    let mut msg = [0i16; 8];
    msg[0] = WM_REDRAW;
    msg[1] = super::appl::current_pid();
    msg[3] = handle;
    msg[4] = rect.x;
    msg[5] = rect.y;
    msg[6] = rect.w;
    msg[7] = rect.h;
    // SAFETY: by_pid hands out the live process slot on the single trap path.
    unsafe {
        if let Some(p) = super::pd::by_pid(owner) {
            p.msg = msg;
            p.msg_pending = true;
        }
    }
}

/// Map a window handle (1-based) to a table slot, if valid and in use.
fn win(handle: i16) -> Option<&'static mut Window> {
    if handle < 1 || handle as usize > MAX_WINDOWS {
        return None;
    }
    let w = &mut slots()[(handle - 1) as usize];
    w.in_use.then_some(w)
}

// ── geometry ────────────────────────────────────────────────────────────

/// Border thickness contributed by `kind` on each side: `(left, top,
/// right, bottom)` (EmuTOS `wm_calc`, non-3D path).
fn borders(kind: u16) -> (i16, i16, i16, i16) {
    let lb = 1;
    let mut tb = 1;
    let mut rb = 1;
    let mut bb = 1;
    if kind & (NAME | CLOSER | FULLER) != 0 {
        tb += GAD - 1;
    }
    if kind & INFO != 0 {
        tb += GAD - 1;
    }
    if kind & (VGADGETS | SIZER) != 0 {
        rb += GAD - 1;
    }
    if kind & (HGADGETS | SIZER) != 0 {
        bb += GAD - 1;
    }
    (lb, tb, rb, bb)
}

/// Convert a rectangle between border and work coordinates (`wm_calc`).
fn calc(wtype: i16, kind: u16, r: Grect) -> Grect {
    let (mut lb, mut tb, mut rb, mut bb) = borders(kind);
    if wtype == WC_BORDER {
        lb = -lb;
        tb = -tb;
        rb = -rb;
        bb = -bb;
    }
    Grect {
        x: r.x + lb,
        y: r.y + tb,
        w: r.w - lb - rb,
        h: r.h - tb - bb,
    }
}

// ── frame drawing ───────────────────────────────────────────────────────

fn rect_outline(r: Grect, backend: &dyn RasterBackend, clip: ClipRect) {
    backend.fill_rect(r.x, r.y, r.w, 1, BLACK, clip); // top
    backend.fill_rect(r.x, r.y + r.h - 1, r.w, 1, BLACK, clip); // bottom
    backend.fill_rect(r.x, r.y, 1, r.h, BLACK, clip); // left
    backend.fill_rect(r.x + r.w - 1, r.y, 1, r.h, BLACK, clip); // right
}

/// Draw one bordered gadget cell with a centred glyph (closer/fuller/
/// arrows/sizer). `sel` inverts it (pressed feedback).
fn gadget_sel(r: Grect, ch: u16, sel: bool, backend: &dyn RasterBackend, clip: ClipRect) {
    // Modern 3D gadget: grey face + bevel (raised, recessed when pressed).
    backend.fill_rect(r.x, r.y, r.w, r.h, GRAY_LT, clip);
    bevel(r, !sel, backend, clip);
    let gx = r.x + (r.w - 8) / 2;
    let gy = r.y + (r.h - 16) / 2;
    backend.draw_glyph(gx, gy, crate::vdi::font::glyph(ch), BLACK, WriteMode::Transparent, clip);
}

/// Draw a 2-pixel 3D bevel: `raised` = white top/left + dark-grey
/// bottom/right; recessed swaps them.
fn bevel(r: Grect, raised: bool, backend: &dyn RasterBackend, clip: ClipRect) {
    if r.w < 2 || r.h < 2 {
        return;
    }
    let (tl, br) = if raised { (WHITE, GRAY_DK) } else { (GRAY_DK, WHITE) };
    for i in 0..2 {
        backend.fill_rect(r.x + i, r.y + i, r.w - 2 * i, 1, tl, clip);
        backend.fill_rect(r.x + i, r.y + i, 1, r.h - 2 * i, tl, clip);
        backend.fill_rect(r.x + i, r.y + r.h - 1 - i, r.w - 2 * i, 1, br, clip);
        backend.fill_rect(r.x + r.w - 1 - i, r.y + i, 1, r.h - 2 * i, br, clip);
    }
}

fn gadget(r: Grect, ch: u16, backend: &dyn RasterBackend, clip: ClipRect) {
    gadget_sel(r, ch, false, backend, clip);
}

/// Draw text left-aligned within a band, one 8×16 glyph per byte.
fn draw_label(x: i16, y: i16, max_w: i16, s: &[u8], backend: &dyn RasterBackend, clip: ClipRect) {
    let cy = y + 1;
    let mut cx = x;
    for &b in s {
        if b == 0 || cx + 8 > x + max_w {
            break;
        }
        backend.draw_glyph(cx, cy, crate::vdi::font::glyph(b as u16), BLACK, WriteMode::Transparent, clip);
        cx += 8;
    }
}

/// Draw the full window frame for `w` (border + gadgets + work area), confined
/// to `clip`. Callers repainting the whole screen pass the full-device clip;
/// the damage-scoped composite ([`redraw_clip`]) passes the union of the moved
/// window's old and new border rects, so a settle-overlap repaint never clears
/// a work area (or overdraws a frame) outside the damage region.
fn draw_frame(w: &Window, istop: bool, backend: &dyn RasterBackend, clip: ClipRect) {
    let b = w.curr;
    let kind = w.kind;

    // Outer frame.
    rect_outline(b, backend, clip);

    let mut top = b.y + 1; // running top of the next band

    // Title row (name area + closer + fuller). EmuTOS: white base, the name
    // area carries a light dither on the *active* window (the classic
    // move-bar look), plain on the inactive one.
    if kind & (NAME | CLOSER | FULLER) != 0 {
        let bar = Grect { x: b.x + 1, y: top, w: b.w - 2, h: GAD - 1 };
        backend.fill_rect(bar.x, bar.y, bar.w, bar.h, WHITE, clip);

        let mut name_x = bar.x;
        let mut name_w = bar.w;
        if kind & CLOSER != 0 {
            let g = Grect { x: b.x, y: top - 1, w: GAD, h: GAD };
            gadget(g, G_CLOSE, backend, clip);
            name_x += GAD - 1;
            name_w -= GAD - 1;
        }
        if kind & FULLER != 0 {
            let g = Grect { x: b.x + b.w - GAD, y: top - 1, w: GAD, h: GAD };
            gadget(g, G_FULL, backend, clip);
            name_w -= GAD - 1;
        }
        // Active window: the classic move-bar dither behind the name.
        if istop {
            backend.fill_pattern(
                name_x,
                top,
                name_w,
                GAD - 1,
                PatternFill { fg: BLACK, bg: WHITE, pattern: &TITLE_STIPPLE, mode: WriteMode::Replace },
                clip,
            );
        }
        // Window name (centred when there is room), on a small white box so it
        // stays crisp over the dither (EmuTOS clears the name's glyph cells).
        let nlen = name_len(&w.name) as i16 * 8;
        let nx = name_x + ((name_w - nlen) / 2).max(2);
        if istop && nlen > 0 {
            backend.fill_rect((nx - 3).max(name_x), top, nlen + 6, GAD - 1, WHITE, clip);
        }
        draw_label(nx, top, name_w, &w.name, backend, clip);
        // separator under the title row
        backend.fill_rect(b.x, top + GAD - 1, b.w, 1, BLACK, clip);
        top += GAD - 1;
    }

    // Info line.
    if kind & INFO != 0 {
        let band = Grect { x: b.x + 1, y: top, w: b.w - 2, h: GAD - 1 };
        backend.fill_rect(band.x, band.y, band.w, band.h, GRAY_LT, clip);
        draw_label(band.x + 2, top, band.w - 2, &w.info, backend, clip);
        backend.fill_rect(b.x, top + GAD - 1, b.w, 1, BLACK, clip);
        top += GAD - 1;
    }

    // Work area (white client region). Its 1 px black frame is drawn by the
    // content owner *after* the content (the desktop's `draw_dir_listing`),
    // because the app clears the work area on every content redraw and would
    // otherwise overdraw a border painted here.
    let work = calc(WC_WORK, kind, b);
    backend.fill_rect(work.x, work.y, work.w, work.h, WHITE, clip);

    // Right scroll bar (arrows + track + thumb).
    draw_vslider(w, backend, clip);

    // Bottom scroll bar (arrows + track + thumb). Fill the whole bottom band
    // first: with a sizer but no horizontal scrollbar the band would otherwise
    // be left unpainted, showing the desktop through the bottom of the window.
    // The band fill lives here (not in `draw_hslider`) so the slider-only
    // redraw path never erases the sizer corner it does not repaint.
    if kind & (HGADGETS | SIZER) != 0 {
        let row_y = b.y + b.h - GAD;
        backend.fill_rect(b.x + 1, row_y, b.w - 2, GAD - 1, GRAY_LT, clip);
    }
    draw_hslider(w, backend, clip);

    // Sizer (bottom-right corner).
    if kind & SIZER != 0 {
        gadget(
            Grect { x: b.x + b.w - GAD, y: b.y + b.h - GAD, w: GAD, h: GAD },
            G_SIZE,
            backend,
            clip,
        );
    }
}

/// Draw the right (vertical) scroll bar — up/down arrows, the track, and a
/// thumb positioned + sized from `w.vslide` / `w.vslsize` (both per-mille).
/// Shared by `draw_frame` and `wind_set(WF_VSLIDE/WF_VSLSIZ)`; drawing only
/// the slider column leaves the work area (content) untouched.
fn draw_vslider(w: &Window, backend: &dyn RasterBackend, clip: ClipRect) {
    let b = w.curr;
    let kind = w.kind;
    if kind & (VGADGETS | SIZER) == 0 {
        return;
    }
    let col_x = b.x + b.w - GAD;
    // Track top: below the title + info bands.
    let mut sy = b.y + 1;
    if kind & (NAME | CLOSER | FULLER) != 0 {
        sy += GAD - 1;
    }
    if kind & INFO != 0 {
        sy += GAD - 1;
    }
    let mut send = b.y + b.h - 1;
    if kind & (HGADGETS | SIZER) != 0 {
        send -= GAD - 1;
    }
    if kind & UPARROW != 0 {
        gadget(Grect { x: col_x, y: sy, w: GAD, h: GAD }, G_UP, backend, clip);
        sy += GAD - 1;
    }
    if kind & DNARROW != 0 {
        gadget(Grect { x: col_x, y: send - GAD + 1, w: GAD, h: GAD }, G_DN, backend, clip);
        send -= GAD - 1;
    }
    if kind & VSLIDE != 0 && send > sy {
        let track_h = send - sy;
        backend.fill_rect(col_x, sy, GAD, track_h, GRAY_LT, clip);
        rect_outline(Grect { x: col_x, y: sy, w: GAD, h: track_h }, backend, clip);
        // Thumb: size = vslsize per-mille of the track (min one gadget cell),
        // position = vslide per-mille of the remaining travel. A stored −1
        // ("use default size") renders as one gadget cell — EmuTOS `w_bldvbar`
        // `if (w_vslsiz == -1) size = gl_hbox` (gemwmlib.c:665-667) — never a
        // zero/negative-height elevator; the stored/returned value stays −1.
        let min_thumb = GAD.min(track_h);
        let thumb_h = if w.vslsize == -1 {
            min_thumb
        } else {
            ((track_h as i32 * w.vslsize.clamp(1, 1000) as i32 / 1000) as i16).clamp(min_thumb, track_h)
        };
        let travel = track_h - thumb_h;
        let pos = w.vslide.clamp(0, 1000);
        let thumb_y = sy + (travel as i32 * pos as i32 / 1000) as i16;
        backend.fill_rect(col_x, thumb_y, GAD, thumb_h, GRAY_LT, clip);
        bevel(Grect { x: col_x, y: thumb_y, w: GAD, h: thumb_h }, true, backend, clip);
    }
}

/// Redraw just window `handle`'s vertical slider (no work-area clear), for
/// `wind_set(WF_VSLIDE/WF_VSLSIZ)`.
fn redraw_vslider(handle: i16) {
    if let (Some(backend), Some(w)) = (host().and_then(|h| h.raster()), win(handle)) {
        if w.open {
            draw_vslider(w, backend, ClipRect::full(backend.width(), backend.height()));
        }
    }
}

/// Draw the bottom (horizontal) scroll bar — left/right arrows, the track, and
/// a thumb positioned + sized from `w.hslide` / `w.hslsize` (both per-mille).
/// The mirror image of [`draw_vslider`] with the axes swapped (x↔y, w↔h): the
/// track runs along the bottom row instead of the right column, offset only by
/// the left border and the vertical scroll bar / sizer column on the right (no
/// title/info bands consume the horizontal axis). Shared by `draw_frame` and
/// `wind_set(WF_HSLIDE/WF_HSLSIZ)`; drawing only the slider row leaves the work
/// area (content) untouched. The bottom-band fill is done by the caller.
fn draw_hslider(w: &Window, backend: &dyn RasterBackend, clip: ClipRect) {
    let b = w.curr;
    let kind = w.kind;
    if kind & (HGADGETS | SIZER) == 0 {
        return;
    }
    let row_y = b.y + b.h - GAD;
    // Track left: just past the left border (no left-side bands on this axis).
    let mut sx = b.x + 1;
    let mut send = b.x + b.w - 1;
    if kind & (VGADGETS | SIZER) != 0 {
        send -= GAD - 1;
    }
    if kind & LFARROW != 0 {
        gadget(Grect { x: sx, y: row_y, w: GAD, h: GAD }, G_LF, backend, clip);
        sx += GAD - 1;
    }
    if kind & RTARROW != 0 {
        gadget(Grect { x: send - GAD + 1, y: row_y, w: GAD, h: GAD }, G_RT, backend, clip);
        send -= GAD - 1;
    }
    if kind & HSLIDE != 0 && send > sx {
        let track_w = send - sx;
        backend.fill_rect(sx, row_y, track_w, GAD, GRAY_LT, clip);
        rect_outline(Grect { x: sx, y: row_y, w: track_w, h: GAD }, backend, clip);
        // Thumb: size = hslsize per-mille of the track (min one gadget cell),
        // position = hslide per-mille of the remaining travel. A stored −1
        // ("use default size") renders as one gadget cell — EmuTOS `w_bldhbar`
        // `if (w_hslsiz == -1) size = gl_wbox` (gemwmlib.c:706-708) — never a
        // zero/negative-width elevator; the stored/returned value stays −1.
        let min_thumb = GAD.min(track_w);
        let thumb_w = if w.hslsize == -1 {
            min_thumb
        } else {
            ((track_w as i32 * w.hslsize.clamp(1, 1000) as i32 / 1000) as i16).clamp(min_thumb, track_w)
        };
        let travel = track_w - thumb_w;
        let pos = w.hslide.clamp(0, 1000);
        let thumb_x = sx + (travel as i32 * pos as i32 / 1000) as i16;
        backend.fill_rect(thumb_x, row_y, thumb_w, GAD, GRAY_LT, clip);
        bevel(Grect { x: thumb_x, y: row_y, w: thumb_w, h: GAD }, true, backend, clip);
    }
}

/// Redraw just window `handle`'s horizontal slider (no work-area clear), for
/// `wind_set(WF_HSLIDE/WF_HSLSIZ)`.
fn redraw_hslider(handle: i16) {
    if let (Some(backend), Some(w)) = (host().and_then(|h| h.raster()), win(handle)) {
        if w.open {
            draw_hslider(w, backend, ClipRect::full(backend.width(), backend.height()));
        }
    }
}

fn name_len(s: &[u8]) -> usize {
    s.iter().position(|&b| b == 0).unwrap_or(s.len())
}

fn copy_cstr(dst: &mut [u8], src: *const u8) {
    if src.is_null() {
        dst[0] = 0;
        return;
    }
    // SAFETY: trusted GEM pointer to a NUL-terminated string.
    unsafe {
        let mut i = 0;
        while i < dst.len() - 1 {
            let c = *src.add(i);
            dst[i] = c;
            if c == 0 {
                break;
            }
            i += 1;
        }
        dst[dst.len() - 1] = 0;
    }
}

/// Copy the NUL-terminated string in `src` (up to its terminator) out to the
/// caller's `dst` buffer, including the terminating NUL. `src` is one of the
/// window's own bounded name/info buffers (≤ `NAME_CAP - 1` chars + NUL), so
/// the caller's buffer — which the WF_NAME/WF_INFO contract requires to be
/// large enough — is never overrun by our stored length.
fn copy_out_cstr(dst: *mut u8, src: &[u8]) {
    let len = name_len(src);
    // SAFETY: trusted GEM caller buffer, large enough per the WF_NAME/
    // WF_INFO contract; we write `len + 1` bytes bounded by `NAME_CAP`.
    unsafe {
        for (i, &b) in src.iter().take(len).enumerate() {
            *dst.add(i) = b;
        }
        *dst.add(len) = 0;
    }
}

/// Resolve the `WF_NAME`/`WF_INFO` string/buffer pointer for both calling
/// conventions used at our altitude (F7 ABI-width audit):
///
///  * **Native** (the desktop's `desk::wind_set_ptr`, `selftest_objc`): the
///    pointer travels in `addr_in[0]` with `AIN_LEN == 1`.
///  * **Classic AES** (gemlib `wind_set_str` / `wind_get`, both ILP32 and
///    LP64): the pointer is packed at `&int_in[2]` with `AIN_LEN == 0`, at
///    the caller's pointer width — 4 bytes ILP32, 8 bytes LP64.
///
/// `AIN_LEN` is the declared discriminator, so an LP64 desktop and an ILP32
/// gemlib app (QED) both resolve to the right slot.
fn wf_str_ptr(pb: &Aespb) -> u64 {
    if pb.ain_len() >= 1 {
        pb.addrin(0)
    } else {
        pb.intin_ptr(2)
    }
}

// ── dispatch entry points ───────────────────────────────────────────────

/// `wind_create` (100) — allocate a window of border `kind`, max extent
/// `int_in[1..4]`. Returns a handle (≥1) or a negative error.
pub fn wind_create(pb: &Aespb) {
    let kind = pb.intin(0) as u16;
    let full = Grect { x: pb.intin(1), y: pb.intin(2), w: pb.intin(3), h: pb.intin(4) };
    for (i, w) in slots().iter_mut().enumerate() {
        if !w.in_use {
            *w = Window::EMPTY;
            w.in_use = true;
            w.kind = kind;
            w.full = full;
            w.curr = full;
            pb.set_ret((i + 1) as i16);
            return;
        }
    }
    pb.set_ret(-1); // no free handle
}

/// `wind_open` (101) — display window `int_in[0]` at rect `int_in[1..4]`,
/// on top of the stack. Owned by the calling process.
pub fn wind_open(pb: &Aespb) {
    let handle = pb.intin(0);
    let r = Grect { x: pb.intin(1), y: pb.intin(2), w: pb.intin(3), h: pb.intin(4) };
    if let Some(w) = win(handle) {
        w.prev = w.curr;
        w.curr = r;
        w.open = true;
        w.owner = super::appl::current_pid();
        w.z = bump_z();
        // The new window is topmost and covers exactly its own rect `r`; nothing
        // outside `r` changes. Repaint only `r` (background + frames, this window
        // on top) rather than the whole screen — the open/close full-screen
        // flicker fix, mirroring the move/size damage-scope. This window's content
        // follows via `send_redraw` (WM_REDRAW).
        redraw_clip(r.x, r.y, r.w, r.h);
        send_redraw(handle, calc(WC_WORK, w.kind, r));
        pb.set_ret(1);
    } else {
        pb.set_ret(0);
    }
}

/// `wind_close` (102) — hide window `int_in[0]` (the handle stays valid).
/// Repaints the desktop + remaining windows (no save/restore needed).
pub fn wind_close(pb: &Aespb) {
    let handle = pb.intin(0);
    if let Some(w) = win(handle) {
        w.open = false;
        // Only the closed window's rect is now vacated; repaint just that region
        // (background + the frames of the windows below, which `compose` draws in
        // z-order) instead of the whole screen — the open/close flicker fix. The
        // exposed lower windows' content is re-laid by the desktop owner's
        // `damage_redraw` after this returns.
        let r = w.curr;
        redraw_clip(r.x, r.y, r.w, r.h);
        pb.set_ret(1);
    } else {
        pb.set_ret(0);
    }
}

/// `wind_delete` (103) — free the handle `int_in[0]`.
pub fn wind_delete(pb: &Aespb) {
    let handle = pb.intin(0);
    if let Some(w) = win(handle) {
        w.in_use = false;
        w.open = false;
        pb.set_ret(1);
    } else {
        pb.set_ret(0);
    }
}

/// Write a single scalar to `int_out[1]` with `ret = 1` — the common
/// `wind_get` scalar-field return (WF_KIND / WF_TOP / the slider fields).
fn scalar_out(pb: &Aespb, v: i16) {
    pb.set_intout(1, v);
    pb.set_ret(1);
}

/// `wind_get` (104) — read a `WF_*` field of window `int_in[0]`.
pub fn wind_get(pb: &Aespb) {
    let handle = pb.intin(0);
    let field = pb.intin(1);

    // Rectangle-list iteration is computed separately from the slot fields.
    if field == WF_FIRSTXYWH || field == WF_NEXTXYWH {
        if field == WF_FIRSTXYWH {
            compute_rlist(handle);
        }
        let r = rlist_next();
        pb.set_intout(1, r.x);
        pb.set_intout(2, r.y);
        pb.set_intout(3, r.w);
        pb.set_intout(4, r.h);
        pb.set_ret(1);
        return;
    }

    // WF_TOP is answered from the global stack, independent of `handle` —
    // in particular wind_get(0, WF_TOP) (a desktop-owner query) must return
    // the top window handle, not fall through to the handle-0 screen rect.
    if field == WF_TOP {
        pb.set_intout(1, get_top());
        pb.set_ret(1);
        return;
    }

    // WF_SCREEN (17): the menu/alert bb_save buffer address + length. Like
    // WF_TOP it needs no valid handle (EmuTOS gemwmlib.c:1517-1520). EmuTOS
    // reports a single *persistent* global menu/alert save buffer
    // (gl_pmpmlo/gl_mlen). NewTOS's memory-MFDB bb_save (#51) instead
    // allocates a *transient* per-alert buffer from the heap and frees it on
    // close (tos_api::vdi::bb_save), so there is no stable address/length to
    // hand back. Report a null/zero-length buffer honestly (ret=1) rather
    // than exposing a freed transient pointer; deferred until/unless a
    // program is found that relies on a persistent buffer here.
    if field == WF_SCREEN {
        for i in 1..=4 {
            pb.set_intout(i, 0); // address (hi/lo) + length (hi/lo)
        }
        pb.set_ret(1);
        return;
    }

    let r = match win(handle) {
        Some(w) => match field {
            WF_WXYWH => calc(WC_WORK, w.kind, w.curr),
            WF_CXYWH => w.curr,
            WF_PXYWH => w.prev,
            WF_FXYWH => w.full,
            WF_KIND => {
                pb.set_intout(1, w.kind as i16);
                pb.set_ret(1);
                return;
            }
            // Slider position/size scalars → int_out[1] (EmuTOS wm_get,
            // gemwmlib.c:1542-1551). WF_HSLSIZ/WF_VSLSIZ round-trip the −1
            // "use default size" marker unchanged.
            WF_HSLIDE => return scalar_out(pb, w.hslide),
            WF_VSLIDE => return scalar_out(pb, w.vslide),
            WF_HSLSIZ => return scalar_out(pb, w.hslsize),
            WF_VSLSIZ => return scalar_out(pb, w.vslsize),
            // WF_NAME/WF_INFO get: copy the stored title/info out to the
            // caller's buffer (pointer at `&int_in[2]`, caller width —
            // gemlib `wind_get`, `a_wind_get.c:344`; the native `addr_in[0]`
            // convention is honoured too via `wf_str_ptr`). Per the gemlib
            // contract, an empty title returns FALSE and leaves the caller's
            // buffer untouched.
            WF_NAME | WF_INFO => {
                let src = if field == WF_NAME { w.name } else { w.info };
                if name_len(&src) == 0 {
                    pb.set_ret(0);
                    return;
                }
                let dst = wf_str_ptr(pb) as *mut u8;
                if dst.is_null() {
                    pb.set_ret(0);
                    return;
                }
                copy_out_cstr(dst, &src);
                pb.set_ret(1);
                return;
            }
            // Any other field on a valid window (COLOR/DCOLOR/OWNER/… — not
            // implemented at this altitude) → FALSE (EmuTOS default).
            _ => {
                pb.set_ret(0);
                return;
            }
        },
        // Desktop (handle 0): the WORK area sits *under* the menu bar (EmuTOS
        // gl_rfull); CURR/PREV/FULL are the whole screen (gl_rscreen).
        None if handle == 0 => match field {
            WF_WXYWH => desktop_work(),
            WF_CXYWH | WF_PXYWH | WF_FXYWH => screen_rect(),
            _ => {
                pb.set_ret(0);
                return;
            }
        },
        None => {
            pb.set_ret(0);
            return;
        }
    };
    pb.set_intout(1, r.x);
    pb.set_intout(2, r.y);
    pb.set_intout(3, r.w);
    pb.set_intout(4, r.h);
    pb.set_ret(1);
}

/// `wind_set` (105) — set a `WF_*` field of window `int_in[0]`.
pub fn wind_set(pb: &Aespb) {
    let handle = pb.intin(0);
    let field = pb.intin(1);
    if win(handle).is_none() {
        pb.set_ret(0);
        return;
    }
    match field {
        WF_NAME => {
            // The title pointer arrives two ways (see `wf_str_ptr`): the
            // desktop's native `addr_in[0]` (AIN_LEN=1), or the classic-AES
            // `&int_in[2]` packing at the caller's pointer width (gemlib
            // `wind_set_str`, AIN_LEN=0) — the latter is how ILP32 apps
            // (QED) pass it. Reading `addr_in[0]` unconditionally handed an
            // ILP32 caller an empty slot → garbage title (F7 audit).
            let title = wf_str_ptr(pb) as *const u8;
            if let Some(w) = win(handle) {
                copy_cstr(&mut w.name, title);
            }
            redraw_open(handle);
        }
        WF_INFO => {
            let info = wf_str_ptr(pb) as *const u8;
            if let Some(w) = win(handle) {
                copy_cstr(&mut w.info, info);
            }
            redraw_open(handle);
        }
        WF_CXYWH => {
            let r = Grect { x: pb.intin(2), y: pb.intin(3), w: pb.intin(4), h: pb.intin(5) };
            let (kind, prev) = if let Some(w) = win(handle) {
                w.prev = w.curr;
                w.curr = r;
                (w.kind, w.prev)
            } else {
                (0, r)
            };
            // Repaint only the union of the old and new border rects — the
            // programmatic move/size path (desktop WM_FULLED, app `wind_set`) —
            // not the whole screen. The frame composite (background + frames) is
            // damage-scoped here; this window's content follows via `send_redraw`
            // (WM_REDRAW). Lower windows' content is re-laid by the owner's
            // damage repaint when it drives this path (`desk::redraw`).
            let d = union_grect(prev, r);
            redraw_clip(d.x, d.y, d.w, d.h);
            send_redraw(handle, calc(WC_WORK, kind, r));
        }
        WF_TOP => top_window(handle),
        // Slider positions clamp to 1..1000; sizes clamp to 1..1000 but let −1
        // ("use default size") pass through (EmuTOS wm_set, gemwmlib.c:1643-1656).
        WF_VSLIDE => {
            if let Some(w) = win(handle) {
                w.vslide = pb.intin(2).clamp(1, 1000);
            }
            redraw_vslider(handle);
        }
        WF_VSLSIZ => {
            if let Some(w) = win(handle) {
                w.vslsize = slider_size(pb.intin(2));
            }
            redraw_vslider(handle);
        }
        WF_HSLIDE => {
            if let Some(w) = win(handle) {
                w.hslide = pb.intin(2).clamp(1, 1000);
            }
            redraw_hslider(handle);
        }
        WF_HSLSIZ => {
            if let Some(w) = win(handle) {
                w.hslsize = slider_size(pb.intin(2));
            }
            redraw_hslider(handle);
        }
        // Unknown or unimplemented field → FALSE (EmuTOS wm_set default:
        // gemwmlib.c:1736). NewTOS does not implement WF_NEWDESK/WF_COLOR/
        // WF_DCOLOR at this altitude, so they honestly report FALSE rather
        // than faking success.
        _ => {
            pb.set_ret(0);
            return;
        }
    }
    pb.set_ret(1);
}

/// Validate a slider *size* (`WF_HSLSIZ`/`WF_VSLSIZ`): clamp to 1..1000, but
/// pass −1 ("use default size") through unclamped (EmuTOS gemwmlib.c:1643-1648).
fn slider_size(v: i16) -> i16 {
    if v == -1 {
        -1
    } else {
        v.clamp(1, 1000)
    }
}

/// Redraw just `handle`'s frame if it is open (for name/info changes).
fn redraw_open(handle: i16) {
    if let (Some(backend), Some(w)) = (host().and_then(|h| h.raster()), win(handle)) {
        if w.open {
            draw_frame(w, handle == get_top(), backend, ClipRect::full(backend.width(), backend.height()));
        }
    }
}

/// `wind_find` (106) — handle of the window under `(int_in[0], int_in[1])`;
/// 0 = desktop.
pub fn wind_find(pb: &Aespb) {
    pb.set_ret(find_handle(pb.intin(0), pb.intin(1)));
}

/// `wind_update` (107) — screen-lock / mouse-ownership begin/end. Batch 1
/// has no rectangle list to lock, so this is a successful no-op.
pub fn wind_update(pb: &Aespb) {
    pb.set_ret(1);
}

/// `wind_calc` (108) — convert work↔border rect for `kind`.
pub fn wind_calc(pb: &Aespb) {
    let wtype = pb.intin(0);
    let kind = pb.intin(1) as u16;
    let r = Grect { x: pb.intin(2), y: pb.intin(3), w: pb.intin(4), h: pb.intin(5) };
    let o = calc(wtype, kind, r);
    pb.set_intout(1, o.x);
    pb.set_intout(2, o.y);
    pb.set_intout(3, o.w);
    pb.set_intout(4, o.h);
    pb.set_ret(1);
}

/// `wind_new` (109) — close + delete every window, reset the manager.
pub fn wind_new(pb: &Aespb) {
    for w in slots().iter_mut() {
        *w = Window::EMPTY;
    }
    // SAFETY: single-CPU AES.
    unsafe { *TABLE.next_z.get() = 1 };
    if let Some(backend) = host().and_then(|h| h.raster()) {
        redraw_all(backend);
    }
    pb.set_ret(1);
}

fn inside(r: Grect, x: i16, y: i16) -> bool {
    x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h
}

/// Repaint the desktop background + every open window. The Form Manager's
/// `form_dial(FMD_FINISH)` calls this to restore the screen under a
/// just-closed modal dialog (NewTOS has no per-dialog save/restore).
pub fn redraw() {
    if let Some(backend) = host().and_then(|h| h.raster()) {
        redraw_all(backend);
    }
}

fn screen_rect() -> Grect {
    if let Some(backend) = host().and_then(|h| h.raster()) {
        Grect { x: 0, y: 0, w: backend.width() as i16, h: backend.height() as i16 }
    } else {
        Grect { x: 0, y: 0, w: 640, h: 480 }
    }
}

/// The desktop pseudo-window's work rectangle: the full screen minus the menu
/// bar at the top (EmuTOS `gl_rscreen` minus the menu-bar owner rect). When no
/// menu bar is installed it is the whole screen. Seeds [`compute_rlist`]'s
/// [`DESKWH`] rectangle list.
fn desktop_work() -> Grect {
    let full = screen_rect();
    let top = super::menu::desk_top();
    Grect { x: full.x, y: full.y + top, w: full.w, h: full.h - top }
}

// ── Batch 2: interactive frame + WM_* messages ──────────────────────────

// WM_* message ids (aesdefs.h:229-239).
const WM_TOPPED: i16 = 21;
const WM_CLOSED: i16 = 22;
const WM_FULLED: i16 = 23;
const WM_ARROWED: i16 = 24;
const WM_SIZED: i16 = 27;
const WM_MOVED: i16 = 28;

// WM_ARROWED actions (aesdefs.h:246-253).
const WA_UPLINE: i16 = 2;
const WA_DNLINE: i16 = 3;
const WA_LFLINE: i16 = 6;
const WA_RTLINE: i16 = 7;

/// Which frame component a point falls in.
#[derive(PartialEq, Clone, Copy)]
enum Gadget {
    None,
    Closer,
    Fuller,
    Mover,
    Sizer,
    Up,
    Dn,
    Lf,
    Rt,
}

/// Hit-test a point against `w`'s frame gadgets (mirrors `draw_frame`'s
/// layout).
fn gadget_at(w: &Window, mx: i16, my: i16) -> Gadget {
    let b = w.curr;
    let kind = w.kind;
    if !inside(b, mx, my) {
        return Gadget::None;
    }
    let has_title = kind & (NAME | CLOSER | FULLER) != 0;

    // Title row.
    if has_title && my < b.y + GAD {
        if kind & CLOSER != 0 && mx < b.x + GAD {
            return Gadget::Closer;
        }
        if kind & FULLER != 0 && mx >= b.x + b.w - GAD {
            return Gadget::Fuller;
        }
        if kind & MOVER != 0 {
            return Gadget::Mover;
        }
        return Gadget::None;
    }
    // Sizer corner.
    if kind & SIZER != 0 && mx >= b.x + b.w - GAD && my >= b.y + b.h - GAD {
        return Gadget::Sizer;
    }
    // Right scroll-bar arrows.
    if kind & (VGADGETS | SIZER) != 0 && mx >= b.x + b.w - GAD {
        let mut top = b.y + 1;
        if has_title {
            top += GAD - 1;
        }
        if kind & INFO != 0 {
            top += GAD - 1;
        }
        let mut bot = b.y + b.h - 1;
        if kind & (HGADGETS | SIZER) != 0 {
            bot -= GAD - 1;
        }
        if kind & UPARROW != 0 && my < top + GAD {
            return Gadget::Up;
        }
        if kind & DNARROW != 0 && my >= bot - GAD + 1 {
            return Gadget::Dn;
        }
    }
    // Bottom scroll-bar arrows.
    if kind & (HGADGETS | SIZER) != 0 && my >= b.y + b.h - GAD {
        let left = b.x + 1;
        let mut right = b.x + b.w - 1;
        if kind & (VGADGETS | SIZER) != 0 {
            right -= GAD - 1;
        }
        if kind & LFARROW != 0 && mx < left + GAD {
            return Gadget::Lf;
        }
        if kind & RTARROW != 0 && mx >= right - GAD {
            return Gadget::Rt;
        }
    }
    Gadget::None
}

/// The screen rectangle of a title-row gadget (closer/fuller), for the
/// `watchbox` feedback.
fn gad_rect(w: &Window, g: Gadget) -> Grect {
    let b = w.curr;
    match g {
        Gadget::Closer => Grect { x: b.x, y: b.y, w: GAD, h: GAD },
        Gadget::Fuller => Grect { x: b.x + b.w - GAD, y: b.y, w: GAD, h: GAD },
        _ => Grect::default(),
    }
}

/// Track the button over a closer/fuller gadget: invert it while the mouse
/// is inside, restore on leave; return whether it was released inside
/// (EmuTOS `gr_watchbox`).
fn watchbox(r: Grect, ch: u16, backend: &dyn RasterBackend) -> bool {
    let Some(h) = host() else { return false };
    let clip = ClipRect::full(backend.width(), backend.height());
    let mut was_in = true;
    gadget_sel(r, ch, true, backend, clip);
    loop {
        let (mx, my) = crate::vdi::mouse_position();
        let now_in = inside(r, mx, my);
        if now_in != was_in {
            gadget_sel(r, ch, now_in, backend, clip);
            was_in = now_in;
        }
        if crate::vdi::mouse_buttons() & 0x01 == 0 {
            break;
        }
        h.pump_input();
    }
    gadget_sel(r, ch, false, backend, clip); // restore normal
    was_in
}

/// Live-drag the whole frame following the mouse until release; returns
/// the final border rect (the frame is left drawn there). Stands in for
/// EmuTOS `gr_dragbox` (outline drag) at our altitude.
fn drag_move(w: &mut Window, backend: &dyn RasterBackend) -> Grect {
    let Some(h) = host() else { return w.curr };
    let full = ClipRect::full(backend.width(), backend.height());
    let (sx, sy) = crate::vdi::mouse_position();
    let orig = w.curr;
    let sw = backend.width() as i16;
    let sh = backend.height() as i16;
    let mut last = orig;
    loop {
        let (mx, my) = crate::vdi::mouse_position();
        // Bound the window's top to the desktop work area — below the menu bar
        // (`menu::desk_top`, 0 when none is installed). A drag must never push
        // the window over the menu bar, which the desktop paints once and never
        // restores (EmuTOS bounds window moves to `gl_rfull` = screen minus the
        // menu bar). The bottom/left/right still reach the screen edges.
        // `.max(..)` guards the clamp: a window wider/taller than the available
        // area would make `max < min` and panic `i16::clamp` (fatal in no_std).
        let top = super::menu::desk_top();
        let nx = (orig.x + mx - sx).clamp(0, (sw - orig.w).max(0));
        let ny = (orig.y + my - sy).clamp(top, (sh - orig.h).max(top));
        let nr = Grect { x: nx, y: ny, w: orig.w, h: orig.h };
        if nr != last {
            // Solid drag (modern HW — no rubber-band outline): blit the
            // rendered window (frame + content) to the new position via a
            // fast screen-to-screen copy, then repaint the vacated L-region
            // with the desktop background. The whole window follows the mouse
            // over the intact background — no white trail, no re-render. Hide
            // the cursor around the blit so its sprite isn't duplicated into
            // the moved region.
            crate::vdi::cursor_hide(backend);
            backend.copy_rect(
                Rect { x: last.x, y: last.y, w: last.w, h: last.h },
                nr.x,
                nr.y,
                3, // BitBlt replace
                full,
            );
            repaint_vacated(backend, last, nr);
            crate::vdi::cursor_show(backend);
            w.curr = nr;
            last = nr;
        }
        if crate::vdi::mouse_buttons() & 0x01 == 0 {
            break;
        }
        h.pump_input();
    }
    // Record the pre-move border rect as the window's previous rect so the owner
    // (the desktop) can compute the damage region (old ∪ new) for the
    // settle-overlap repaint on WM_MOVED. The interactive drag mutated `w.curr`
    // directly, bypassing the `wind_set(WF_CXYWH)` path that maintains `prev`.
    w.prev = orig;
    last
}

/// Repaint the part of `old` not covered by `new` (same size — a window
/// move) with the desktop background: the vacated L-region of a solid drag.
/// Up to two rectangles — a full-width horizontal strip for the vertical
/// motion, plus a vertical strip limited to the overlapping rows (so the
/// corner is not painted twice, and the still-covered part is never erased).
fn repaint_vacated(backend: &dyn RasterBackend, old: Grect, new: Grect) {
    let dx = new.x - old.x;
    let dy = new.y - old.y;
    if dy > 0 {
        paint_bg_clip(backend, rect_clip(old.x, old.y, old.w, dy));
    } else if dy < 0 {
        paint_bg_clip(backend, rect_clip(old.x, new.y + old.h, old.w, -dy));
    }
    let oy = old.y.max(new.y);
    let oh = (old.y + old.h).min(new.y + old.h) - oy;
    if oh > 0 {
        if dx > 0 {
            paint_bg_clip(backend, rect_clip(old.x, oy, dx, oh));
        } else if dx < 0 {
            paint_bg_clip(backend, rect_clip(new.x + old.w, oy, -dx, oh));
        }
    }
}

/// Live-resize the frame (bottom-right corner follows the mouse) until
/// release; returns the final border rect. Stands in for `gr_rubwind`.
fn drag_size(w: &mut Window, backend: &dyn RasterBackend) -> Grect {
    let Some(h) = host() else { return w.curr };
    let orig = w.curr;
    let min_w = 3 * GAD;
    let min_h = 3 * GAD;
    let sw = backend.width() as i16;
    let sh = backend.height() as i16;
    let mut last = orig;
    loop {
        let (mx, my) = crate::vdi::mouse_position();
        // `.max(min_*)` guards the clamp bounds (origin near the screen edge
        // could make `max < min`, panicking `i16::clamp` — fatal in no_std).
        let nw = (mx - orig.x + 1).clamp(min_w, (sw - orig.x).max(min_w));
        let nh = (my - orig.y + 1).clamp(min_h, (sh - orig.y).max(min_h));
        let nr = Grect { x: orig.x, y: orig.y, w: nw, h: nh };
        if nr != last {
            // Resize reflows the content, so (unlike a move) it cannot blit —
            // repaint the desktop background over the previous window rect,
            // then redraw the frame at the new size. No white trail; the work
            // area's content returns on release via the desktop's redraw.
            crate::vdi::cursor_hide(backend);
            paint_bg_clip(backend, rect_clip(orig.x, orig.y, last.w, last.h));
            w.curr = nr;
            draw_frame(w, true, backend, ClipRect::full(backend.width(), backend.height()));
            crate::vdi::cursor_show(backend);
            last = nr;
        }
        if crate::vdi::mouse_buttons() & 0x01 == 0 {
            break;
        }
        h.pump_input();
    }
    // Record the pre-resize border rect as the window's previous rect so the
    // owner (the desktop) can compute the damage region (old ∪ new) for the
    // settle-overlap repaint on WM_SIZED, exactly as a programmatic WF_CXYWH
    // set would. The interactive drag mutated `w.curr` directly, bypassing the
    // `wind_set(WF_CXYWH)` path that normally maintains `prev`.
    w.prev = orig;
    last
}

/// Block until the left button is released (used after edge-triggered
/// gadgets like topped/arrowed so one press fires once).
fn wait_release() {
    if let Some(h) = host() {
        while crate::vdi::mouse_buttons() & 0x01 != 0 {
            h.pump_input();
        }
    }
}

/// Handle of the open window under `(mx,my)`, **top-most first** (the highest
/// z among the windows whose border rect contains the point); 0 = desktop.
/// Mirrors EmuTOS `wm_find` = `ob_find(gl_wtree, 0, 2, x, y)`, which walks the
/// window tree strictly top-to-bottom (gemsuper.c:348-350, gemwmlib.c:1767).
fn find_handle(mx: i16, my: i16) -> i16 {
    let mut best = 0i16;
    let mut best_z = i16::MIN;
    for (i, w) in slots().iter().enumerate() {
        if w.in_use && w.open && inside(w.curr, mx, my) && w.z >= best_z {
            best_z = w.z;
            best = (i + 1) as i16;
        }
    }
    best
}

/// AES window-frame interception (the `evnt_multi` hook, like the Menu
/// Manager). On a button press in a window frame, handle the gadget and
/// write a `WM_*` message into `pbuff`; returns true if a message was
/// produced. A click in the work area (or desktop) returns false so the
/// app sees the raw button.
pub fn intercept(pbuff: u64) -> bool {
    if crate::vdi::mouse_buttons() & 0x01 == 0 {
        return false;
    }
    let (mx, my) = crate::vdi::mouse_position();
    let handle = find_handle(mx, my);
    if handle == 0 {
        return false;
    }
    let Some(backend) = host().and_then(|h| h.raster()) else {
        return false;
    };

    let mut msg = [0i16; 8];
    msg[1] = super::appl::current_pid();
    msg[3] = handle;

    // Click on a non-top window → bring it to the top and tell the owner.
    if handle != get_top() {
        msg[0] = WM_TOPPED;
        wait_release();
        top_window(handle);
        return emit(pbuff, &msg);
    }

    let w = match win(handle) {
        Some(w) => w,
        None => return false,
    };
    match gadget_at(w, mx, my) {
        Gadget::Closer => {
            if !watchbox(gad_rect(w, Gadget::Closer), G_CLOSE, backend) {
                return false;
            }
            msg[0] = WM_CLOSED;
        }
        Gadget::Fuller => {
            if !watchbox(gad_rect(w, Gadget::Fuller), G_FULL, backend) {
                return false;
            }
            msg[0] = WM_FULLED;
        }
        Gadget::Mover => {
            let r = drag_move(w, backend);
            msg[0] = WM_MOVED;
            msg[4] = r.x;
            msg[5] = r.y;
            msg[6] = r.w;
            msg[7] = r.h;
        }
        Gadget::Sizer => {
            let r = drag_size(w, backend);
            msg[0] = WM_SIZED;
            msg[4] = r.x;
            msg[5] = r.y;
            msg[6] = r.w;
            msg[7] = r.h;
        }
        Gadget::Up | Gadget::Dn | Gadget::Lf | Gadget::Rt => {
            msg[0] = WM_ARROWED;
            msg[4] = match gadget_at(w, mx, my) {
                Gadget::Up => WA_UPLINE,
                Gadget::Dn => WA_DNLINE,
                Gadget::Lf => WA_LFLINE,
                _ => WA_RTLINE,
            };
            wait_release();
        }
        Gadget::None => return false,
    }
    emit(pbuff, &msg)
}

fn emit(pbuff: u64, msg: &[i16; 8]) -> bool {
    if pbuff != 0 {
        // SAFETY: the message buffer is >= 8 WORDs (GEM contract).
        unsafe {
            let p = pbuff as *mut i16;
            for (i, &v) in msg.iter().enumerate() {
                p.add(i).write(v);
            }
        }
    }
    true
}

/// Test hook: name of the gadget under a point (for the headless logic
/// check in the selftest).
pub fn gadget_name_at(handle: i16, mx: i16, my: i16) -> &'static str {
    match win(handle) {
        Some(w) => match gadget_at(w, mx, my) {
            Gadget::None => "none",
            Gadget::Closer => "closer",
            Gadget::Fuller => "fuller",
            Gadget::Mover => "mover",
            Gadget::Sizer => "sizer",
            Gadget::Up => "up",
            Gadget::Dn => "dn",
            Gadget::Lf => "lf",
            Gadget::Rt => "rt",
        },
        None => "no-window",
    }
}
