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
use crate::vdi::backend::{ClipRect, Pel, RasterBackend};
use core::sync::atomic::{AtomicUsize, Ordering};

/// `THESCREEN` — the menu tree root; its `ob_tail` is the box that holds
/// all drop-down menus.
const THESCREEN: i16 = 0;
/// `THEBAR` — the menu-bar object (child of the root).
const THEBAR: i16 = 1;
/// `THEACTIVE` — the box holding the title objects (children of the bar).
const THEACTIVE: i16 = 2;
const NIL: i16 = -1;
const MAX_DEPTH: i16 = 16;

/// The active menu tree (`gl_mntree`), set by `menu_bar(show=1)` and
/// cleared by `menu_bar(show=0)`. Stored as a `usize` for `Sync` without
/// `unsafe`; only the single boot CPU touches it.
static MNTREE: AtomicUsize = AtomicUsize::new(0);

/// The currently installed menu tree, or null if none.
pub fn active() -> *mut Object {
    MNTREE.load(Ordering::Relaxed) as *mut Object
}
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
        MNTREE.store(tree as usize, Ordering::Relaxed);
    } else {
        // Erase the bar area.
        backend.fill_rect(0, 0, sw, BAR_H, WHITE, clip);
        MNTREE.store(0, Ordering::Relaxed);
    }
    pb.set_ret(1);
}

/// Interactive drop-down tracking — the only genuinely new Menu-Manager
/// logic. Entered while the button is held over the bar (e.g. from
/// `evnt_multi`). Follows the mouse: the title under the cursor opens its
/// drop-down (`menu_sub`), items highlight on hover, and on **release**
/// the item under the cursor is the selection. Returns
/// `Some((title, item))` or `None` (released outside any item).
///
/// Behaviour matches EmuTOS `mn_do` (press-drag-release); the rectangle-
/// wait state machine is replaced by a poll loop at our event altitude.
/// **Limit:** no save/restore under the box yet — closing a menu erases
/// to white, which is correct on the empty desktop but needs `menu_sr`
/// once windows can sit beneath the bar.
pub fn track(tree: *mut Object, backend: &dyn RasterBackend) -> Option<(i16, i16)> {
    let h = host()?;
    let tp = tree as *const Object;
    let mut cur_title = NIL;
    let mut cur_box = NIL;
    let mut cur_item = NIL;
    let mut was_down = true; // entered on a press in the bar

    loop {
        let (mx, my) = crate::vdi::mouse_position();
        let down = crate::vdi::mouse_buttons() & 0x01 != 0;

        // Title under the cursor (children of THEACTIVE).
        let t = object::child_at(tp, THEACTIVE, mx, my);
        if t != NIL && t != cur_title {
            if cur_title != NIL {
                close_box(tree, cur_title, cur_box, backend);
                cur_item = NIL;
            }
            cur_title = t;
            cur_box = open_box(tree, t, backend);
        }

        // Item under the cursor (children of the open box).
        if cur_box != NIL {
            let it = object::child_at(tp, cur_box, mx, my);
            if it != cur_item {
                if cur_item != NIL {
                    object::set_state_bit(tree, cur_item, SELECTED, false);
                }
                if it != NIL {
                    object::set_state_bit(tree, it, SELECTED, true);
                }
                cur_item = it;
            }
        }

        // Release edge → select the item under the cursor, or cancel.
        if was_down && !down {
            if cur_item != NIL {
                erase_box(tree, cur_box, backend);
                object::set_state_bit(tree, cur_item, SELECTED, false);
                // Leave the title highlighted: the app un-highlights it
                // via menu_tnormal after acting on MN_SELECTED.
                return Some((cur_title, cur_item));
            }
            if cur_title != NIL {
                close_box(tree, cur_title, cur_box, backend);
            }
            return None;
        }
        was_down = down;
        h.pump_input();
    }
}

/// Programmatically open `title`'s drop-down and optionally highlight
/// `item` (`NIL` for none). Returns the box index. Renders the same state
/// the interactive `track` shows mid-drag; also the primitive a future
/// `menu_popup` builds on.
pub fn open(tree: *mut Object, title: i16, item: i16, backend: &dyn RasterBackend) -> i16 {
    let imenu = open_box(tree, title, backend);
    if item != NIL {
        object::set_state_bit(tree, item, SELECTED, true);
    }
    imenu
}

/// Title index → its drop-down box index (EmuTOS `menu_sub`): walk the box
/// list under `THESCREEN.ob_tail` by the title's position.
fn menu_sub(tree: *const Object, title: i16) -> i16 {
    // SAFETY: valid menu tree (GEM contract).
    unsafe {
        let themenus = (*tree.add(THESCREEN as usize)).ob_tail;
        let mut imenu = (*tree.add(themenus as usize)).ob_head;
        let mut i = title - THEACTIVE;
        while i > 1 {
            imenu = (*tree.add(imenu as usize)).ob_next;
            i -= 1;
        }
        imenu
    }
}

/// Highlight `title` and draw its drop-down box; returns the box index.
fn open_box(tree: *mut Object, title: i16, backend: &dyn RasterBackend) -> i16 {
    object::set_state_bit(tree, title, SELECTED, true);
    let imenu = menu_sub(tree as *const Object, title);
    object::draw_obj(tree as *const Object, imenu, MAX_DEPTH, backend);
    imenu
}

/// Un-highlight `title` and erase its drop-down box.
fn close_box(tree: *mut Object, title: i16, imenu: i16, backend: &dyn RasterBackend) {
    object::set_state_bit(tree, title, SELECTED, false);
    erase_box(tree, imenu, backend);
}

/// Erase a drop-down box back to white (no save/restore yet — see `track`).
fn erase_box(tree: *mut Object, imenu: i16, backend: &dyn RasterBackend) {
    if imenu == NIL {
        return;
    }
    let (x, y, w, h) = object::abs_rect(tree as *const Object, imenu);
    let clip = ClipRect::full(backend.width(), backend.height());
    backend.fill_rect(x, y, w, h, WHITE, clip);
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
