//! AES Graphics Manager (`graf_*`, opcodes 70–79).
//!
//! Spec: [`docs/api/aes/window-graphics.md`]. Source: EmuTOS @ `1a7484ab`
//! `aes/gemgrlib.c` (`gr_*`), dispatch `aes/gemsuper.c`. Interactive
//! feedback helpers (rubber-band / drag / animate an outline box, track a
//! gadget) plus three queries (`graf_handle`, `graf_mouse`,
//! `graf_mkstate`). The box-feedback calls own the mouse and block until
//! the button is released.
//!
//! At our altitude the outline is a solid 1-pixel rectangle redrawn as it
//! follows the mouse (erased to white between steps) rather than an XOR
//! band — correct on the desktop background; full save/restore arrives
//! with the window rectangle list (D4 Batch 3).

use super::object::{self, Object};
use super::Aespb;
use crate::host::host;
use crate::vdi::backend::{ClipRect, Pel, RasterBackend};

const WHITE: Pel = 0;
const BLACK: Pel = 1;

// graf_mouse special numbers.
const M_OFF: i16 = 256;
const M_ON: i16 = 257;

/// Draw (or erase) a 1-pixel outline rectangle.
fn outline(x: i16, y: i16, w: i16, h: i16, col: Pel, backend: &dyn RasterBackend, clip: ClipRect) {
    if w <= 0 || h <= 0 {
        return;
    }
    backend.fill_rect(x, y, w, 1, col, clip);
    backend.fill_rect(x, y + h - 1, w, 1, col, clip);
    backend.fill_rect(x, y, 1, h, col, clip);
    backend.fill_rect(x + w - 1, y, 1, h, col, clip);
}

fn mouse_down() -> bool {
    crate::vdi::mouse_buttons() & 0x01 != 0
}

/// `graf_rubberbox` (70) — rubber-band a box anchored at `(x,y)`; the
/// opposite corner follows the mouse (min size `minw`/`minh`). Returns the
/// final `w,h` in `int_out[1..2]`.
pub fn graf_rubberbox(pb: &Aespb) {
    let x = pb.intin(0);
    let y = pb.intin(1);
    let minw = pb.intin(2).max(1);
    let minh = pb.intin(3).max(1);
    let (mut lw, mut lh) = (minw, minh);
    if let (Some(h), Some(backend)) = (host(), host().and_then(|h| h.raster())) {
        let clip = ClipRect::full(backend.width(), backend.height());
        outline(x, y, lw, lh, BLACK, backend, clip);
        loop {
            let (mx, my) = crate::vdi::mouse_position();
            let nw = (mx - x + 1).max(minw);
            let nh = (my - y + 1).max(minh);
            if nw != lw || nh != lh {
                outline(x, y, lw, lh, WHITE, backend, clip);
                lw = nw;
                lh = nh;
                outline(x, y, lw, lh, BLACK, backend, clip);
            }
            if !mouse_down() {
                break;
            }
            h.pump_input();
        }
        outline(x, y, lw, lh, WHITE, backend, clip);
    }
    pb.set_intout(1, lw);
    pb.set_intout(2, lh);
    pb.set_ret(1);
}

/// `graf_dragbox` (71) — drag a fixed-size `w×h` box (starting at
/// `sx,sy`) with the mouse, clamped to the bound rect `int_in[4..7]`.
/// Returns the final `x,y` in `int_out[1..2]`.
pub fn graf_dragbox(pb: &Aespb) {
    let w = pb.intin(0);
    let h = pb.intin(1);
    let sx = pb.intin(2);
    let sy = pb.intin(3);
    let (bx, by, bw, bh) = (pb.intin(4), pb.intin(5), pb.intin(6), pb.intin(7));
    let (mut lx, mut ly) = (sx, sy);
    if let (Some(host_h), Some(backend)) = (host(), host().and_then(|h| h.raster())) {
        let clip = ClipRect::full(backend.width(), backend.height());
        let (smx, smy) = crate::vdi::mouse_position();
        outline(lx, ly, w, h, BLACK, backend, clip);
        loop {
            let (mx, my) = crate::vdi::mouse_position();
            // Guard the clamp bounds: a bound rect smaller than the box makes
            // `max < min`, which would panic `i16::clamp` (fatal in no_std).
            let nx = (sx + mx - smx).clamp(bx, (bx + bw - w).max(bx));
            let ny = (sy + my - smy).clamp(by, (by + bh - h).max(by));
            if nx != lx || ny != ly {
                outline(lx, ly, w, h, WHITE, backend, clip);
                lx = nx;
                ly = ny;
                outline(lx, ly, w, h, BLACK, backend, clip);
            }
            if !mouse_down() {
                break;
            }
            host_h.pump_input();
        }
        outline(lx, ly, w, h, WHITE, backend, clip);
    }
    pb.set_intout(1, lx);
    pb.set_intout(2, ly);
    pb.set_ret(1);
}

/// Quick N-step outline animation between two rects (movebox/grow/shrink).
fn animate(from: (i16, i16, i16, i16), to: (i16, i16, i16, i16)) {
    let Some(backend) = host().and_then(|h| h.raster()) else {
        return;
    };
    let clip = ClipRect::full(backend.width(), backend.height());
    const STEPS: i16 = 8;
    let lerp = |a: i16, b: i16, i: i16| a + (b - a) * i / STEPS;
    let mut prev: Option<(i16, i16, i16, i16)> = None;
    for i in 0..=STEPS {
        let r = (
            lerp(from.0, to.0, i),
            lerp(from.1, to.1, i),
            lerp(from.2, to.2, i),
            lerp(from.3, to.3, i),
        );
        if let Some(p) = prev {
            outline(p.0, p.1, p.2, p.3, WHITE, backend, clip);
        }
        outline(r.0, r.1, r.2, r.3, BLACK, backend, clip);
        // brief dwell so the animation is visible
        if let Some(h) = host() {
            let t = h.monotonic_millis() + 15;
            while h.monotonic_millis() < t {
                core::hint::spin_loop();
            }
        }
        prev = Some(r);
    }
    if let Some(p) = prev {
        outline(p.0, p.1, p.2, p.3, WHITE, backend, clip);
    }
}

/// `graf_movebox` (72) — animate an outline box of size `w×h` from
/// `(sx,sy)` to `(dx,dy)`.
pub fn graf_movebox(pb: &Aespb) {
    let w = pb.intin(0);
    let h = pb.intin(1);
    let sx = pb.intin(2);
    let sy = pb.intin(3);
    let dx = pb.intin(4);
    let dy = pb.intin(5);
    animate((sx, sy, w, h), (dx, dy, w, h));
    pb.set_ret(1);
}

/// `graf_growbox` (73) — zoom an outline from the small rect (`int_in
/// [0..3]`) out to the big rect (`int_in[4..7]`).
pub fn graf_growbox(pb: &Aespb) {
    let small = (pb.intin(0), pb.intin(1), pb.intin(2), pb.intin(3));
    let big = (pb.intin(4), pb.intin(5), pb.intin(6), pb.intin(7));
    animate(small, big);
    pb.set_ret(1);
}

/// `graf_shrinkbox` (74) — zoom an outline from the big rect down to the
/// small rect.
pub fn graf_shrinkbox(pb: &Aespb) {
    let small = (pb.intin(0), pb.intin(1), pb.intin(2), pb.intin(3));
    let big = (pb.intin(4), pb.intin(5), pb.intin(6), pb.intin(7));
    animate(big, small);
    pb.set_ret(1);
}

/// `graf_watchbox` (75) — track the button while it is held: object `obj`
/// in `addr_in[0]` shows `instate` while the mouse is inside its rect and
/// `outstate` while outside. Returns 1 if released inside, else 0.
pub fn graf_watchbox(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let obj = pb.intin(1);
    let instate = pb.intin(2) as u16;
    let outstate = pb.intin(3) as u16;
    if tree.is_null() || obj < 0 {
        pb.set_ret(0);
        return;
    }
    let mut was_in = true;
    if let Some(h) = host() {
        object::change(tree, obj, instate, true);
        loop {
            let (mx, my) = crate::vdi::mouse_position();
            let (rx, ry, rw, rh) = object::abs_rect(tree as *const Object, obj);
            let now_in = mx >= rx && mx < rx + rw && my >= ry && my < ry + rh;
            if now_in != was_in {
                object::change(tree, obj, if now_in { instate } else { outstate }, true);
                was_in = now_in;
            }
            if !mouse_down() {
                break;
            }
            h.pump_input();
        }
    }
    pb.set_ret(if was_in { 1 } else { 0 });
}

/// `graf_slidebox` (76) — slide child `obj` within `parent` along one axis
/// (`isvert`=`int_in[2]`); returns the position 0–1000.
pub fn graf_slidebox(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let parent = pb.intin(0);
    let obj = pb.intin(1);
    let isvert = pb.intin(2) != 0;
    if tree.is_null() || parent < 0 || obj < 0 {
        pb.set_ret(0);
        return;
    }
    let mut pos = 0i16;
    // SAFETY: valid object tree (GEM contract).
    unsafe {
        let p = &*tree.add(parent as usize);
        let c = &mut *tree.add(obj as usize);
        let (travel, csize, c0) = if isvert {
            (p.ob_height - c.ob_height, c.ob_height, c.ob_y)
        } else {
            (p.ob_width - c.ob_width, c.ob_width, c.ob_x)
        };
        let _ = (csize, c0);
        if let Some(h) = host() {
            let (prx, pry, _, _) = object::abs_rect(tree as *const Object, parent);
            loop {
                let (mx, my) = crate::vdi::mouse_position();
                // `travel` (parent − child extent) can be negative for a
                // malformed slider; `.max(0)` keeps `i16::clamp` from panicking.
                let hi = travel.max(0);
                let rel = if isvert { (my - pry).clamp(0, hi) } else { (mx - prx).clamp(0, hi) };
                if isvert {
                    c.ob_y = rel;
                } else {
                    c.ob_x = rel;
                }
                if let Some(backend) = h.raster() {
                    object::draw_obj(tree as *const Object, parent, 8, backend);
                }
                pos = if travel > 0 { (rel as i32 * 1000 / travel as i32) as i16 } else { 0 };
                if !mouse_down() {
                    break;
                }
                h.pump_input();
            }
        }
    }
    pb.set_intout(1, pos);
    pb.set_ret(1);
}

/// `graf_handle` (77) — the AES's VDI workstation handle (`int_out[0]`)
/// plus the system-font cell metrics in `int_out[1..4]`.
pub fn graf_handle(pb: &Aespb) {
    pb.set_ret(1); // physical-workstation handle
    pb.set_intout(1, 8); // wchar
    pb.set_intout(2, 16); // hchar
    pb.set_intout(3, 8); // wbox
    pb.set_intout(4, 16); // hbox
}

/// `graf_mouse` (78) — set the mouse cursor: `int_in[0]` = a preset shape
/// (kept as the arrow), or `M_OFF`/`M_ON` to hide/show it.
pub fn graf_mouse(pb: &Aespb) {
    let number = pb.intin(0);
    if let Some(backend) = host().and_then(|h| h.raster()) {
        match number {
            M_OFF => crate::vdi::mouse::remove(backend),
            M_ON => {
                let (x, y) = crate::vdi::mouse_position();
                crate::vdi::mouse::display_at(x, y, backend);
            }
            _ => {} // preset/user shapes: single arrow cursor for now
        }
    }
    pb.set_ret(1);
}

/// `graf_mkstate` (79) — poll the instantaneous mouse position
/// (`int_out[1..2]`), button mask (`int_out[3]`) and keyboard shift state
/// (`int_out[4]`). Does not wait.
pub fn graf_mkstate(pb: &Aespb) {
    let (mx, my) = crate::vdi::mouse_position();
    pb.set_intout(1, mx);
    pb.set_intout(2, my);
    pb.set_intout(3, crate::vdi::mouse_buttons() as i16);
    pb.set_intout(4, host().map(|h| h.keyboard_shift_state()).unwrap_or(0) as i16);
    pb.set_ret(1);
}
