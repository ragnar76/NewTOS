//! `desk::win` — the window node (`WNode`) and the window-list management
//! (`win_start`/`win_alloc`/`win_free`/`win_find`/`win_top`/`win_count`).
//!
//! Reimplements the list-bookkeeping half of EmuTOS `deskwin.c`
//! (`deskwin.c:105-283`), per `docs/api/desktop/deskwin.md`. `WNODE` never
//! crosses an ABI boundary (`_data-model.md`: "Contract: none"), so the
//! z-order chain and free-list — a hand-threaded `w_next` linked array in
//! EmuTOS — are modelled as a slot [`Vec`] plus an explicit z-order [`Vec`].
//!
//! The **rendering** half of `deskwin.c` — `win_view`, `win_ocalc`,
//! `win_bldview`, scrolling and the bulk ops — is Task #6 and lives here
//! later; it is deliberately absent for now.

use alloc::string::String;
use alloc::vec::Vec;

use tos_api::aes::object::IconBlk;
use tos_api::vdi::font::{CELL_H, CELL_W};

use super::global::{
    min_wint, DeskState, Grect, DROOT, G_BOX, G_ICON, G_STRING, MIN_HINT, NIL, NUM_WNODES,
};
use super::icons::{build_iconblk, IconKind};
use super::model::{FNode, PNode, View, LEN_LABEL};

/// `LEN_FNODE` (`deskwin.h:36`) — the fixed formatted text-line width in
/// characters; the `V_TEXT` item's own extent is `LEN_FNODE * char_w`.
pub const LEN_FNODE: i16 = 48;

/// `ob_state` `SELECTED` bit (`obdefs.h`) — a selected/highlighted item; the
/// AES object renderer inverts a `G_ICON`/`G_STRING` that carries it.
pub const OB_SELECTED: u16 = 0x0001;

/// The `ob_spec` colour word for a window's item-root box: solid-white
/// interior, **no** border (border thickness 0 in bits 16-23; colour word
/// `0x0070` = `gr_crack` border 0 / text 0 / pattern IP_SOLID(7) / inside
/// WHITE(0)). Making the root a filled box means `objc_draw(g_screen, w_root)`
/// **clears the window work area to white before laying the items on top** —
/// so every content redraw (scroll, view/sort switch via `desk_all`) is
/// self-erasing and leaves no stale-icon residue, exactly as GEM's window
/// root object does (the AES `draw_frame` comment: "the app clears the work
/// area on every content redraw"). The desktop `DROOT` keeps its `G_IBOX`
/// (its background is the pattern fill, not white).
const WORK_BG_SPEC: u64 = 0x0000_0070;

// `WM_ARROWED` action codes (`aesdefs.h:246-253`) the scroll handlers map to a
// `w_cvcol`/`w_cvrow` delta.
/// Scroll up one page.
pub const WA_UPPAGE: i16 = 0;
/// Scroll down one page.
pub const WA_DNPAGE: i16 = 1;
/// Scroll up one line.
pub const WA_UPLINE: i16 = 2;
/// Scroll down one line.
pub const WA_DNLINE: i16 = 3;
/// Scroll left one page.
pub const WA_LFPAGE: i16 = 4;
/// Scroll right one page.
pub const WA_RTPAGE: i16 = 5;
/// Scroll left one line.
pub const WA_LFLINE: i16 = 6;
/// Scroll right one line.
pub const WA_RTLINE: i16 = 7;

/// `w_flags` bit marking `DeskState::windows.desktop` (`WN_DESKTOP`).
pub const WN_DESKTOP: u16 = 0x0001;
/// `w_flags` bit "needs rebuilding" (`WN_REBUILD`) — set/tested by later
/// tasks.
pub const WN_REBUILD: u16 = 0x8000;
/// The desktop pseudo-window's AES handle (`DESKWH`).
pub const DESKWH: i16 = 0;

/// One open window, or (via [`WindowList::desktop`]) the desktop pseudo-window
/// (EmuTOS `WNODE`, `deskwin.h:57-80`).
#[derive(Default)]
pub struct WNode {
    /// `WN_DESKTOP`/`WN_REBUILD` flags.
    pub w_flags: u16,
    /// AES window handle; `0` = slot not in use (the free sentinel).
    pub w_id: i16,
    /// Desktop object id for this window (negative → drive letter that opened
    /// it, one's-complemented).
    pub w_obid: i16,
    /// Index into `DeskState::g_screen[]` of this window's item-object root.
    pub w_root: i16,
    /// Current virtual top-left column being displayed (scroll position).
    pub w_cvcol: i16,
    /// Current virtual top-left row being displayed (scroll position).
    pub w_cvrow: i16,
    /// Physical column count — how many items fit the current window width.
    pub w_pncol: i16,
    /// Physical row count — how many items fit the current window height.
    pub w_pnrow: i16,
    /// Virtual column count — columns needed to show every file.
    pub w_vncol: i16,
    /// Virtual row count — rows needed to show every file.
    pub w_vnrow: i16,
    /// This window's file list (embedded, not a pointer).
    pub w_pnode: PNode,
    /// Title-bar text (path).
    pub w_name: String,
    /// Info-line text (item count / total size).
    pub w_info: String,
}

impl WNode {
    /// A fresh, free window slot (all fields zero, `w_id == 0`).
    pub fn free_slot() -> Self {
        WNode::default()
    }
}

/// The desktop's window list (EmuTOS `g_wdesktop`/`g_wlist`/`g_wfirst`/
/// `g_wcnt`). The desktop pseudo-window is held separately from the real
/// window slots.
#[derive(Default)]
pub struct WindowList {
    /// The desktop pseudo-window (`g_wdesktop`, handle [`DESKWH`]).
    pub desktop: WNode,
    /// The [`NUM_WNODES`] real window slots (`g_wlist`); a slot is free when
    /// its `w_id == 0`.
    pub slots: Vec<WNode>,
    /// Slot indices in z-order, front-to-back (the `g_wfirst` chain).
    pub zorder: Vec<usize>,
    /// Count of currently-open windows (`g_wcnt`).
    pub count: usize,
}

impl DeskState {
    /// `win_start` — one-time window-list init (EmuTOS `deskwin.c:105-135`).
    ///
    /// Sets the boot view/sort defaults (`START_VIEW`=icon, `START_SORT`=name,
    /// `g_ifit`=true), marks the desktop pseudo-window `WN_DESKTOP`, and
    /// allocates [`NUM_WNODES`] free window slots.
    ///
    /// **Deferred:** EmuTOS `win_start()` also calls `win_view()` here to
    /// derive the grid pitch before `obj_init()`. That call (and the metrics
    /// it produces) is Task #6; NewTOS's `obj_init` does not depend on it (see
    /// [`DeskState::obj_init`]).
    pub fn win_start(&mut self) {
        self.view = super::model::View::Icon;
        self.sort = super::model::Sort::Name;
        self.size_to_fit = true;

        let wl = &mut self.windows;
        wl.desktop = WNode::free_slot();
        wl.desktop.w_flags = WN_DESKTOP;
        wl.desktop.w_id = DESKWH;

        wl.slots.clear();
        wl.slots.resize_with(NUM_WNODES, WNode::free_slot);
        wl.zorder.clear();
        wl.count = 0;
    }

    /// `win_find` — resolve an AES window handle to its [`WNode`] (EmuTOS
    /// `deskwin.c:198-212`). [`DESKWH`] returns the desktop pseudo-window;
    /// otherwise a linear scan of the open slots. `None` if not open (every
    /// caller must handle this — it defends against "window closed between
    /// event post and processing").
    pub fn win_find(&mut self, wh: i16) -> Option<&mut WNode> {
        if wh == DESKWH {
            return Some(&mut self.windows.desktop);
        }
        self.windows.slots.iter_mut().find(|w| w.w_id == wh)
    }

    /// `win_alloc` — register a caller-created AES window (`w_id` from
    /// `wind_create`) into the list (EmuTOS `deskwin.c:156-190`). Reserves a
    /// free slot and its `g_screen[]` root box (`obj_walloc`-equivalent, one of
    /// the boxes at indices `2..WOBS_START`), initialises the scroll/grid
    /// counters to zero, and splices the slot to the front of the z-order.
    /// Returns the slot index, or `None` if all [`NUM_WNODES`] slots are in
    /// use.
    ///
    /// (Creating the AES window and building its view is the caller's job in
    /// Task #6/#8; this function owns only the list + slot + root-object
    /// bookkeeping.)
    pub fn win_alloc(&mut self, w_id: i16, w_obid: i16) -> Option<usize> {
        if self.windows.count >= NUM_WNODES {
            return None;
        }
        let slot = self.windows.slots.iter().position(|w| w.w_id == 0)?;

        let mut node = WNode::free_slot();
        node.w_id = w_id;
        node.w_obid = w_obid;
        // One window box per slot: g_screen[2 + slot] (2..WOBS_START).
        node.w_root = 2 + slot as i16;
        self.windows.slots[slot] = node;

        self.windows.count += 1;
        self.windows.zorder.insert(0, slot);
        Some(slot)
    }

    /// `win_free` — release the window with handle `w_id` (EmuTOS
    /// `deskwin.c:141-150`). Returns this window's item objects to the arena
    /// free chain (`obj_wfree`, the **F4** fix — `deskwin.c:148`), then resets
    /// the slot to free (`w_id = 0`), decrements the open count, and unlinks it
    /// from the z-order. Returns `false` if no such window is open.
    ///
    /// Without the `obj_wfree` call, a closed window's item `OBJECT`s stayed
    /// allocated on `g_screen[]` forever, leaking the fixed arena pool: after
    /// enough open/close cycles `obj_ialloc` would exhaust and later windows
    /// would render empty.
    pub fn win_free(&mut self, w_id: i16) -> bool {
        let Some(slot) = self.windows.slots.iter().position(|w| w.w_id == w_id) else {
            return false;
        };
        // Return the item objects to the free chain and collapse the root box
        // (deskwin.c:148 `obj_wfree(pwin->w_root, 0,0,0,0)`).
        let w_root = self.windows.slots[slot].w_root;
        self.obj_wfree(w_root, 0, 0, 0, 0);

        self.windows.slots[slot] = WNode::free_slot();
        self.windows.count = self.windows.count.saturating_sub(1);
        self.windows.zorder.retain(|&s| s != slot);
        true
    }

    /// `win_top` — move the window with handle `w_id` to the front of the
    /// z-order (topmost paint order) (EmuTOS `deskwin.c:218-240`). Returns
    /// `false` if no such window is open.
    pub fn win_top(&mut self, w_id: i16) -> bool {
        let Some(slot) = self.windows.slots.iter().position(|w| w.w_id == w_id) else {
            return false;
        };
        self.windows.zorder.retain(|&s| s != slot);
        self.windows.zorder.insert(0, slot);
        true
    }

    /// `win_count` — number of currently-open windows (`g_wcnt`).
    pub fn win_count(&self) -> usize {
        self.windows.count
    }

    /// `win_view` — recompute the single-item extent/interval/pitch for the
    /// **current** `self.view` (EmuTOS `deskwin.c:63-99`). There is one active
    /// view type for the whole desktop, not per-window.
    ///
    /// Reads `self.full_work_w`, the work-area width of a maximum-size window at
    /// the current resolution (cached at boot); it feeds `g_icols`
    /// (`CONF_WITH_SIZE_TO_FIT`). Called at boot and again on every view / size-
    /// to-fit change (the grid pitch differs per view).
    ///
    /// Must run **after** `app_start` (needs `g_wicon`/`g_hicon` for the
    /// `V_ICON` case) and after `full_work_w` is set.
    pub fn win_view(&mut self) {
        let full_work_w = self.full_work_w;
        let screen_h = self.g_desk.y + self.g_desk.h;
        match self.view {
            View::Text => {
                // A whole text line + a leading gap for the rubber-band select
                // margin (the '-1' byte-aligns the fast blit path in EmuTOS;
                // reproduced for metric parity).
                self.g_iwext = LEN_FNODE * CELL_W;
                self.g_ihext = CELL_H;
                self.g_iwint = 2 * CELL_W - 1;
                self.g_ihint = 2;
            }
            View::Icon => {
                self.g_iwext = self.g_wicon;
                self.g_ihext = self.g_hicon;
                self.g_iwint = min_wint(screen_h);
                self.g_ihint = MIN_HINT;
            }
        }
        // Column/row pitch = extent + fixed gap (a *window* pitch: no even
        // redistribution, unlike the desktop's g_icw/g_ich).
        self.g_iwspc = self.g_iwext + self.g_iwint;
        self.g_ihspc = self.g_ihext + self.g_ihint;
        // Max columns fitting a full-size window (CONF_WITH_SIZE_TO_FIT).
        self.g_icols = (full_work_w / self.g_iwspc).max(1);
    }

    /// `win_bldview` — rebuild the displayed item objects of window `slot` for
    /// the current work-area rect `work` (EmuTOS `deskwin.c:392-520`). Frees
    /// the previous items, computes the virtual/physical grid via
    /// [`win_ocalc`], walks the file list from the scroll-adjusted start, and
    /// allocates one `OBJECT` per visible item on the same grid model for
    /// **both** views (the D5 single-scroll-unit fix). Returns the scrollbar
    /// size/position (thousandths) for the caller to hand to `wind_set`.
    pub fn win_bldview(&mut self, slot: usize, work: Grect) -> SliderSet {
        // 1. Reset: free previous item objects, resize the root box to `work`,
        //    and mark every file "not displayed" (f_obid = NIL).
        let w_root = self.windows.slots[slot].w_root;
        self.obj_wfree(w_root, work.x, work.y, work.w, work.h);
        // Make the item-root a solid-white borderless box so `objc_draw`
        // clears the work area before drawing the items — self-erasing content
        // redraws (no scroll / view-switch residue). See [`WORK_BG_SPEC`].
        {
            let root = &mut self.g_screen[w_root as usize];
            root.ob_type = G_BOX;
            root.ob_spec = WORK_BG_SPEC;
        }
        for f in &mut self.windows.slots[slot].w_pnode.p_flist {
            f.f_obid = NIL;
        }

        // 2. win_ocalc: derive the fit counts from the pixel work-area size /
        //    the current pitch (integer division — a partial trailing row/col
        //    is not counted as "fits").
        let cnt = self.windows.slots[slot].w_pnode.p_count;
        let wfit = work.w / self.g_iwspc;
        let hfit = work.h / self.g_ihspc;
        let oc = win_ocalc(
            cnt,
            wfit,
            hfit,
            self.size_to_fit,
            self.g_icols,
            self.windows.slots[slot].w_cvcol,
            self.windows.slots[slot].w_cvrow,
        );
        {
            let wn = &mut self.windows.slots[slot];
            wn.w_vncol = oc.vncol;
            wn.w_vnrow = oc.vnrow;
            wn.w_pncol = oc.pncol;
            wn.w_pnrow = oc.pnrow;
            wn.w_cvcol = oc.cvcol;
            wn.w_cvrow = oc.cvrow;
        }

        // 3. Effective fit for this build: one *extra* partially-visible
        //    row/column at the fringe (the deliberate `+1`, deskwin.c:412-417),
        //    clamped to the remaining virtual extent.
        let o_wfit = if self.size_to_fit {
            (oc.pncol + 1).min(oc.vncol - oc.cvcol)
        } else {
            oc.pncol
        };
        let o_hfit = (oc.pnrow + 1).min(oc.vnrow - oc.cvrow);

        // 4. Placement loop.
        let view = self.view;
        let g_wicon = self.g_wicon;
        let (g_iwspc, g_ihspc, g_iwint, g_ihint, g_iwext, g_ihext) = (
            self.g_iwspc,
            self.g_ihspc,
            self.g_iwint,
            self.g_ihint,
            self.g_iwext,
            self.g_ihext,
        );
        let mut pidx = oc.start;
        let (mut r_cnt, mut c_cnt) = (0i16, 0i16);
        while c_cnt < o_wfit && r_cnt < o_hfit && pidx < cnt {
            let yoff = r_cnt * g_ihspc;
            let xoff = c_cnt * g_iwspc;
            // The item box is inset from the cell top-left by the full gap
            // (a leading margin, applied before every item incl. the first).
            let obid =
                match self.obj_ialloc(w_root, xoff + g_iwint, yoff + g_ihint, g_iwext, g_ihext) {
                    Some(o) => o,
                    None => break, // free chain exhausted → stop (EmuTOS parity)
                };
            self.windows.slots[slot].w_pnode.p_flist[pidx].f_obid = obid;
            self.g_screeninfo[obid as usize].fnode = pidx as i16;
            self.build_item(slot, pidx, obid, view, g_wicon);

            pidx += 1;
            c_cnt += 1;
            if c_cnt == o_wfit {
                r_cnt += 1;
                c_cnt = 0;
                // Skip virtual columns scrolled off to the right, so the next
                // row's first file is the one at column cvcol (size-to-fit).
                if self.size_to_fit {
                    pidx += (oc.vncol - o_wfit).max(0) as usize;
                }
            }
        }

        // 5. Scrollbar size/position in thousandths.
        SliderSet {
            vslsize: mul_div(oc.pnrow, 1000, oc.vnrow),
            vslide: if oc.vnrow > oc.pnrow {
                mul_div(oc.cvrow, 1000, oc.vnrow - oc.pnrow)
            } else {
                0
            },
            hslsize: if self.size_to_fit {
                mul_div(oc.pncol, 1000, oc.vncol)
            } else {
                1000
            },
            hslide: if self.size_to_fit && oc.vncol > oc.pncol {
                mul_div(oc.cvcol, 1000, oc.vncol - oc.pncol)
            } else {
                0
            },
        }
    }

    /// Build one item `OBJECT` (icon or text line) at `obid` for file `pidx`
    /// of window `slot` — the per-item half of the placement loop, shared by
    /// both views so they use one grid/scroll model (D5).
    fn build_item(&mut self, slot: usize, pidx: usize, obid: i16, view: View, g_wicon: i16) {
        // Copy the fields out first so the file-list borrow ends before the
        // g_screen/g_screeninfo borrows.
        let f = self.windows.slots[slot].w_pnode.p_flist[pidx].clone();
        let info = &mut self.g_screeninfo[obid as usize];
        info.label = [0; LEN_LABEL];
        info.atype = if f.is_dir() {
            super::model::AType::Folder
        } else {
            super::model::AType::File
        };

        match view {
            View::Icon => {
                let kind = if f.is_dir() { IconKind::Folder } else { IconKind::Doc };
                let name = f.name_bytes();
                let end = name.len().min(LEN_LABEL - 1);
                info.label[..end].copy_from_slice(&name[..end]);
                let ptext = info.label.as_ptr() as u64;
                info.block = build_iconblk(kind, ptext, 0, g_wicon);
                let spec = &info.block as *const IconBlk as u64;
                let o = &mut self.g_screen[obid as usize];
                o.ob_type = G_ICON;
                o.ob_spec = spec;
            }
            View::Text => {
                format_fnode(&f, &mut info.label);
                let spec = info.label.as_ptr() as u64;
                let o = &mut self.g_screen[obid as usize];
                o.ob_type = G_STRING;
                o.ob_spec = spec;
            }
        }
    }

    /// The item-object root of window `handle`: [`DROOT`] for the desktop (or a
    /// closed handle), else the window's `w_root`.
    pub fn root_of(&self, handle: i16) -> i16 {
        match self.win_slot(handle) {
            Some(slot) => self.windows.slots[slot].w_root,
            None => DROOT,
        }
    }

    /// The open-window slot index for AES handle `w_id` (`None` for the
    /// desktop or a closed handle).
    pub fn win_slot(&self, w_id: i16) -> Option<usize> {
        if w_id == DESKWH {
            return None;
        }
        self.windows.slots.iter().position(|w| w.w_id == w_id)
    }

    /// `win_srtall` (EmuTOS `deskwin.c:768-780`) — for every open window, reset
    /// the vertical scroll to the top (`w_cvrow = 0`) and re-sort its file list
    /// by the current [`Sort`] key. Does **not** rebuild the view (that is
    /// `win_bdall`) or redraw (that is `win_shwall`).
    pub fn win_srtall(&mut self) {
        let sort = self.sort;
        for w in &mut self.windows.slots {
            if w.w_id != DESKWH {
                w.w_cvrow = 0;
                w.w_pnode.pn_sort(sort);
            }
        }
    }

    /// `desk_verify` (EmuTOS `desksupp.c:175`) — make window `handle` the
    /// current one: set `g_cwin`/`g_croot`/`g_wlastsel`. When `changed`, rebuild
    /// its item grid first (a size change re-flows the columns) and return the
    /// new [`SliderSet`] for the caller to push via `wind_set`; otherwise
    /// `None`.
    pub fn desk_verify(&mut self, handle: i16, work: Grect, changed: bool) -> Option<SliderSet> {
        self.g_cwin = handle;
        self.g_wlastsel = handle;
        self.g_croot = match self.win_slot(handle) {
            Some(slot) => self.windows.slots[slot].w_root,
            None => DROOT,
        };
        if changed {
            if let Some(slot) = self.win_slot(handle) {
                return Some(self.win_bldview(slot, work));
            }
        }
        None
    }

    /// `win_arrow` (EmuTOS `deskwin.c:716-762`) — translate a `WM_ARROWED`
    /// action into a new scroll position and apply it via [`win_blt`]. Returns
    /// the new [`SliderSet`] when the scroll position actually moved, else
    /// `None` (nothing to redraw). `work` is the window's current work rect.
    pub fn win_arrow(&mut self, slot: usize, work: Grect, action: i16) -> Option<SliderSet> {
        let w = &self.windows.slots[slot];
        let (newcv, horizontal) = match action {
            WA_UPLINE => (w.w_cvrow - 1, false),
            WA_DNLINE => (w.w_cvrow + 1, false),
            WA_UPPAGE => (w.w_cvrow - w.w_pnrow.max(1), false),
            WA_DNPAGE => (w.w_cvrow + w.w_pnrow.max(1), false),
            WA_LFLINE => (w.w_cvcol - 1, true),
            WA_RTLINE => (w.w_cvcol + 1, true),
            WA_LFPAGE => (w.w_cvcol - w.w_pncol.max(1), true),
            WA_RTPAGE => (w.w_cvcol + w.w_pncol.max(1), true),
            _ => return None,
        };
        self.win_blt(slot, work, horizontal, newcv)
    }

    /// `win_slide` (EmuTOS `deskwin.c:682-714`) — an AES slider drag value
    /// (0..1000 per-mille of travel) to a new scroll position, applied via
    /// [`win_blt`]. Returns the new [`SliderSet`] when the position moved.
    pub fn win_slide(
        &mut self,
        slot: usize,
        work: Grect,
        horizontal: bool,
        value: i16,
    ) -> Option<SliderSet> {
        let w = &self.windows.slots[slot];
        let (vn, pn) =
            if horizontal { (w.w_vncol, w.w_pncol) } else { (w.w_vnrow, w.w_pnrow) };
        let range = (vn - pn).max(0);
        let newcv = mul_div(value.clamp(0, 1000), range, 1000);
        self.win_blt(slot, work, horizontal, newcv)
    }

    /// `win_blt` (EmuTOS `deskwin.c:553-636`, contract-only part) — clamp the
    /// requested new scroll position `newcv` (`win_delta`: `>= 0`, `<= vn-pn`),
    /// and if it differs from the current one, apply it and rebuild the view.
    /// Returns the resulting [`SliderSet`], or `None` on a no-op scroll.
    ///
    /// EmuTOS's blit fast-path (copy the still-valid strip, redraw only the
    /// freshly-exposed edge) is a performance optimisation whose *result* is
    /// identical to a full rebuild+redraw; NewTOS always rebuilds the view (the
    /// self-clearing white root box makes the redraw residue-free) — a clean
    /// reimplementation of the observable contract, not the m68k blit trick.
    fn win_blt(
        &mut self,
        slot: usize,
        work: Grect,
        horizontal: bool,
        newcv: i16,
    ) -> Option<SliderSet> {
        let w = &self.windows.slots[slot];
        let (vn, pn, cur) = if horizontal {
            (w.w_vncol, w.w_pncol, w.w_cvcol)
        } else {
            (w.w_vnrow, w.w_pnrow, w.w_cvrow)
        };
        let newcv = newcv.clamp(0, (vn - pn).max(0));
        if newcv == cur {
            return None;
        }
        if horizontal {
            self.windows.slots[slot].w_cvcol = newcv;
        } else {
            self.windows.slots[slot].w_cvrow = newcv;
        }
        Some(self.win_bldview(slot, work))
    }

    /// `desk_clear` (EmuTOS `desksupp.c:122`) — deselect every item in window
    /// `handle` (or the desktop): clear each object's [`OB_SELECTED`] bit and,
    /// for a real window, its file's `f_selected`. Returns `true` if anything
    /// was actually deselected (so the caller knows to repaint).
    pub fn desk_clear(&mut self, handle: i16) -> bool {
        let root = match self.win_slot(handle) {
            Some(slot) => self.windows.slots[slot].w_root,
            None => DROOT,
        };
        let mut changed = self.clear_obj_selection(root);
        if let Some(slot) = self.win_slot(handle) {
            for f in &mut self.windows.slots[slot].w_pnode.p_flist {
                if f.f_selected {
                    f.f_selected = false;
                    changed = true;
                }
            }
        }
        changed
    }

    /// Clear [`OB_SELECTED`] on every child of `root`; returns whether any bit
    /// was set. The object-side half of [`desk_clear`] (the paired
    /// `FNode.f_selected` is cleared there).
    fn clear_obj_selection(&mut self, root: i16) -> bool {
        let mut changed = false;
        let mut child = self.g_screen[root as usize].ob_head;
        while child != NIL && child != root {
            let o = &mut self.g_screen[child as usize];
            if o.ob_state & OB_SELECTED != 0 {
                o.ob_state &= !OB_SELECTED;
                changed = true;
            }
            child = o.ob_next;
        }
        changed
    }


    /// Number of currently-selected items under `root` (children carrying
    /// [`OB_SELECTED`]) — `men_update`'s `nsel`.
    pub fn count_selected(&self, root: i16) -> usize {
        let mut n = 0;
        let mut child = self.g_screen[root as usize].ob_head;
        while child != NIL && child != root {
            if self.g_screen[child as usize].ob_state & OB_SELECTED != 0 {
                n += 1;
            }
            child = self.g_screen[child as usize].ob_next;
        }
        n
    }

    /// The [`SliderSet`] describing the current scroll state of window `slot`
    /// without rebuilding — used when a scroll request was a no-op but the
    /// caller still wants the values.
    pub fn current_sliders(&self, slot: usize) -> SliderSet {
        let w = &self.windows.slots[slot];
        SliderSet {
            vslsize: mul_div(w.w_pnrow, 1000, w.w_vnrow),
            vslide: if w.w_vnrow > w.w_pnrow {
                mul_div(w.w_cvrow, 1000, w.w_vnrow - w.w_pnrow)
            } else {
                0
            },
            hslsize: if self.size_to_fit { mul_div(w.w_pncol, 1000, w.w_vncol) } else { 1000 },
            hslide: if self.size_to_fit && w.w_vncol > w.w_pncol {
                mul_div(w.w_cvcol, 1000, w.w_vncol - w.w_pncol)
            } else {
                0
            },
        }
    }
}

/// The scrollbar size/position (`WF_*SLSIZ`/`WF_*SLIDE`, thousandths)
/// [`DeskState::win_bldview`] computes; the caller sets them via `wind_set`.
#[derive(Clone, Copy, Default)]
pub struct SliderSet {
    /// Vertical thumb size (`WF_VSLSIZ`).
    pub vslsize: i16,
    /// Vertical thumb position (`WF_VSLIDE`).
    pub vslide: i16,
    /// Horizontal thumb size (`WF_HSLSIZ`); `1000` (full) when not size-to-fit.
    pub hslsize: i16,
    /// Horizontal thumb position (`WF_HSLIDE`).
    pub hslide: i16,
}

/// The virtual/physical grid dimensions + scroll-adjusted start index that
/// [`win_ocalc`] produces.
struct OCalc {
    vncol: i16,
    vnrow: i16,
    pncol: i16,
    pnrow: i16,
    cvcol: i16,
    cvrow: i16,
    /// Linear index into `p_flist` of the top-left visible item.
    start: usize,
}

/// `win_ocalc` (EmuTOS `deskwin.c:293-372`) — compute the virtual grid (how
/// many rows/cols would show every file), the physical grid (how many fit
/// now), the re-clamped scroll position, and the starting file index.
///
/// `wfit`/`hfit` are `work_w / g_iwspc` / `work_h / g_ihspc`. Pure scalar
/// math (the `f_obid` reset happens in [`DeskState::win_bldview`]).
fn win_ocalc(
    cnt: usize,
    wfit: i16,
    hfit: i16,
    size_to_fit: bool,
    g_icols: i16,
    mut cvcol: i16,
    mut cvrow: i16,
) -> OCalc {
    let wfit = wfit.max(1);
    let hfit = hfit.max(1);
    let cnt_i = cnt as i32;

    // Virtual grid.
    let (vncol, vnrow) = if size_to_fit {
        // Exactly as wide as the current window — no horizontal scroll.
        let vncol = wfit;
        let vnrow = ceil_div(cnt_i, vncol as i32).max(1);
        (vncol, vnrow)
    } else {
        // Fixed max-window-width virtual grid.
        let cols = g_icols.max(1);
        let vncol = (cnt_i.min(cols as i32) as i16).max(1);
        let vnrow = ceil_div(cnt_i, cols as i32).max(1);
        (vncol, vnrow)
    };

    // Physical grid.
    let pncol = wfit;
    let pnrow = hfit.min(vnrow);

    // Re-clamp the scroll position if the window just grew (decrement-only).
    let space_c = wfit.min(vncol);
    while cvcol > 0 && (vncol - cvcol) < space_c {
        cvcol -= 1;
    }
    let space_r = hfit.min(vnrow);
    while cvrow > 0 && (vnrow - cvrow) < space_r {
        cvrow -= 1;
    }

    // Starting file index (linear).
    let start = if size_to_fit {
        cvrow as usize * vncol as usize + cvcol as usize
    } else {
        cvrow as usize * pncol as usize
    };

    OCalc { vncol, vnrow, pncol, pnrow, cvcol, cvrow, start }
}

/// Ceiling of `n / d` for non-negative `n`, `d >= 1`.
fn ceil_div(n: i32, d: i32) -> i16 {
    if d <= 0 {
        return 1;
    }
    ((n + d - 1) / d) as i16
}

/// Scaled integer multiply-then-divide (EmuTOS `mul_div`, no float): the AES
/// slider `WF_*SLSIZ`/`WF_*SLIDE` thousandths idiom.
fn mul_div(a: i16, mult: i16, div: i16) -> i16 {
    if div == 0 {
        return 0;
    }
    ((a as i32 * mult as i32) / div as i32) as i16
}

/// Copy `src` into `out` starting at column `col` (bounds-checked, leaves the
/// NUL-terminator slot free).
fn put(out: &mut [u8; LEN_LABEL], col: usize, src: &[u8]) {
    for (i, &b) in src.iter().enumerate() {
        if col + i < LEN_LABEL - 1 {
            out[col + i] = b;
        }
    }
}

/// Format decimal `v` into `buf`, returning the written slice (no leading
/// zeros).
fn fmt_u32(mut v: u32, buf: &mut [u8; 12]) -> &[u8] {
    if v == 0 {
        buf[0] = b'0';
        return &buf[..1];
    }
    let mut tmp = [0u8; 12];
    let mut i = 12;
    while v > 0 {
        i -= 1;
        tmp[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    let len = 12 - i;
    buf[..len].copy_from_slice(&tmp[i..]);
    &buf[..len]
}

/// FAT date word → `"MM-DD-YY"` (EmuTOS `fmt_date`, 2-digit year).
fn fmt_date(date: u16) -> [u8; 8] {
    let day = (date & 0x1f) as u8;
    let month = ((date >> 5) & 0x0f) as u8;
    let year = ((1980u16 + (date >> 9)) % 100) as u8;
    [
        b'0' + month / 10,
        b'0' + month % 10,
        b'-',
        b'0' + day / 10,
        b'0' + day % 10,
        b'-',
        b'0' + year / 10,
        b'0' + year % 10,
    ]
}

/// FAT time word → `"HH:MM"` (EmuTOS `fmt_time`, 24-hour).
fn fmt_time(time: u16) -> [u8; 5] {
    let min = ((time >> 5) & 0x3f) as u8;
    let hour = ((time >> 11) & 0x1f) as u8;
    [b'0' + hour / 10, b'0' + hour % 10, b':', b'0' + min / 10, b'0' + min % 10]
}

/// Format one directory entry as a fixed-width `V_TEXT` line (EmuTOS
/// `format_fnode`, `deskinf.c:220-307`, per `deskinf.md`): folder/read-only
/// indicator, 8.3 name, size (blank for folders), date, time. The exact
/// column widths are a NewTOS choice (the visual layout is fidelity, not
/// contract); the **field order and semantics** match EmuTOS. Returns the
/// NUL-terminated line length.
fn format_fnode(f: &FNode, out: &mut [u8; LEN_LABEL]) -> usize {
    /// `FA_RO` (read-only) attribute bit.
    const FA_RO: u8 = 0x01;
    *out = [b' '; LEN_LABEL];

    // 1. Indicator: 0x07 folder / 0x7F read-only / space.
    out[0] = if f.is_dir() {
        0x07
    } else if f.f_attr & FA_RO != 0 {
        0x7f
    } else {
        b' '
    };

    // 2. Name (8) + extension (3), space-padded.
    let name = f.name_bytes();
    let (nm, ext) = match name.iter().position(|&c| c == b'.') {
        Some(dot) => (&name[..dot.min(8)], &name[dot + 1..]),
        None => (&name[..name.len().min(8)], &name[name.len()..]),
    };
    put(out, 2, nm);
    put(out, 11, &ext[..ext.len().min(3)]);

    // 3. Size (right-justified, ending at column 25); blank for folders.
    if !f.is_dir() {
        let mut num = [0u8; 12];
        let s = fmt_u32(f.f_size, &mut num);
        put(out, 25usize.saturating_sub(s.len()), s);
    }

    // 4. Date + time.
    put(out, 27, &fmt_date(f.f_date));
    put(out, 37, &fmt_time(f.f_time));

    // NUL-terminate after the last used column, trimming trailing spaces.
    let mut end = 42usize;
    while end > 0 && out[end - 1] == b' ' {
        end -= 1;
    }
    out[end] = 0;
    end
}
