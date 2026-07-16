//! AES Graphics Manager (`graf_*`, opcodes 70–79).
//!
//! Spec: [`docs/api/aes/window-graphics.md`]. Source: EmuTOS @ `1a7484ab`
//! `aes/gemgrlib.c` (`gr_*`), dispatch `aes/gemsuper.c`. Interactive
//! feedback helpers (rubber-band / drag / animate an outline box, track a
//! gadget) plus three queries (`graf_handle`, `graf_mouse`,
//! `graf_mkstate`). The box-feedback calls own the mouse and block until
//! the button is released.
//!
//! The interactive outline (rubber-band select, drag-box, move-box) is a
//! 1-pixel rectangle drawn in **XOR** mode: XOR is self-inverse, so
//! redrawing the same rectangle erases it and restores the exact pixels
//! underneath — over the (non-white) desktop pattern, window content and
//! desktop icons alike. Matches EmuTOS `gr_rubwind` / `gsx_xline`
//! (`MD_XOR`). (The earlier solid-white erase assumed a white background and
//! left a white "staircase" trail on the patterned desktop / windows.)

use super::object::{self, Object};
use super::Aespb;
use crate::host::host;
use crate::vdi::backend::{ClipRect, Pel, RasterBackend, WriteMode};

const WHITE: Pel = 0;
const BLACK: Pel = 1;

// graf_mouse special numbers.
const M_OFF: i16 = 256;
const M_ON: i16 = 257;
const M_SAVE: i16 = 258;
const M_RESTORE: i16 = 259;
const M_PREVIOUS: i16 = 260;

/// Draw/erase a 1-pixel outline rectangle in **XOR** mode. XOR is
/// self-inverse, so drawing the same rectangle a second time restores the exact
/// pixels underneath — the rubber-band / drag / move outline leaves no trail
/// over the desktop pattern, window content or icons (unlike a solid draw, which
/// would need the original pixels to erase and previously painted white).
///
/// The `_col` pen is ignored — `WriteMode::Xor` complements the destination
/// regardless of pen — but the argument is kept so the draw/erase call sites
/// read symmetrically (`BLACK` to show, `WHITE` to hide). The vertical edges
/// exclude the corner rows already covered by the horizontal edges, so no pixel
/// is XOR'd twice within one call (a double XOR would cancel it, leaving gaps).
fn outline(x: i16, y: i16, w: i16, h: i16, _col: Pel, backend: &dyn RasterBackend, clip: ClipRect) {
    if w <= 0 || h <= 0 {
        return;
    }
    const SOLID: u16 = 0xFFFF; // dash mask: every pixel on
    let (xr, yb) = (x + w - 1, y + h - 1);
    // Top + bottom edges (full width). A 1-row box has only the top edge.
    backend.draw_line(x, y, xr, y, BLACK, WriteMode::Xor, clip, SOLID);
    if h > 1 {
        backend.draw_line(x, yb, xr, yb, BLACK, WriteMode::Xor, clip, SOLID);
    }
    // Left + right edges, excluding the corner rows already drawn above so no
    // pixel is complemented twice. A 1-column box has only the left edge.
    if h > 2 {
        backend.draw_line(x, y + 1, x, yb - 1, BLACK, WriteMode::Xor, clip, SOLID);
        if w > 1 {
            backend.draw_line(xr, y + 1, xr, yb - 1, BLACK, WriteMode::Xor, clip, SOLID);
        }
    }
}

fn mouse_down() -> bool {
    crate::vdi::mouse_buttons() & 0x01 != 0
}

/// EmuTOS `mul_div_round` (util/miscasm.S): `mult1 * mult2 / divisor`, rounded
/// **away from zero** — the D-098 "slider creep" fix (gemgrlib.c:410). Computed
/// as `((m1·m2·2)/divisor ± 1) >> 1`, matching the m68k `divs.w`/`asr.w`
/// sequence; done in `i32` so `rel·1000` cannot overflow.
fn mul_div_round(mult1: i32, mult2: i32, divisor: i32) -> i32 {
    if divisor == 0 {
        return 0;
    }
    let n = mult1 * mult2 * 2 / divisor;
    if n < 0 {
        (n - 1) >> 1
    } else {
        (n + 1) >> 1
    }
}

/// `gr_clamp` (EmuTOS `gemgrlib.c:83`): the signed rubber-band extent from a
/// pinned `origin` to the live `cursor`, plus one, clamped by a **signed** `max`
/// to `min`. The `+1` makes the extent span both endpoints inclusively; the
/// clamp is a signed `max`, *not* a magnitude clamp. A positive `min` therefore
/// forces a down-right minimum-size box (a leftward drag is pulled back up to
/// `+min`); `SHRT_MIN` disables the clamp for free four-quadrant growth
/// (`w`/`h` go negative when dragging up/left).
fn gr_clamp(cursor: i16, origin: i16, min: i16) -> i16 {
    (cursor - origin + 1).max(min)
}

/// Normalise a pinned `origin` plus a signed `gr_clamp` extent
/// (`ext = cursor - origin + 1`) into a drawable top-left / positive-length
/// pair for [`outline`]: the far corner sits at `origin + ext - 1`, so the box
/// spans that corner and `origin` and grows toward the cursor in any quadrant.
/// Saturating arithmetic keeps a `SHRT_MIN`-derived extreme extent from
/// overflowing (`outline` needs a non-negative length).
fn draw_span(origin: i16, ext: i16) -> (i16, i16) {
    let far = origin.saturating_add(ext.saturating_sub(1));
    let lo = origin.min(far);
    let hi = origin.max(far);
    (lo, (hi - lo).saturating_add(1))
}

/// `graf_rubberbox` (70) — rubber-band a box with one corner pinned at `(x,y)`;
/// the opposite corner tracks the mouse until the button comes up
/// (spec: `window-graphics.md` §`graf_rubberbox (70)`).
///
/// `int_out[1]`/`int_out[2]` are the **signed** `gr_clamp` extents
/// `w = max(cursor_x - x + 1, minw)`, `h = max(cursor_y - y + 1, minh)` — the
/// `+1` and the *signed* `max` are load-bearing. A positive `minw`/`minh`
/// yields a positive, down-right-only minimum-size box; a caller wanting free
/// four-quadrant growth (negative `w`/`h` when dragging up/left) passes
/// `minw = minh = SHRT_MIN`, exactly as the desktop's rubber-band select does
/// (EmuTOS `deskact.c:631`). The live outline follows the same signed extent
/// (top-left/length via [`draw_span`], like EmuTOS `gr_rubwind`). `int_out[0]`=1.
pub fn graf_rubberbox(pb: &Aespb) {
    let ax = pb.intin(0);
    let ay = pb.intin(1);
    let minw = pb.intin(2);
    let minh = pb.intin(3);
    // Seed the extent from the current mouse position so the first drawn box is
    // the live rubber-band rect, not a degenerate `SHRT_MIN`-sized frame.
    let (mut w, mut h) = {
        let (mx, my) = crate::vdi::mouse_position();
        (gr_clamp(mx, ax, minw), gr_clamp(my, ay, minh))
    };
    if let (Some(host_h), Some(backend)) = (host(), host().and_then(|h| h.raster())) {
        let clip = ClipRect::full(backend.width(), backend.height());
        // Hide the cursor for the whole rubber-band: the XOR outline is the drag
        // feedback (its far corner tracks the mouse), and a shown cursor's
        // save-under would otherwise capture XOR outline pixels and strew single-
        // pixel artefacts along the drag path. EmuTOS `gr_rubwind` likewise
        // brackets the band with `gsx_moff`/`gsx_mon`.
        crate::vdi::cursor_hide(backend);
        let (mut bx, mut bw) = draw_span(ax, w);
        let (mut by, mut bh) = draw_span(ay, h);
        outline(bx, by, bw, bh, BLACK, backend, clip);
        loop {
            let (mx, my) = crate::vdi::mouse_position();
            let nw = gr_clamp(mx, ax, minw);
            let nh = gr_clamp(my, ay, minh);
            if nw != w || nh != h {
                outline(bx, by, bw, bh, WHITE, backend, clip);
                w = nw;
                h = nh;
                (bx, bw) = draw_span(ax, w);
                (by, bh) = draw_span(ay, h);
                outline(bx, by, bw, bh, BLACK, backend, clip);
            }
            if !mouse_down() {
                break;
            }
            host_h.pump_input();
        }
        outline(bx, by, bw, bh, WHITE, backend, clip);
        crate::vdi::cursor_show(backend);
    }
    pb.set_intout(1, w);
    pb.set_intout(2, h);
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
        // Hide the cursor for the whole drag — see the note in `graf_rubberbox`:
        // the XOR outline is the feedback, and a shown cursor's save-under would
        // capture XOR pixels and leave a dotted trail.
        crate::vdi::cursor_hide(backend);
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
        crate::vdi::cursor_show(backend);
    }
    pb.set_intout(1, lx);
    pb.set_intout(2, ly);
    pb.set_ret(1);
}

/// Quick N-step outline animation between two rects (movebox/grow/shrink).
/// Also used by the Form Manager's `form_dial` (FMD_GROW/FMD_SHRINK).
pub(crate) fn animate(from: (i16, i16, i16, i16), to: (i16, i16, i16, i16)) {
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
    pb.set_ret(if watchbox(tree, obj, instate, outstate) { 1 } else { 0 });
}

/// Track the button over object `obj`: show `instate` while the mouse is
/// inside its rectangle, `outstate` while outside, restoring on each edge;
/// return whether it was released inside (EmuTOS `gr_watchbox`). Shared with
/// the Form Manager's `form_button`.
pub(crate) fn watchbox(tree: *mut Object, obj: i16, instate: u16, outstate: u16) -> bool {
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
    was_in
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
        // `travel` = parent − child extent (EmuTOS `divis = c.g_h - t.g_h`);
        // `c0` = the elevator's start position relative to the parent.
        let (travel, c0) = if isvert {
            (p.ob_height - c.ob_height, c.ob_y)
        } else {
            (p.ob_width - c.ob_width, c.ob_x)
        };
        if let Some(h) = host() {
            let (prx, pry, _, _) = object::abs_rect(tree as *const Object, parent);
            // Grip offset: how far the mouse sat from the elevator's top-left
            // at grab time, held constant for the whole drag so the elevator
            // does not snap its leading edge to the cursor. EmuTOS `gr_dragbox`
            // takes it via `gr_clamp(sx+1, sy+1, 0, 0, &off)` → `off =
            // max(mouse − elev, 0)` (gemgrlib.c:253-260). `travel` can be
            // negative for a malformed slider; `.max(0)` keeps `clamp` sound.
            let (m0x, m0y) = crate::vdi::mouse_position();
            let grip = if isvert {
                (m0y - (pry + c0)).max(0)
            } else {
                (m0x - (prx + c0)).max(0)
            };
            let hi = travel.max(0);
            loop {
                let (mx, my) = crate::vdi::mouse_position();
                // New elevator position (relative to the parent) = mouse − grip
                // − parent origin, constrained to the track [0, travel].
                let rel = if isvert {
                    (my - grip - pry).clamp(0, hi)
                } else {
                    (mx - grip - prx).clamp(0, hi)
                };
                if isvert {
                    c.ob_y = rel;
                } else {
                    c.ob_x = rel;
                }
                if let Some(backend) = h.raster() {
                    object::draw_obj(tree as *const Object, parent, 8, backend);
                }
                // Position 0..1000, rounded away from zero — EmuTOS D-098
                // "slider creep" fix: `mul_div_round(divnd, 1000, divis)`
                // (gemgrlib.c:410), not a truncating divide.
                pos = if travel > 0 {
                    mul_div_round(rel as i32, 1000, travel as i32) as i16
                } else {
                    0
                };
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
/// plus the system-font cell metrics in `int_out[1..4]` (EmuTOS
/// gemsuper.c:287-293).
pub fn graf_handle(pb: &Aespb) {
    pb.set_ret(1); // gl_handle: the physical VDI workstation the AES draws through
    pb.set_intout(1, 8); // gl_wchar: system-font cell width
    pb.set_intout(2, 16); // gl_hchar: system-font cell height
    // gl_wbox / gl_hbox: the gadget box unit. For the 8×16 system font both
    // are 19 — gl_hbox = gl_hchar+3 = 19; gl_wbox = max(gl_wchar+4, gl_hbox·
    // aspect) = 19 (gemgraf.c:412-415, gemsuper.c:290-291). Shared with the
    // window manager's gadget cell so there is only one `19` in the tree.
    pb.set_intout(3, super::window::GAD); // gl_wbox
    pb.set_intout(4, super::window::GAD); // gl_hbox
}

/// `graf_mouse` (78) — set the mouse cursor: `int_in[0]` = a preset shape
/// (ARROW 0 … OUTLN_CROSS 7), `USER_DEF` (255) with an `MFORM` in
/// `addr_in[0]`, or `M_OFF`/`M_ON` to hide/show it.
pub fn graf_mouse(pb: &Aespb) {
    let number = pb.intin(0);
    if let Some(backend) = host().and_then(|h| h.raster()) {
        match number {
            M_OFF => crate::vdi::mouse::remove(backend),
            M_ON => {
                // Read the parked position *after* init so the first M_ON
                // shows the cursor at screen centre, not the power-up (0,0)
                // over the menu bar (would open an AES hover-menu at boot).
                let (x, y) = crate::vdi::mouse::position_init(backend);
                crate::vdi::mouse::display_at(x, y, backend);
            }
            USER_DEF => set_user_form(pb, backend),
            0..=7 => crate::vdi::mouse::set_preset(number, backend),
            M_SAVE | M_RESTORE | M_PREVIOUS => {
                // DEFERRED (see docs/STATUS.md "Blocked/deferred"): EmuTOS
                // gr_mouse M_SAVE stores the current cursor form in the process
                // descriptor, M_RESTORE/M_PREVIOUS reinstall it (gemgrlib.c:
                // 440-450). Faithful support needs a saved-form slot *and* a
                // current-form reader in the VDI cursor subsystem
                // (tos-api/src/vdi/mouse.rs) — outside these two files. The
                // cursor form is left unchanged (an honest no-op, never a
                // fabricated form). graf_mouse is void, so int_out[0] stays 1.
            }
            _ => {} // undefined shape number: leave the cursor unchanged
        }
    }
    pb.set_ret(1);
}

/// USER_DEF cursor form number (`aesdefs.h`).
const USER_DEF: i16 = 255;

/// Install a caller-supplied `MFORM` from `addr_in[0]` (EmuTOS `gsx_mfset`):
/// 37 native WORDs — hotspot x/y, planes, fg/bg colour, 16 mask rows, 16
/// data rows.
fn set_user_form(pb: &Aespb, backend: &dyn RasterBackend) {
    let p = pb.addrin(0);
    if p == 0 {
        return;
    }
    // SAFETY: GEM contract — addr_in[0] points at a 37-WORD MFORM.
    let w = p as *const i16;
    let word = |i: usize| unsafe { w.add(i).read() };
    let mut mask = [0u16; 16];
    let mut data = [0u16; 16];
    for i in 0..16 {
        mask[i] = word(5 + i) as u16;
        data[i] = word(21 + i) as u16;
    }
    crate::vdi::mouse::set_form(word(0), word(1), &mask, &data, backend);
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
