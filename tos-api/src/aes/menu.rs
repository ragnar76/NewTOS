//! AES Menu Manager (`menu_*`, opcodes 30–34) — Batch 1.
//!
//! Spec: [`docs/api/aes/menu.md`]. A menu **is** an OBJECT tree: the root
//! (`G_IBOX`) holds the bar (`THEBAR`, object 1) whose children are the
//! titles (`G_TITLE`); each title's child is a drop-down `G_BOX` of items
//! (`G_STRING`). So `menu_bar` just draws the bar subtree, and the
//! item/title state calls (`icheck`/`ienable`/`tnormal`) flip one
//! `ob_state` bit and redraw via the shared `object::change` helper
//! (EmuTOS `do_chg`).
//!
//! **Batch-1 scope:** draw the bar + the four original-TOS state/text
//! calls. The interactive drop-down tracking (mouse over titles → open
//! the box → highlight item → post `MN_SELECTED`) is Batch 2.

use super::object::{self, Object};
use super::Aespb;
use crate::host::host;
use crate::vdi::backend::{ClipRect, Pel};

/// `THEBAR` — the menu-bar object (child of the root).
const THEBAR: i16 = 1;
const MAX_DEPTH: i16 = 16;
/// Menu-bar height in pixels (≈ char height + 3, the GEM `gl_hbox`).
const BAR_H: i16 = 19;

// ob_state bits.
const SELECTED: u16 = 0x0001;
const CHECKED: u16 = 0x0004;
const DISABLED: u16 = 0x0008;

const WHITE: Pel = 0;
const BLACK: Pel = 1;

/// `menu_bar` (30) — install (`int_in[0]`=1) or remove (0) the menu bar
/// `addr_in[0]` at the top of the screen.
pub fn menu_bar(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let show = pb.intin(0);
    if tree.is_null() {
        pb.set_ret(0);
        return;
    }
    let Some(backend) = host().and_then(|h| h.raster()) else {
        pb.set_ret(0);
        return;
    };
    let sw = backend.width() as i16;
    let clip = ClipRect::full(backend.width(), backend.height());

    if show != 0 {
        // The bar spans the full screen width.
        // SAFETY: valid menu tree; THEBAR is object 1.
        unsafe {
            let bar = &mut *tree.add(THEBAR as usize);
            bar.ob_width = sw - bar.ob_x;
        }
        object::draw_obj(tree as *const Object, THEBAR, MAX_DEPTH, backend);
        // Separator line under the bar.
        backend.fill_rect(0, BAR_H - 1, sw, 1, BLACK, clip);
    } else {
        // Erase the bar area.
        backend.fill_rect(0, 0, sw, BAR_H, WHITE, clip);
    }
    pb.set_ret(1);
}

/// `menu_icheck` (31) — add (`int_in[1]`=1) / remove the check mark on
/// item `int_in[0]`.
pub fn menu_icheck(pb: &Aespb) {
    do_chg(pb, pb.intin(0), CHECKED, pb.intin(1) != 0);
}

/// `menu_ienable` (32) — enable (`int_in[1]`=1) / disable item
/// `int_in[0]` (bit 15 = a special/title item, masked off).
pub fn menu_ienable(pb: &Aespb) {
    let item = pb.intin(0) & 0x7fff;
    do_chg(pb, item, DISABLED, pb.intin(1) == 0); // disable when enable==0
}

/// `menu_tnormal` (33) — draw title `int_in[0]` normal (`int_in[1]`=1) or
/// highlighted/inverted (0).
pub fn menu_tnormal(pb: &Aespb) {
    do_chg(pb, pb.intin(0), SELECTED, pb.intin(1) == 0); // select when normal==0
}

/// `menu_text` (34) — replace item `int_in[0]`'s text with the string at
/// `addr_in[1]`, in place.
pub fn menu_text(pb: &Aespb) {
    let tree = pb.addrin(0) as *const Object;
    let item = pb.intin(0);
    let src = pb.addrin(1) as *const u8;
    if tree.is_null() || item < 0 || src.is_null() {
        pb.set_ret(0);
        return;
    }
    // SAFETY: valid tree; the item's ob_spec points at a writable text
    // buffer of at least the original capacity (GEM contract).
    unsafe {
        let dst = (*tree.add(item as usize)).ob_spec as *mut u8;
        if !dst.is_null() {
            let mut i = 0;
            while i < 128 {
                let c = *src.add(i);
                *dst.add(i) = c;
                if c == 0 {
                    break;
                }
                i += 1;
            }
        }
    }
    if let Some(backend) = host().and_then(|h| h.raster()) {
        object::draw_obj(tree, item, 0, backend);
    }
    pb.set_ret(1);
}

/// Shared helper: set/clear one `ob_state` bit on `obj` and redraw it
/// (EmuTOS `do_chg`).
fn do_chg(pb: &Aespb, obj: i16, stateflag: u16, set: bool) {
    let tree = pb.addrin(0) as *mut Object;
    if tree.is_null() || obj < 0 {
        pb.set_ret(0);
        return;
    }
    // SAFETY: valid tree.
    let mut state = unsafe { (*tree.add(obj as usize)).ob_state };
    if set {
        state |= stateflag;
    } else {
        state &= !stateflag;
    }
    object::change(tree, obj, state, true);
    pb.set_ret(1);
}
