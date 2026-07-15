//! AES Menu Manager (`menu_*`, opcodes 30–34) — Batch 1.
//!
//! Spec: [`docs/api/aes/menu.md`]. A menu **is** an OBJECT tree: the root
//! (`THESCREEN`, `G_IBOX`) holds the bar (`THEBAR`, object 1) as its
//! `ob_head`, and the titles (`G_TITLE`) hang under `THEACTIVE`. A title's
//! drop-down box is **not** its child (D-085): the drop-down `G_BOX`es are the
//! `ob_next` sibling chain rooted at `THESCREEN`'s hidden *second* child
//! (`ob_tail`, the `THEMENUS` container), the *N*-th box matching the *N*-th
//! title — this is what `menu_sub` walks. Each box's own children are the
//! items (`G_STRING`). So `menu_bar` just draws the bar subtree, and the
//! item/title state calls (`icheck`/`ienable`/`tnormal`) flip one `ob_state`
//! bit through the shared `do_chg` helper — each with its own redraw /
//! disabled-guard contract (`gemsuper.c:171-180`): `icheck` never repaints,
//! `ienable` repaints only when item bit 15 is set, `tnormal` always repaints
//! and refuses a `DISABLED` title.
//!
//! **Batch-1 scope:** draw the bar + the four original-TOS state/text
//! calls. The interactive drop-down tracking (mouse over titles → open
//! the box → highlight item → post `MN_SELECTED`) is Batch 2.

use super::object::{self, Object};
use super::Aespb;
use crate::host::host;
use crate::vdi::backend::{ClipRect, Pel, RasterBackend};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicI16, AtomicU8, AtomicUsize, Ordering};

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

/// The y-coordinate where the desktop work area begins: the bottom edge of the
/// installed menu bar ([`BAR_H`]), or `0` when no menu bar is installed. The
/// window manager uses this to bound the desktop pseudo-window's rectangle list
/// (the desk area = screen minus the menu bar); see `window::desktop_work`.
pub(super) fn desk_top() -> i16 {
    if active().is_null() {
        0
    } else {
        BAR_H
    }
}

// ob_state bits.
const SELECTED: u16 = 0x0001;
const CHECKED: u16 = 0x0004;
const DISABLED: u16 = 0x0008;

/// `G_STRING` object type (`obdefs.h`) — the low byte of `ob_type`. A submenu
/// can only be attached to a `G_STRING` menu item (`menu_attach` guard,
/// `gemmnext.c:539-541`).
const G_STRING: u16 = 28;

/// `G_TITLE` object type (`obdefs.h`) — a menu-bar title. A title spans the
/// full bar height, so its `just_draw` fill covers the separator row at
/// `BAR_H-1` (see [`redraw_bar_separator`]).
const G_TITLE: u16 = 32;

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
/// logic. Entered (from `evnt_multi`) as soon as the cursor is over the bar.
/// Follows the mouse: the title under the cursor opens its drop-down
/// (`menu_sub`), items highlight on hover, a button **press** over an item is
/// the selection, and leaving the bar + open box closes it. Returns
/// `Some((title, item))` or `None` (cancelled).
///
/// Behaviour matches GEM's hover menus (open-on-enter, click-to-pick). The
/// screen under each box is saved on open and restored on close (`save_under`
/// / `restore_under` — GEM's `menu_sr`), so it works over windows too; the
/// mouse cursor is hidden across every box/highlight draw so it neither
/// smears nor pollutes the backing.
pub fn track(tree: *mut Object, backend: &dyn RasterBackend) -> Option<(i16, i16)> {
    let h = host()?;
    let tp = tree as *const Object;
    let mut cur_title = NIL;
    let mut cur_box = NIL;
    let mut cur_item = NIL;
    // Hover model (GEM): the menu opens as the cursor enters a title; an item
    // is chosen on a button *press* over it; leaving the bar and the open box
    // closes the menu with no selection.
    let mut was_down = crate::vdi::mouse_buttons() & 0x01 != 0;

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
                // Bracket the highlight change so the visible cursor (which
                // sits over the item) does not smear.
                crate::vdi::cursor_hide(backend);
                if cur_item != NIL {
                    // `draw_obj` only paints a G_STRING's background when it is
                    // SELECTED, so clear the old item's black highlight to
                    // white before redrawing it normal — otherwise the
                    // hovered-over item stays black.
                    let (ix, iy, iw, ih) = object::abs_rect(tp, cur_item);
                    let clip = ClipRect::full(backend.width(), backend.height());
                    backend.fill_rect(ix, iy, iw, ih, WHITE, clip);
                    object::set_state_bit(tree, cur_item, SELECTED, false);
                }
                if it != NIL {
                    object::set_state_bit(tree, it, SELECTED, true);
                }
                // Item fills span the full box width, so a highlight/clear
                // paints over the box's 1-px frame; redraw it so the hovered
                // rows keep their border (top/bottom + both sides).
                let (bx, by, bw, bh) = object::abs_rect(tp, cur_box);
                let clip = ClipRect::full(backend.width(), backend.height());
                backend.fill_rect(bx, by, bw, 1, BLACK, clip); // top
                backend.fill_rect(bx, by + bh - 1, bw, 1, BLACK, clip); // bottom
                backend.fill_rect(bx, by, 1, bh, BLACK, clip); // left
                backend.fill_rect(bx + bw - 1, by, 1, bh, BLACK, clip); // right
                crate::vdi::cursor_show(backend);
                cur_item = it;
            }
        }

        // Press edge over an item → select it.
        if !was_down && down && cur_item != NIL {
            // Clear the item's highlight bit and restore the screen under the
            // box, cursor hidden so neither smears. The title stays
            // highlighted: the app un-highlights it via menu_tnormal after
            // acting on MN_SELECTED.
            crate::vdi::cursor_hide(backend);
            object::set_state_bit(tree, cur_item, SELECTED, false);
            erase_box_inner(tree, cur_box, backend);
            crate::vdi::cursor_show(backend);
            return Some((cur_title, cur_item));
        }

        // Leaving the bar *and* the open box closes the menu (no selection).
        let in_bar = my < BAR_H;
        let in_box = cur_box != NIL && point_in(tp, cur_box, mx, my);
        if !in_bar && !in_box {
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

/// Is device point `(mx, my)` inside object `obj`'s absolute rectangle?
fn point_in(tree: *const Object, obj: i16, mx: i16, my: i16) -> bool {
    let (x, y, w, h) = object::abs_rect(tree, obj);
    mx >= x && mx < x + w && my >= y && my < y + h
}

// ── drop-down save/restore backing (GEM's "save under") ──────────────────
//
// Opening a menu box covers part of the screen; closing it must put the
// pixels back. Like the cursor backing, snapshot the box rectangle into a
// CPU buffer on open and blit it back on close — covers both selection and
// cancel without a full desktop repaint, and works over windows too. The
// snapshot is taken with the mouse cursor hidden so it is clean and the
// cursor stays on top of the open menu.
const MB_W: usize = 256;
const MB_H: usize = 400;
struct MenuBack {
    px: UnsafeCell<[Pel; MB_W * MB_H]>,
    x: AtomicI16,
    y: AtomicI16,
    w: AtomicI16,
    h: AtomicI16,
    valid: AtomicU8,
}
// SAFETY: touched only from the single-CPU AES trap / desktop event path.
unsafe impl Sync for MenuBack {}
static MBACK: MenuBack = MenuBack {
    px: UnsafeCell::new([0; MB_W * MB_H]),
    x: AtomicI16::new(0),
    y: AtomicI16::new(0),
    w: AtomicI16::new(0),
    h: AtomicI16::new(0),
    valid: AtomicU8::new(0),
};

/// Snapshot the pixels the box `(x,y,w,h)` will cover. Marks invalid if the
/// box is larger than the backing → the caller falls back to a white erase.
fn save_under(backend: &dyn RasterBackend, x: i16, y: i16, w: i16, h: i16) {
    if w <= 0 || h <= 0 || w as usize > MB_W || h as usize > MB_H {
        MBACK.valid.store(0, Ordering::Relaxed);
        return;
    }
    // SAFETY: single path; bounds checked above.
    let store = unsafe { &mut *MBACK.px.get() };
    for row in 0..h {
        for col in 0..w {
            store[row as usize * MB_W + col as usize] = backend.read_pixel(x + col, y + row);
        }
    }
    MBACK.x.store(x, Ordering::Relaxed);
    MBACK.y.store(y, Ordering::Relaxed);
    MBACK.w.store(w, Ordering::Relaxed);
    MBACK.h.store(h, Ordering::Relaxed);
    MBACK.valid.store(1, Ordering::Relaxed);
}

/// Blit the saved pixels back. Returns `false` if nothing was saved (box too
/// big), so the caller can white-erase instead.
fn restore_under(backend: &dyn RasterBackend) -> bool {
    if MBACK.valid.swap(0, Ordering::Relaxed) == 0 {
        return false;
    }
    let x = MBACK.x.load(Ordering::Relaxed);
    let y = MBACK.y.load(Ordering::Relaxed);
    let w = MBACK.w.load(Ordering::Relaxed);
    let h = MBACK.h.load(Ordering::Relaxed);
    let clip = ClipRect::full(backend.width(), backend.height());
    // SAFETY: single path.
    let store = unsafe { &*MBACK.px.get() };
    for row in 0..h {
        for col in 0..w {
            let p = store[row as usize * MB_W + col as usize];
            backend.fill_rect(x + col, y + row, 1, 1, p, clip);
        }
    }
    true
}

/// Highlight `title` and draw its drop-down box; returns the box index. The
/// cursor is hidden across the draw so it stays on top and the save-under
/// snapshot is clean.
fn open_box(tree: *mut Object, title: i16, backend: &dyn RasterBackend) -> i16 {
    crate::vdi::cursor_hide(backend);
    object::set_state_bit(tree, title, SELECTED, true); // redraws the title highlighted
    let imenu = menu_sub(tree as *const Object, title);
    let (x, y, w, h) = object::abs_rect(tree as *const Object, imenu);
    save_under(backend, x, y, w, h);
    object::draw_obj(tree as *const Object, imenu, MAX_DEPTH, backend);
    crate::vdi::cursor_show(backend);
    imenu
}

/// Un-highlight `title`, restore the screen under its box and redraw the
/// title normal.
fn close_box(tree: *mut Object, title: i16, imenu: i16, backend: &dyn RasterBackend) {
    crate::vdi::cursor_hide(backend);
    erase_box_inner(tree, imenu, backend);
    object::set_state_bit(tree, title, SELECTED, false); // redraws the title normal
    // A G_TITLE spans the whole bar height, so the white un-highlight fill
    // erased the bar's separator line under it — redraw it.
    redraw_bar_separator(backend);
    crate::vdi::cursor_show(backend);
}

/// Repaint the menu bar's separator line — the full-width black stroke at
/// `y = BAR_H-1` drawn by [`menu_bar`] (`menu.rs`). A bar `G_TITLE` has
/// `ob_height == BAR_H`, so drawing a title (normal *or* highlighted) fills its
/// cell across the whole bar height, including the separator row — erasing the
/// segment of the line under that title. This is the single source of truth
/// both the interactive cancel path ([`close_box`]) and the app-driven
/// un-highlight ([`menu_tnormal`]) call to restore the line after the title
/// redraw. A full-width repaint over an intact line is idempotent.
fn redraw_bar_separator(backend: &dyn RasterBackend) {
    let sw = backend.width() as i16;
    let clip = ClipRect::full(backend.width(), backend.height());
    backend.fill_rect(0, BAR_H - 1, sw, 1, BLACK, clip);
}

/// Does normalizing `obj` need the bar separator repainted? True only for a
/// bar-level `G_TITLE` whose absolute cell spans the separator row `BAR_H-1` —
/// the objects whose `just_draw` white-fill erases the line. Drop-down items
/// (`G_STRING`, below the bar) and non-title objects do not qualify, so an
/// unrelated `menu_tnormal` never paints a spurious line across the screen.
fn title_touches_separator(tree: *const Object, obj: i16) -> bool {
    // SAFETY: valid tree (GEM contract); `obj` is a caller-validated index.
    let ob_type = unsafe { (*tree.add(obj as usize)).ob_type & 0x00ff };
    if ob_type != G_TITLE {
        return false;
    }
    let sep_row = BAR_H - 1;
    let (_, y, _, h) = object::abs_rect(tree, obj);
    y <= sep_row && sep_row < y + h
}

/// Put the pixels saved under the box back, or white-erase if the box was too
/// big to snapshot. Caller owns the cursor hide/show.
fn erase_box_inner(tree: *mut Object, imenu: i16, backend: &dyn RasterBackend) {
    if imenu == NIL {
        return;
    }
    if !restore_under(backend) {
        let (x, y, w, h) = object::abs_rect(tree as *const Object, imenu);
        let clip = ClipRect::full(backend.width(), backend.height());
        backend.fill_rect(x, y, w, h, WHITE, clip);
    }
}

/// `menu_icheck` (31) — add (`int_in[1]`=1) / remove the check mark on
/// item `int_in[0]`. `do_chg(..., CHECKED, CHECK_IT, FALSE, FALSE)`
/// (`gemsuper.c:172`): **no immediate redraw** (`dodraw=false`) — the mark
/// appears the next time the drop-down is drawn — and **no disabled guard**.
pub fn menu_icheck(pb: &Aespb) {
    do_chg(pb, pb.intin(0), CHECKED, pb.intin(1) != 0, false, false);
}

/// `menu_ienable` (32) — enable (`int_in[1]`=1) / disable item
/// `int_in[0]`. `do_chg(..., ITEM_NUM&0x7fff, DISABLED, !ENABLE_IT,
/// (ITEM_NUM&0x8000)!=0, FALSE)` (`gemsuper.c:175-176`): bit 15 of the item
/// word is the **redraw-now** flag (masked off the index, reused as `dodraw`);
/// no disabled guard.
pub fn menu_ienable(pb: &Aespb) {
    let raw = pb.intin(0);
    let item = raw & 0x7fff;
    let dodraw = (raw as u16) & 0x8000 != 0; // item bit 15 = redraw-now flag
    do_chg(pb, item, DISABLED, pb.intin(1) == 0, dodraw, false); // disable when enable==0
}

/// `menu_tnormal` (33) — draw title `int_in[0]` normal (`int_in[1]`=1) or
/// highlighted/inverted (0). `do_chg(..., TITLE_NUM, SELECTED, !NORMAL_IT,
/// TRUE, TRUE)` (`gemsuper.c:179`): **always redraws** and, with
/// `chkdisabled=true`, refuses a `DISABLED` title (returns `0`, state
/// unchanged).
pub fn menu_tnormal(pb: &Aespb) {
    do_chg(pb, pb.intin(0), SELECTED, pb.intin(1) == 0, true, true); // select when normal==0
    // `do_chg` (dodraw=true) repainted the title's cell. A bar `G_TITLE` spans
    // the full bar height, so that fill covered the separator row at BAR_H-1,
    // erasing the segment of the bar's separator line under the title (the same
    // reason `close_box` restores it on the cancel path). Repaint the line when
    // the normalized object is a bar title touching that row — not for
    // drop-down items or unrelated objects.
    let tree = pb.addrin(0) as *const Object;
    let obj = pb.intin(0);
    if tree.is_null() || obj < 0 {
        return;
    }
    if title_touches_separator(tree, obj) {
        if let Some(backend) = host().and_then(|h| h.raster()) {
            redraw_bar_separator(backend);
        }
    }
}

/// `menu_text` (34) — replace item `int_in[0]`'s text with the string at
/// `addr_in[1]`, in place. EmuTOS is an inline `strcpy` into `ob_spec` with
/// **no redraw** (`gemsuper.c:183`): the caller repaints the item itself
/// (e.g. via `objc_draw`, or a `menu_ienable` with item bit 15 set). Bounded
/// to 128 bytes as a NewTOS safety cap over EmuTOS's unbounded copy — see the
/// spec "NewTOS implementation notes".
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
    pb.set_ret(1);
}

/// Desk-accessory registry (EmuTOS `mn_register`). NewTOS records the
/// registration — the title pointer + owning application id — and hands back
/// a stable slot id. Wiring the accessory into the Desk menu's drop-down and
/// posting `AC_OPEN` when it is picked is a documented follow-up (it needs
/// the desk-menu accessory convention); the registration is real, so a
/// caller's `menu_register` succeeds and its slot id is stable.
const NUM_ACCS: usize = 6;
struct Accs {
    title: UnsafeCell<[u64; NUM_ACCS]>, // title pointer per slot (0 = free)
    pid: UnsafeCell<[i16; NUM_ACCS]>,
}
// SAFETY: single boot CPU only touches the AES.
unsafe impl Sync for Accs {}
static ACCS: Accs = Accs {
    title: UnsafeCell::new([0; NUM_ACCS]),
    pid: UnsafeCell::new([0; NUM_ACCS]),
};

/// `menu_register` (35) — register a desk accessory. `int_in[0]` = the
/// application id, `addr_in[0]` = its menu-title string. Returns the
/// accessory's menu id (`0..NUM_ACCS`) or `-1` when the id is invalid or no
/// slot is free (EmuTOS `mn_register`).
pub fn menu_register(pb: &Aespb) {
    let pid = pb.intin(0);
    let pstr = pb.addrin(0);
    if pid < 0 {
        pb.set_ret(-1);
        return;
    }
    // SAFETY: single-CPU AES; exclusive owner of the registry.
    unsafe {
        let title = &mut *ACCS.title.get();
        let apid = &mut *ACCS.pid.get();
        for slot in 0..NUM_ACCS {
            if title[slot] == 0 {
                title[slot] = pstr;
                apid[slot] = pid;
                pb.set_ret(slot as i16);
                return;
            }
        }
    }
    pb.set_ret(-1); // no free accessory slot
}

/// Shared helper: set/clear one `ob_state` bit on `obj`, optionally redrawing
/// it (EmuTOS `do_chg`, `gemmnlib.c:145-172`).
///
/// - `set`: set (`true`) or clear (`false`) `stateflag` in `ob_state` (EmuTOS
///   `dochg`).
/// - `dodraw`: repaint the object after the change (EmuTOS `dodraw`). When
///   `false`, `ob_state` is still updated but nothing is drawn — the change
///   shows on the next draw of the enclosing box.
/// - `chkdisabled`: if `true` and the object is already `DISABLED`, refuse the
///   change — leave `ob_state` untouched, draw nothing, return `0`.
///
/// Returns `1` normally, `0` only when the disabled guard blocks the change.
fn do_chg(pb: &Aespb, obj: i16, stateflag: u16, set: bool, dodraw: bool, chkdisabled: bool) {
    let tree = pb.addrin(0) as *mut Object;
    if tree.is_null() || obj < 0 {
        pb.set_ret(0);
        return;
    }
    // SAFETY: valid tree.
    let state = unsafe { (*tree.add(obj as usize)).ob_state };
    if chkdisabled && (state & DISABLED) != 0 {
        pb.set_ret(0); // disabled item: no mutation, no draw
        return;
    }
    let new = if set { state | stateflag } else { state & !stateflag };
    object::change(tree, obj, new, dodraw);
    pb.set_ret(1);
}

// ── menu extensions (later-AES: pop-up / sub-menu / settings) ─────────────

/// Native `MENU` descriptor (later-AES pop-up / sub-menu).
#[repr(C)]
struct Menu {
    mn_tree: u64,   // OBJECT*
    mn_menu: i16,   // the drop-down box object
    mn_item: i16,   // start / selected item
    mn_scroll: i16,
    mn_keystate: i16,
}

/// `menu_popup` (36) — display the pop-up menu described by `addr_in[0]`
/// (`MENU`) so its start item sits at (`int_in[0]`, `int_in[1]`), track it,
/// and write the chosen item into the `MENU` at `addr_in[1]`. Returns 1 on a
/// selection, 0 on cancel. EmuTOS `mn_popup` (single level, no sub-menus).
pub fn menu_popup(pb: &Aespb) {
    let menu = pb.addrin(0) as *const Menu;
    let (xpos, ypos) = (pb.intin(0), pb.intin(1));
    let mdata = pb.addrin(1) as *mut Menu;
    if menu.is_null() {
        pb.set_ret(0);
        return;
    }
    let Some(backend) = host().and_then(|h| h.raster()) else {
        pb.set_ret(0);
        return;
    };
    // SAFETY: valid MENU (GEM contract).
    let (tree, box_obj, start_item) =
        unsafe { ((*menu).mn_tree as *mut Object, (*menu).mn_menu, (*menu).mn_item) };
    if tree.is_null() || box_obj < 0 {
        pb.set_ret(0);
        return;
    }
    // Position the box so `start_item` sits at (xpos, ypos); clamp to screen.
    // SAFETY: valid tree.
    unsafe {
        let iy = if start_item >= 0 { (*tree.add(start_item as usize)).ob_y } else { 0 };
        let b = &mut *tree.add(box_obj as usize);
        b.ob_x = xpos.clamp(0, (backend.width() as i16 - b.ob_width).max(0));
        b.ob_y = (ypos - iy).clamp(0, (backend.height() as i16 - b.ob_height).max(0));
    }
    let sel = popup_track(tree, box_obj, backend);
    if !mdata.is_null() {
        // SAFETY: valid MENU out (GEM contract).
        unsafe {
            (*mdata).mn_tree = tree as u64;
            (*mdata).mn_menu = box_obj;
            (*mdata).mn_item = sel;
            (*mdata).mn_scroll = 0;
            (*mdata).mn_keystate = 0;
        }
    }
    pb.set_ret(if sel >= 0 { 1 } else { 0 });
}

/// Track a single pop-up box: hover highlights the item under the cursor, a
/// button press picks it (or cancels outside any item). Returns the item or
/// `NIL`. Reuses the menu-bar save-under + cursor-bracketing.
fn popup_track(tree: *mut Object, box_obj: i16, backend: &dyn RasterBackend) -> i16 {
    let Some(h) = host() else { return NIL };
    let tp = tree as *const Object;
    // Wait for the button that opened the pop-up to be released.
    while crate::vdi::mouse_buttons() & 0x01 != 0 {
        h.pump_input();
    }
    crate::vdi::cursor_hide(backend);
    let (bx, by, bw, bh) = object::abs_rect(tp, box_obj);
    save_under(backend, bx, by, bw, bh);
    object::draw_obj(tp, box_obj, MAX_DEPTH, backend);
    crate::vdi::cursor_show(backend);

    let mut cur_item = NIL;
    let result;
    loop {
        let (mx, my) = crate::vdi::mouse_position();
        let it = object::child_at(tp, box_obj, mx, my);
        if it != cur_item {
            crate::vdi::cursor_hide(backend);
            let clip = ClipRect::full(backend.width(), backend.height());
            if cur_item != NIL {
                let (ix, iy, iw, ih) = object::abs_rect(tp, cur_item);
                backend.fill_rect(ix, iy, iw, ih, WHITE, clip);
                object::set_state_bit(tree, cur_item, SELECTED, false);
            }
            if it != NIL {
                object::set_state_bit(tree, it, SELECTED, true);
            }
            let (rx, ry, rw, rh) = object::abs_rect(tp, box_obj);
            backend.fill_rect(rx, ry, rw, 1, BLACK, clip);
            backend.fill_rect(rx, ry + rh - 1, rw, 1, BLACK, clip);
            backend.fill_rect(rx, ry, 1, rh, BLACK, clip);
            backend.fill_rect(rx + rw - 1, ry, 1, rh, BLACK, clip);
            crate::vdi::cursor_show(backend);
            cur_item = it;
        }
        if crate::vdi::mouse_buttons() & 0x01 != 0 {
            result = cur_item; // NIL when pressed outside any item → cancel
            break;
        }
        h.pump_input();
    }
    crate::vdi::cursor_hide(backend);
    if result != NIL {
        object::set_state_bit(tree, result, SELECTED, false);
    }
    erase_box_inner(tree, box_obj, backend);
    crate::vdi::cursor_show(backend);
    result
}

/// Native `MN_SET` behaviour block (later-AES).
#[repr(C)]
#[derive(Clone, Copy)]
struct MnSet {
    display: i32,
    drag: i32,
    delay: i32,
    speed: i32,
    height: i16,
}

/// EmuTOS `MIN_HEIGHT_VALUE` (`gemmnext.c:76`) — the smallest legal scroll
/// height, in items.
const MIN_HEIGHT_VALUE: i16 = 5;

/// Clamp `menu_settings`'s `height` field to EmuTOS's
/// `[MIN_HEIGHT_VALUE, gl_rfull.g_h / gl_hchar]` range (`clamp_height`,
/// `gemmnext.c:83-95`): at least 5 items, at most as many system-font rows as
/// fit on the full screen. `gl_hchar` maps to the 16-pixel system-font cell
/// height ([`crate::vdi::font::CELL_H`]); `gl_rfull.g_h` to the raster
/// backend's full-screen height. When no raster is up (workstation not yet
/// initialised) only the lower bound is enforceable — the upper clamp is
/// skipped rather than guessed.
fn clamp_height(height: i16) -> i16 {
    if height < MIN_HEIGHT_VALUE {
        return MIN_HEIGHT_VALUE;
    }
    if let Some(backend) = host().and_then(|h| h.raster()) {
        let max_height = backend.height() as i16 / crate::vdi::font::CELL_H;
        if height > max_height {
            return max_height;
        }
    }
    height
}

struct Settings(UnsafeCell<MnSet>);
// SAFETY: single boot CPU only touches the AES.
unsafe impl Sync for Settings {}
static SETTINGS: Settings = Settings(UnsafeCell::new(MnSet {
    display: 0,
    drag: 0,
    delay: 250, // sub-menu delay (ms) — a common TOS default
    speed: 10,
    height: BAR_H,
}));

/// `menu_settings` (39) — get (`int_in[0]`=0) or set (1) the menu-behaviour
/// block at `addr_in[0]` (`MN_SET`). Negative fields are left unchanged on
/// set (like TOS). EmuTOS `mn_settings`.
pub fn menu_settings(pb: &Aespb) {
    let set = pb.addrin(0) as *mut MnSet;
    let flag = pb.intin(0);
    if set.is_null() {
        pb.set_ret(0);
        return;
    }
    // SAFETY: single-CPU AES; valid MN_SET (GEM contract).
    unsafe {
        let cur = &mut *SETTINGS.0.get();
        if flag == 0 {
            *set = *cur;
        } else {
            let s = *set;
            if s.display >= 0 {
                cur.display = s.display;
            }
            if s.drag >= 0 {
                cur.drag = s.drag;
            }
            if s.delay >= 0 {
                cur.delay = s.delay;
            }
            if s.speed >= 0 {
                cur.speed = s.speed;
            }
            if s.height >= 0 {
                cur.height = clamp_height(s.height);
            }
        }
    }
    pb.set_ret(1);
}

/// `menu_attach` (37) — attach (`ME_ATTACH`=1) / remove (`ME_REMOVE`=2) /
/// query (`ME_INQUIRE`=0) a submenu on a menu-bar item (EmuTOS `mn_attach`,
/// `gemmnext.c:535-557`). Dispatch: `mn_attach(MPOP_FLAG, MPOP_IN,
/// MPOP_ITEM, MPOP_OUT)` = `(int_in[0], addr_in[0], int_in[1], addr_in[1])`.
///
/// NewTOS has no `SMIB` submenu registry (`gl_submenu`) yet, so no submenu can
/// actually be recorded or displayed. The **argument-determined** outcomes are
/// still honoured with EmuTOS's real failure values — **no faked success**:
///  - the target item must be a `G_STRING` (guard applied to all three
///    operations, `gemmnext.c:539-541`) → else `0`;
///  - `ME_REMOVE` / `ME_INQUIRE` on an item with no attached submenu → `0`
///    (nothing to remove/report), `gemmnext.c:401-433,471-481`;
///  - `ME_ATTACH` → `0`: a real attach needs the SMIB registry NewTOS lacks
///    → returned as "not attached", never a fake `1`.
///
/// Full submenu support (`ME_ATTACH` success, the `ob_type`/`ob_flags`
/// encoding of D-087) requires the SMIB submenu infrastructure — tracked as a
/// separate task. Until then every path returns an honest EmuTOS `FALSE`.
pub fn menu_attach(pb: &Aespb) {
    let tree = pb.addrin(0) as *const Object;
    let item = pb.intin(1);
    if tree.is_null() || item < 0 {
        pb.set_ret(0);
        return;
    }
    // A submenu can only be attached to a G_STRING item — the guard EmuTOS
    // applies before the ME_* switch, for all three operations.
    // SAFETY: valid tree (GEM contract).
    let ob_type = unsafe { (*tree.add(item as usize)).ob_type };
    if (ob_type & 0x00ff) != G_STRING {
        pb.set_ret(0);
        return;
    }
    // With no SMIB registry, no item ever has a submenu: inquire/remove report
    // "none" and attach cannot record one. All honest EmuTOS FALSE values.
    pb.set_ret(0);
}

/// `menu_istart` (38) — get/set the starting item of a scrollable submenu
/// (EmuTOS `mn_istart`, `gemmnext.c:562-587`). Dispatch: `mn_istart(MPOP_FLAG,
/// MPOP_IN, MPOP_ITEM, MPOP_ITEM2)` = `(int_in[0], addr_in[0], int_in[1],
/// int_in[2])`.
///
/// `mn_istart` first looks the submenu up by (tree, menu box) and returns
/// `FALSE` (0) when none is attached — *before* the flag switch. NewTOS
/// registers no submenus (no SMIB registry, same separate task as
/// `menu_attach`), so the lookup always fails: return `0` honestly after
/// validating the arguments. **Never a fake `1`.**
pub fn menu_istart(pb: &Aespb) {
    let flag = pb.intin(0);
    let tree = pb.addrin(0) as *const Object;
    let menu = pb.intin(1);
    // Valid flags are 0 (inquire) and 1 (update); any other → FALSE
    // (gemmnext.c:584-585). A null tree or negative box is likewise invalid.
    if tree.is_null() || menu < 0 || (flag != 0 && flag != 1) {
        pb.set_ret(0);
        return;
    }
    // No submenu is ever attached → find_submenu_from_tree fails → 0, exactly
    // as EmuTOS returns for an unattached submenu.
    pb.set_ret(0);
}
