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
use crate::vdi::backend::{ClipRect, Pel, RasterBackend};
use core::cell::UnsafeCell;

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
const WF_PXYWH: i16 = 6;
const WF_FXYWH: i16 = 7;
const WF_TOP: i16 = 10;

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

const MAX_WINDOWS: usize = 8;
const NAME_CAP: usize = 80;

/// A rectangle (GEM `GRECT`).
#[derive(Clone, Copy, Default)]
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
    full: Grect, // maximum extent (WF_FXYWH)
    curr: Grect, // current border rect (WF_CXYWH)
    prev: Grect, // previous border rect (WF_PXYWH)
    name: [u8; NAME_CAP],
    info: [u8; NAME_CAP],
}

impl Window {
    const EMPTY: Window = Window {
        in_use: false,
        open: false,
        kind: 0,
        full: Grect { x: 0, y: 0, w: 0, h: 0 },
        curr: Grect { x: 0, y: 0, w: 0, h: 0 },
        prev: Grect { x: 0, y: 0, w: 0, h: 0 },
        name: [0; NAME_CAP],
        info: [0; NAME_CAP],
    };
}

/// The window table. Single boot CPU only → `UnsafeCell` with a hand-rolled
/// `Sync`, mirroring the cookie jar and AES process table.
struct WinTable {
    slots: UnsafeCell<[Window; MAX_WINDOWS]>,
    top: UnsafeCell<i16>, // handle of the top window, 0 = desktop
}
// SAFETY: only the single boot CPU touches the AES; no concurrent access.
unsafe impl Sync for WinTable {}

static TABLE: WinTable = WinTable {
    slots: UnsafeCell::new([Window::EMPTY; MAX_WINDOWS]),
    top: UnsafeCell::new(0),
};

#[allow(clippy::mut_from_ref)]
fn slots() -> &'static mut [Window; MAX_WINDOWS] {
    // SAFETY: single-CPU AES; exclusive logical owner.
    unsafe { &mut *TABLE.slots.get() }
}
fn set_top(h: i16) {
    // SAFETY: single-CPU AES.
    unsafe { *TABLE.top.get() = h };
}
fn get_top() -> i16 {
    // SAFETY: single-CPU AES.
    unsafe { *TABLE.top.get() }
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
/// arrows/sizer).
fn gadget(r: Grect, ch: u16, backend: &dyn RasterBackend, clip: ClipRect) {
    backend.fill_rect(r.x, r.y, r.w, r.h, WHITE, clip);
    rect_outline(r, backend, clip);
    let gx = r.x + (r.w - 8) / 2;
    let gy = r.y + (r.h - 16) / 2;
    backend.draw_glyph(gx, gy, crate::vdi::font::glyph(ch), BLACK, clip);
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
fn draw_frame(w: &Window, backend: &dyn RasterBackend) {
    let clip = ClipRect::full(backend.width(), backend.height());
    let b = w.curr;
    let kind = w.kind;

    // Outer frame.
    rect_outline(b, backend, clip);

    let mut top = b.y + 1; // running top of the next band

    // Title row (name bar + closer + fuller).
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
        // Window name (centred when there is room).
        let nlen = name_len(&w.name) as i16 * 8;
        let nx = name_x + ((name_w - nlen) / 2).max(2);
        draw_label(nx, top, name_w, &w.name, backend, clip);
        // separator under the title row
        backend.fill_rect(b.x, top + GAD - 1, b.w, 1, BLACK, clip);
        top += GAD - 1;
    }

    // Info line.
    if kind & INFO != 0 {
        let band = Grect { x: b.x + 1, y: top, w: b.w - 2, h: GAD - 1 };
        backend.fill_rect(band.x, band.y, band.w, band.h, WHITE, clip);
        draw_label(band.x + 2, top, band.w - 2, &w.info, backend, clip);
        backend.fill_rect(b.x, top + GAD - 1, b.w, 1, BLACK, clip);
        top += GAD - 1;
    }

    // Work area (white client region).
    let work = calc(WC_WORK, kind, b);
    backend.fill_rect(work.x, work.y, work.w, work.h, WHITE, clip);

    // Right scroll bar.
    if kind & (VGADGETS | SIZER) != 0 {
        let col_x = b.x + b.w - GAD;
        let mut sy = top;
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
            // slider track (light) + a thumb at the top (Batch-1 default)
            backend.fill_rect(col_x, sy, GAD, send - sy, WHITE, clip);
            rect_outline(Grect { x: col_x, y: sy, w: GAD, h: send - sy }, backend, clip);
            rect_outline(Grect { x: col_x, y: sy, w: GAD, h: GAD.min(send - sy) }, backend, clip);
        }
    }

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
            backend.fill_rect(sx, row_y, send - sx, GAD, WHITE, clip);
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

/// `wind_open` (101) — display window `int_in[0]` at rect `int_in[1..4]`.
pub fn wind_open(pb: &Aespb) {
    let handle = pb.intin(0);
    let r = Grect { x: pb.intin(1), y: pb.intin(2), w: pb.intin(3), h: pb.intin(4) };
    if let Some(w) = win(handle) {
        w.prev = w.curr;
        w.curr = r;
        w.open = true;
        if let Some(backend) = host().and_then(|h| h.raster()) {
            draw_frame(w, backend);
        }
        set_top(handle);
        pb.set_ret(1);
    } else {
        pb.set_ret(0);
    }
}

/// `wind_close` (102) — hide window `int_in[0]` (the handle stays valid).
pub fn wind_close(pb: &Aespb) {
    let handle = pb.intin(0);
    if let Some(w) = win(handle) {
        w.open = false;
        // No save/restore yet: erase the frame to the desktop background.
        if let Some(backend) = host().and_then(|h| h.raster()) {
            let clip = ClipRect::full(backend.width(), backend.height());
            backend.fill_rect(w.curr.x, w.curr.y, w.curr.w + 2, w.curr.h + 2, WHITE, clip);
        }
        if get_top() == handle {
            set_top(0);
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
            WF_TOP => {
                pb.set_intout(1, get_top());
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
    let Some(w) = win(handle) else {
        pb.set_ret(0);
        return;
    };
    match field {
        WF_NAME => copy_cstr(&mut w.name, title),
        WF_INFO => copy_cstr(&mut w.info, title),
        WF_CXYWH => {
            w.prev = w.curr;
            w.curr = Grect { x: pb.intin(2), y: pb.intin(3), w: pb.intin(4), h: pb.intin(5) };
        }
        _ => {}
    }
    // Redraw the frame to reflect the change (name/size).
    if w.open {
        if let Some(backend) = host().and_then(|h| h.raster()) {
            draw_frame(w, backend);
        }
    }
    pb.set_ret(1);
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
    set_top(0);
    pb.set_ret(1);
}

fn inside(r: Grect, x: i16, y: i16) -> bool {
    x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h
}

fn screen_rect() -> Grect {
    if let Some(backend) = host().and_then(|h| h.raster()) {
        Grect { x: 0, y: 0, w: backend.width() as i16, h: backend.height() as i16 }
    } else {
        Grect { x: 0, y: 0, w: 640, h: 480 }
    }
}
