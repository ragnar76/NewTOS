//! `desk::act` — the mouse-driven selection, highlight, and drag state machine
//! (EmuTOS `deskact.c`), per `docs/api/desktop/deskact.md`.
//!
//! Reimplements the four selection rules (`act_bsclick`), the icon/text hit-test
//! refinement (`act_chkobj` — the **D7** single-geometry hit-test on the object's
//! own `ib_xicon`/`ib_xtext` sub-rectangles, via the one `g_screen`/`objc_find`
//! source), the rubber-band bulk select (`act_allchg`), and the desktop-icon
//! reposition (`move_drvicon`, which bypasses `fun_drag`). The interactive drag
//! *loop* (rubber-band tracking / drag-box) lives in [`super::main`], where the
//! AES trap wrappers are, and calls into the pure state methods here.
//!
//! Nothing in this file crosses an ABI boundary; the user-observable selection
//! behaviour is the only "contract" (`deskact.md`) and the state-machine
//! internals are a clean reimplementation.

use alloc::vec::Vec;

use super::global::{DeskState, Grect, Point, DROOT, NIL};
use super::model::{AType, View};
use super::win::{DESKWH, LEN_FNODE, OB_SELECTED};

use tos_api::vdi::font::CELL_W;

/// `OB_DISABLED` state bit — `act_chg` refuses to change a disabled object.
const OB_DISABLED: u16 = 0x0008;

/// The result an [`super::main`] click handler acts on: whether the selection
/// changed (so it can repaint the affected window/desktop).
#[derive(Clone, Copy, Default)]
pub struct ClickResult {
    /// A repaint is needed.
    pub redraw: bool,
    /// The object the click resolved to (`root` for "empty area", or an item).
    pub obj: i16,
}

impl DeskState {
    /// `act_chkobj` — refine `objc_find`'s bounding-box hit for object `obid`
    /// (found in `root`) against the object's **own** icon/text sub-rectangles
    /// (EmuTOS `deskact.c:374`, the **D7** fix). Returns `true` if `(mx, my)`
    /// actually falls on the icon glyph or its label (icon view) / the text
    /// line (text view), not merely the object's bounding box.
    ///
    /// The desktop root (`DROOT`) is **always** treated as icon view, regardless
    /// of `g_iview` (`deskact.c:383`) — `g_iview` only affects real windows.
    pub fn act_chkobj(&self, root: i16, obid: i16, mx: i16, my: i16) -> bool {
        if obid < 0 || obid == root {
            return false;
        }
        // Absolute top-left of the object: root's screen origin + the item's
        // root-relative box (root's parent is ROOT at (0,0)).
        let (rx, ry) = (self.g_screen[root as usize].ob_x, self.g_screen[root as usize].ob_y);
        let ox = rx + self.g_screen[obid as usize].ob_x;
        let oy = ry + self.g_screen[obid as usize].ob_y;

        let view = if root == DROOT { View::Icon } else { self.view };
        match view {
            View::Text => {
                // The whole text line width (LEN_FNODE chars), full item height.
                let w = LEN_FNODE * CELL_W;
                let h = self.g_screen[obid as usize].ob_height;
                in_rect(mx, my, ox, oy, w, h)
            }
            View::Icon => {
                let ib = &self.g_screeninfo[obid as usize].block;
                // Label box first, then the glyph box (EmuTOS tests text first).
                in_rect(mx, my, ox + ib.ib_xtext, oy + ib.ib_ytext, ib.ib_wtext, ib.ib_htext)
                    || in_rect(mx, my, ox + ib.ib_xicon, oy + ib.ib_yicon, ib.ib_wicon, ib.ib_hicon)
            }
        }
    }

    /// `act_chg` — set/clear `obid`'s [`OB_SELECTED`] bit and its backing
    /// `f_selected` (EmuTOS `deskact.c:419`). Refuses a disabled object.
    /// Returns whether the state actually changed.
    pub fn act_chg(&mut self, handle: i16, obid: i16, set: bool) -> bool {
        if self.g_screen[obid as usize].ob_flags & OB_DISABLED != 0 {
            return false;
        }
        let was = self.g_screen[obid as usize].ob_state & OB_SELECTED != 0;
        if was == set {
            return false;
        }
        if set {
            self.g_screen[obid as usize].ob_state |= OB_SELECTED;
        } else {
            self.g_screen[obid as usize].ob_state &= !OB_SELECTED;
        }
        // Keep the paired FNode.f_selected in lockstep (window items only).
        if let Some(slot) = self.win_slot(handle) {
            let fnode = self.g_screeninfo[obid as usize].fnode;
            if fnode >= 0 {
                if let Some(f) = self.windows.slots[slot].w_pnode.p_flist.get_mut(fnode as usize) {
                    f.f_selected = set;
                }
            }
        }
        true
    }

    /// `act_bsclick` — the four click-selection rules (EmuTOS `deskact.c:566`).
    /// `obid` is the hit object (`root` for empty area, [`NIL`] for outside a
    /// window), `shifted` = a shift-click, `dclick` = a double-click. Returns
    /// whether anything changed (so the caller repaints).
    ///
    /// 1. Shift-click on empty space → no-op.
    /// 2. Click on empty space / outside a window → deselect everything here.
    /// 3. Plain click on an object → if a double-click **or** it was not already
    ///    selected, deselect all then select just it; a **single** click on an
    ///    **already-selected** object is a no-op (so a held button can drag the
    ///    existing multi-selection without collapsing it — `deskact.c:590`).
    /// 4. Shift-click on an object → toggle just that object.
    pub fn act_bsclick(&mut self, handle: i16, root: i16, obid: i16, shifted: bool, dclick: bool) -> bool {
        // Rule 1: shift-click on empty space.
        if obid == root && shifted {
            return false;
        }
        // Rule 2: click on empty space or outside a window.
        if obid == root || obid == NIL {
            return self.desk_clear(handle);
        }
        // Rule 4: shift-click on an object → toggle it.
        if shifted {
            let set = self.g_screen[obid as usize].ob_state & OB_SELECTED == 0;
            return self.act_chg(handle, obid, set);
        }
        // Rule 3: plain click on an object.
        let already = self.g_screen[obid as usize].ob_state & OB_SELECTED != 0;
        if dclick || !already {
            let cleared = self.desk_clear(handle);
            let selected = self.act_chg(handle, obid, true);
            return cleared || selected;
        }
        // Single click on an already-selected object: no-op (drag support).
        false
    }

    /// `act_allchg` (rubber-band select) — set every item child of `root`
    /// whose icon/text sub-rectangle intersects the (screen-absolute) selection
    /// rectangle `(sx, sy, sw, sh)` selected, deselecting the rest (EmuTOS
    /// `deskact.c:480`, `set == TRUE`). Returns whether anything changed.
    pub fn act_allchg_rect(&mut self, handle: i16, root: i16, sx: i16, sy: i16, sw: i16, sh: i16) -> bool {
        let mut changed = false;
        let mut child = self.g_screen[root as usize].ob_head;
        while child != NIL && child != root {
            // Hit if the item's own rect intersects the band AND act_chkobj
            // confirms the glyph/label (not just the cell) is inside it.
            let hit = self.rect_hits_obj(root, child, sx, sy, sw, sh);
            if self.act_chg(handle, child, hit) {
                changed = true;
            }
            child = self.g_screen[child as usize].ob_next;
        }
        changed
    }

    /// Whether object `child`'s icon/text sub-rectangle overlaps the
    /// screen-absolute band `(sx, sy, sw, sh)` (helper for [`act_allchg_rect`]).
    fn rect_hits_obj(&self, root: i16, child: i16, sx: i16, sy: i16, sw: i16, sh: i16) -> bool {
        let (rx, ry) = (self.g_screen[root as usize].ob_x, self.g_screen[root as usize].ob_y);
        let ox = rx + self.g_screen[child as usize].ob_x;
        let oy = ry + self.g_screen[child as usize].ob_y;
        let view = if root == DROOT { View::Icon } else { self.view };
        let boxes: [(i16, i16, i16, i16); 2] = match view {
            View::Text => {
                let h = self.g_screen[child as usize].ob_height;
                [(ox, oy, LEN_FNODE * CELL_W, h), (0, 0, 0, 0)]
            }
            View::Icon => {
                let ib = &self.g_screeninfo[child as usize].block;
                [
                    (ox + ib.ib_xtext, oy + ib.ib_ytext, ib.ib_wtext, ib.ib_htext),
                    (ox + ib.ib_xicon, oy + ib.ib_yicon, ib.ib_wicon, ib.ib_hicon),
                ]
            }
        };
        boxes.iter().any(|&(bx, by, bw, bh)| bw > 0 && bh > 0 && rects_overlap(sx, sy, sw, sh, bx, by, bw, bh))
    }

    /// `move_drvicon` — reposition every selected **desktop** icon by the drag
    /// delta `(dx, dy)` and snap each to the desktop grid (EmuTOS
    /// `deskact.c:126-163`; the direct-from-`act_bdown` path that bypasses
    /// `fun_drag`). Returns whether any icon moved.
    ///
    /// The new spot is written back into the icon's [`Anode`]
    /// (`an_disk->a_xspot = tree[obj].ob_x;`, `deskact.c:146-151`) — the object
    /// tree is rebuilt from the list by every `app_blddesk`, so without the
    /// write-back a dragged icon would jump home the moment *Install devices*
    /// or *Remove icon* rebuilt the desktop, and `app_save` would persist the
    /// old spot.
    pub fn move_drvicon(&mut self, dx: i16, dy: i16) -> bool {
        let (icw, ich) = (self.g_icw.max(1), self.g_ich.max(1));
        let cols = (self.g_desk.w / icw).max(1);
        let rows = (self.g_desk.h / ich).max(1);
        let mut moved = false;
        let mut child = self.g_screen[DROOT as usize].ob_head;
        while child != NIL && child != DROOT {
            if self.g_screen[child as usize].ob_state & OB_SELECTED != 0 {
                let ox = self.g_screen[child as usize].ob_x + dx;
                let oy = self.g_screen[child as usize].ob_y + dy;
                // Snap to the nearest grid cell, clamped to the desktop.
                let col = ((ox + icw / 2) / icw).clamp(0, cols - 1);
                let row = ((oy + ich / 2) / ich).clamp(0, rows - 1);
                let nx = col * icw;
                let ny = row * ich;
                if nx != self.g_screen[child as usize].ob_x || ny != self.g_screen[child as usize].ob_y {
                    self.g_screen[child as usize].ob_x = nx;
                    self.g_screen[child as usize].ob_y = ny;
                    // `an_disk = app_afind_by_id(obj); an_disk->a_xspot = …`
                    // (`deskact.c:146-151`, guarded there by the same "must be
                    // true, but just in case" comment).
                    if let Some(i) = self.app_afind_by_id(child) {
                        self.g_ahead[i].a_xspot = nx;
                        self.g_ahead[i].a_yspot = ny;
                    }
                    moved = true;
                }
            }
            child = self.g_screen[child as usize].ob_next;
        }
        moved
    }

    /// The bounding box (screen-absolute) of every selected item under `root`,
    /// or `None` if nothing is selected — the drag-outline extent
    /// (`gr_accobs`/`gr_extent`, `deskact.c:87`/`:169`).
    pub fn selection_extent(&self, root: i16) -> Option<(i16, i16, i16, i16)> {
        let (rx, ry) = (self.g_screen[root as usize].ob_x, self.g_screen[root as usize].ob_y);
        let (mut x0, mut y0, mut x1, mut y1) = (i16::MAX, i16::MAX, i16::MIN, i16::MIN);
        let mut any = false;
        let mut child = self.g_screen[root as usize].ob_head;
        while child != NIL && child != root {
            if self.g_screen[child as usize].ob_state & OB_SELECTED != 0 {
                let ox = rx + self.g_screen[child as usize].ob_x;
                let oy = ry + self.g_screen[child as usize].ob_y;
                let ow = self.g_screen[child as usize].ob_width;
                let oh = self.g_screen[child as usize].ob_height;
                x0 = x0.min(ox);
                y0 = y0.min(oy);
                x1 = x1.max(ox + ow);
                y1 = y1.max(oy + oh);
                any = true;
            }
            child = self.g_screen[child as usize].ob_next;
        }
        any.then_some((x0, y0, x1 - x0, y1 - y0))
    }

    /// `gr_accobs(tree, root, &num)` (EmuTOS `deskact.c:87-127`) — the
    /// screen-absolute top-left of every **selected** child of `root`, in
    /// sibling order, which `gr_drgplns` translates its outline polyline to
    /// once per dragged item.
    ///
    /// EmuTOS allocates the list with `dos_alloc_anyram` and caps it at
    /// `dos_avail_anyram() / sizeof(Point)` (`:110-112`) — a ROM-era guard
    /// against a selection larger than free RAM. A [`Vec`] carries the same
    /// list without the cap; the allocator's own failure is the only limit,
    /// and there is no observable difference for any selection a screen can
    /// hold.
    pub fn gr_accobs(&self, root: i16) -> Vec<Point> {
        let (rx, ry) = (self.g_screen[root as usize].ob_x, self.g_screen[root as usize].ob_y);
        let mut pts = Vec::new();
        let mut obj = self.g_screen[root as usize].ob_head;
        while obj > root {
            if self.g_screen[obj as usize].ob_state & OB_SELECTED != 0 {
                pts.push(Point {
                    x: rx + self.g_screen[obj as usize].ob_x,
                    y: ry + self.g_screen[obj as usize].ob_y,
                });
            }
            obj = self.g_screen[obj as usize].ob_next;
        }
        pts
    }

    /// The screen-absolute extent of object `obid` under `root` — the
    /// `rc_copy((GRECT *)&tree[obj].ob_x, &t); t.g_x += tree[root].ob_x;
    /// t.g_y += tree[root].ob_y;` of `act_chg` (`deskact.c:433-435`), which
    /// its caller clips against `pc` and hands to `do_wredraw`.
    pub fn obj_extent(&self, root: i16, obid: i16) -> Option<Grect> {
        let o = self.g_screen.get(obid as usize)?;
        let r = self.g_screen.get(root as usize)?;
        Some(Grect { x: r.ob_x + o.ob_x, y: r.ob_y + o.ob_y, w: o.ob_width, h: o.ob_height })
    }

    /// Whether object `obid` (under `root`) is currently selected — used by the
    /// drag path to decide "click-on-selected → drag the group".
    pub fn is_selected(&self, obid: i16) -> bool {
        obid >= 0 && self.g_screen[obid as usize].ob_state & OB_SELECTED != 0
    }

    /// The [`AType`] of object `obid` (for the drag destination decision).
    pub fn obj_atype(&self, obid: i16) -> AType {
        self.g_screeninfo.get(obid as usize).map(|s| s.atype).unwrap_or(AType::None)
    }

    /// Whether `handle` is the desktop pseudo-window.
    pub fn is_desktop(handle: i16) -> bool {
        handle == DESKWH
    }
}

/// Point-in-rectangle test (exclusive right/bottom edge).
fn in_rect(px: i16, py: i16, x: i16, y: i16, w: i16, h: i16) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}

/// Rectangle-overlap test (exclusive right/bottom edges).
#[allow(clippy::too_many_arguments)]
fn rects_overlap(ax: i16, ay: i16, aw: i16, ah: i16, bx: i16, by: i16, bw: i16, bh: i16) -> bool {
    ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah
}
