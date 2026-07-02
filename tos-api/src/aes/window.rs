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
use crate::vdi::backend::{ClipRect, Pel, PatternFill, RasterBackend};
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
const WF_VSLIDE: i16 = 9;
const WF_VSLSIZ: i16 = 16;
const WF_PXYWH: i16 = 6;
const WF_FXYWH: i16 = 7;
const WF_TOP: i16 = 10;
const WF_FIRSTXYWH: i16 = 11;
const WF_NEXTXYWH: i16 = 12;

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

/// Gadget cell size (`gl_hbox` = char height + 3; square gadgets).
const GAD: i16 = 19;

const WHITE: Pel = 0;
const BLACK: Pel = 1;
const GRAY_LT: Pel = 8; // 3D face (EmuTOS LWHITE)
const GRAY_DK: Pel = 9; // 3D shadow (EmuTOS LBLACK)

/// 50 % dither to grey out an inactive window's name bar (EmuTOS `IP_4PATT`).
const STIPPLE_50: [u16; 2] = [0xAAAA, 0x5555];

const MAX_WINDOWS: usize = 8;
const NAME_CAP: usize = 80;

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
    /// Vertical slider position (`WF_VSLIDE`, 0..1000 per-mille of travel)
    /// and thumb size (`WF_VSLSIZ`, 1..1000 per-mille of the track; 1000 =
    /// the whole track, i.e. nothing to scroll).
    vslide: i16,
    vslsize: i16,
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
        vslsize: 1000,
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

/// Raise `handle` to the top of the stack and repaint.
fn top_window(handle: i16) {
    if let Some(w) = win(handle) {
        w.z = bump_z();
    }
    if let Some(backend) = host().and_then(|h| h.raster()) {
        redraw_all(backend);
    }
}

/// Optional desktop-background painter. A GEM application that owns the
/// desktop (e.g. the NewTOS desktop) registers a pure drawing function
/// here; the window manager calls it to repaint the background underneath
/// the windows (the EmuTOS DESKWH/DROOT idea). `0` = none → plain white
/// erase, which is what the standalone AES demos want.
///
/// The painter runs **inside** `redraw_all`, i.e. inside the AES SVC
/// handler, so it must draw straight through the backend and issue **no
/// further AES trap** (no re-entrant `svc`).
static DESK_PAINTER: AtomicUsize = AtomicUsize::new(0);

/// Register the desktop-background painter (see [`DESK_PAINTER`]).
pub fn set_desktop_painter(f: fn(&dyn RasterBackend)) {
    DESK_PAINTER.store(f as usize, Ordering::Release);
}

/// Paint the desktop background: the registered painter if any, else a
/// plain white erase of the whole screen.
fn paint_background(backend: &dyn RasterBackend) {
    let p = DESK_PAINTER.load(Ordering::Acquire);
    if p != 0 {
        // SAFETY: only ever set via `set_desktop_painter` from a
        // `fn(&dyn RasterBackend)`; the value is that same pointer.
        let f: fn(&dyn RasterBackend) = unsafe { core::mem::transmute(p) };
        f(backend);
    } else {
        let clip = ClipRect::full(backend.width(), backend.height());
        backend.fill_rect(0, 0, backend.width() as i16, backend.height() as i16, WHITE, clip);
    }
}

/// Repaint the desktop and every open window back-to-front (painter's
/// algorithm — front windows overwrite the ones they cover). The simple
/// stand-in for per-window rectangle-list clipping when redrawing frames;
/// the rectangle list (`WF_FIRSTXYWH`) still drives what the *app* redraws.
fn redraw_all(backend: &dyn RasterBackend) {
    paint_background(backend);
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
        draw_frame(&slots()[pick], istop, backend);
    }
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
fn compute_rlist(handle: i16) {
    // SAFETY: single-CPU AES.
    let (rects, count, cursor) = unsafe {
        (&mut *RLIST.rects.get(), &mut *RLIST.count.get(), &mut *RLIST.cursor.get())
    };
    *count = 0;
    *cursor = 0;
    let (work, myz) = match win(handle) {
        Some(w) if w.open => (calc(WC_WORK, w.kind, w.curr), w.z),
        _ => return,
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
    backend.draw_glyph(gx, gy, crate::vdi::font::glyph(ch), BLACK, clip);
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
        backend.draw_glyph(cx, cy, crate::vdi::font::glyph(b as u16), BLACK, clip);
        cx += 8;
    }
}

/// Draw the full window frame for `w` (border + gadgets + work area).
fn draw_frame(w: &Window, istop: bool, backend: &dyn RasterBackend) {
    let clip = ClipRect::full(backend.width(), backend.height());
    let b = w.curr;
    let kind = w.kind;

    // Outer frame.
    rect_outline(b, backend, clip);

    let mut top = b.y + 1; // running top of the next band

    // Title row (name bar + closer + fuller).
    if kind & (NAME | CLOSER | FULLER) != 0 {
        let bar = Grect { x: b.x + 1, y: top, w: b.w - 2, h: GAD - 1 };
        backend.fill_rect(bar.x, bar.y, bar.w, bar.h, GRAY_LT, clip);

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
        // Window name (centred when there is room).
        let nlen = name_len(&w.name) as i16 * 8;
        let nx = name_x + ((name_w - nlen) / 2).max(2);
        draw_label(nx, top, name_w, &w.name, backend, clip);
        // Non-topped window: grey out the name bar (EmuTOS marks W_NAME
        // DISABLED for the inactive window, drawing a 50 % white stipple
        // over it). Only the name bar carries the active/inactive cue.
        if !istop {
            backend.fill_pattern(
                bar.x,
                bar.y,
                bar.w,
                bar.h,
                PatternFill { fg: WHITE, bg: WHITE, pattern: &STIPPLE_50, opaque: false },
                clip,
            );
        }
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

    // Work area (white client region).
    let work = calc(WC_WORK, kind, b);
    backend.fill_rect(work.x, work.y, work.w, work.h, WHITE, clip);

    // Right scroll bar (arrows + track + thumb).
    draw_vslider(w, backend);

    // Bottom scroll bar.
    if kind & (HGADGETS | SIZER) != 0 {
        let row_y = b.y + b.h - GAD;
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
            backend.fill_rect(sx, row_y, send - sx, GAD, GRAY_LT, clip);
            rect_outline(Grect { x: sx, y: row_y, w: send - sx, h: GAD }, backend, clip);
            rect_outline(Grect { x: sx, y: row_y, w: GAD.min(send - sx), h: GAD }, backend, clip);
        }
    }

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
fn draw_vslider(w: &Window, backend: &dyn RasterBackend) {
    let b = w.curr;
    let kind = w.kind;
    if kind & (VGADGETS | SIZER) == 0 {
        return;
    }
    let clip = ClipRect::full(backend.width(), backend.height());
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
        // position = vslide per-mille of the remaining travel.
        let size = w.vslsize.clamp(1, 1000);
        let thumb_h = ((track_h as i32 * size as i32 / 1000) as i16).clamp(GAD.min(track_h), track_h);
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
            draw_vslider(w, backend);
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
        if let Some(backend) = host().and_then(|h| h.raster()) {
            redraw_all(backend);
        }
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
        if let Some(backend) = host().and_then(|h| h.raster()) {
            redraw_all(backend);
        }
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

    // Desktop (handle 0) reports the full screen for the rect fields.
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
            _ => Grect::default(),
        },
        None if handle == 0 => screen_rect(),
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
    let title = pb.addrin(0) as *const u8;
    if win(handle).is_none() {
        pb.set_ret(0);
        return;
    }
    match field {
        WF_NAME => {
            if let Some(w) = win(handle) {
                copy_cstr(&mut w.name, title);
            }
            redraw_open(handle);
        }
        WF_INFO => {
            if let Some(w) = win(handle) {
                copy_cstr(&mut w.info, title);
            }
            redraw_open(handle);
        }
        WF_CXYWH => {
            let r = Grect { x: pb.intin(2), y: pb.intin(3), w: pb.intin(4), h: pb.intin(5) };
            let kind = if let Some(w) = win(handle) {
                w.prev = w.curr;
                w.curr = r;
                w.kind
            } else {
                0
            };
            if let Some(backend) = host().and_then(|h| h.raster()) {
                redraw_all(backend);
            }
            send_redraw(handle, calc(WC_WORK, kind, r));
        }
        WF_TOP => top_window(handle),
        WF_VSLIDE => {
            if let Some(w) = win(handle) {
                w.vslide = pb.intin(2).clamp(0, 1000);
            }
            redraw_vslider(handle);
        }
        WF_VSLSIZ => {
            if let Some(w) = win(handle) {
                w.vslsize = pb.intin(2).clamp(1, 1000);
            }
            redraw_vslider(handle);
        }
        _ => {}
    }
    pb.set_ret(1);
}

/// Redraw just `handle`'s frame if it is open (for name/info changes).
fn redraw_open(handle: i16) {
    if let (Some(backend), Some(w)) = (host().and_then(|h| h.raster()), win(handle)) {
        if w.open {
            draw_frame(w, handle == get_top(), backend);
        }
    }
}

/// `wind_find` (106) — handle of the window under `(int_in[0], int_in[1])`;
/// 0 = desktop.
pub fn wind_find(pb: &Aespb) {
    let mx = pb.intin(0);
    let my = pb.intin(1);
    // Top-most first (Batch 1: the single top window, then any open one).
    let top = get_top();
    if let Some(w) = win(top) {
        if w.open && inside(w.curr, mx, my) {
            pb.set_ret(top);
            return;
        }
    }
    for (i, w) in slots().iter().enumerate() {
        if w.in_use && w.open && inside(w.curr, mx, my) {
            pb.set_ret((i + 1) as i16);
            return;
        }
    }
    pb.set_ret(0); // desktop
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
    let clip = ClipRect::full(backend.width(), backend.height());
    let (sx, sy) = crate::vdi::mouse_position();
    let orig = w.curr;
    let sw = backend.width() as i16;
    let sh = backend.height() as i16;
    let mut last = orig;
    loop {
        let (mx, my) = crate::vdi::mouse_position();
        // `.max(0)` guards the clamp: a window wider/taller than the screen
        // would make `max < min` and panic `i16::clamp` (fatal in no_std).
        let nx = (orig.x + mx - sx).clamp(0, (sw - orig.w).max(0));
        let ny = (orig.y + my - sy).clamp(0, (sh - orig.h).max(0));
        let nr = Grect { x: nx, y: ny, w: orig.w, h: orig.h };
        if nr != last {
            backend.fill_rect(last.x, last.y, last.w + 1, last.h + 1, WHITE, clip);
            w.curr = nr;
            draw_frame(w, true, backend);
            last = nr;
        }
        if crate::vdi::mouse_buttons() & 0x01 == 0 {
            break;
        }
        h.pump_input();
    }
    last
}

/// Live-resize the frame (bottom-right corner follows the mouse) until
/// release; returns the final border rect. Stands in for `gr_rubwind`.
fn drag_size(w: &mut Window, backend: &dyn RasterBackend) -> Grect {
    let Some(h) = host() else { return w.curr };
    let clip = ClipRect::full(backend.width(), backend.height());
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
            backend.fill_rect(orig.x, orig.y, last.w + 1, last.h + 1, WHITE, clip);
            w.curr = nr;
            draw_frame(w, true, backend);
            last = nr;
        }
        if crate::vdi::mouse_buttons() & 0x01 == 0 {
            break;
        }
        h.pump_input();
    }
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

/// Handle of the open window under `(mx,my)` (top-most first); 0 = desktop.
fn find_handle(mx: i16, my: i16) -> i16 {
    let top = get_top();
    if let Some(w) = win(top) {
        if w.open && inside(w.curr, mx, my) {
            return top;
        }
    }
    for (i, w) in slots().iter().enumerate() {
        if w.in_use && w.open && inside(w.curr, mx, my) {
            return (i + 1) as i16;
        }
    }
    0
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
