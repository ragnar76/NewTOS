//! AES Object Manager (`objc_*`) — Batch 1: the OBJECT tree model plus
//! `objc_draw` (42), `objc_find` (43), `objc_offset` (44).
//!
//! Spec: [`docs/api/aes/object-form.md`]; rendering extracted from EmuTOS
//! `aes/gemoblib.c` (`ob_draw`/`just_draw`).
//!
//! **Native OBJECT layout:** classic TOS packs `OBJECT` into 24 bytes
//! with a 32-bit `ob_spec`. NewTOS is native AArch64, so `ob_spec` is a
//! **64-bit** value (a pointer for text/string types, a packed box word
//! for box types) and the struct is 32 bytes. Programs build trees with
//! this layout; `addr_in[0]` points at the array.
//!
//! **Batch-1 scope (documented limits, not stubs):** types `G_BOX`,
//! `G_IBOX`, `G_STRING`, `G_BUTTON`, `G_BOXCHAR`; black-on-white look
//! (the full `gr_crack` colour word, 3D effects, icons/images, and the
//! `TEDINFO` text types `G_TEXT`/`G_BOXTEXT` land in later batches). The
//! per-object clip rectangle is honoured at screen granularity for now.

use core::sync::atomic::{AtomicI16, Ordering};

use super::Aespb;
use crate::host::host;
use crate::vdi::backend::{ClipRect, MonoSrc, PatternFill, Pel, RasterBackend, Rect};

// ── OBJECT model ────────────────────────────────────────────────────

/// A GEM object tree node (native NewTOS layout, 32 bytes).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Object {
    /// next sibling, or `-1` (`NIL`).
    pub ob_next: i16,
    /// first child, or `-1`.
    pub ob_head: i16,
    /// last child, or `-1`.
    pub ob_tail: i16,
    /// `G_*` graphic type.
    pub ob_type: u16,
    /// `*` flag bits.
    pub ob_flags: u16,
    /// `*` state bits.
    pub ob_state: u16,
    /// type-specific payload: a native pointer (string/TEDINFO) or, for
    /// box types, `(thickness << 16) | colour-word`.
    pub ob_spec: u64,
    /// x relative to the parent.
    pub ob_x: i16,
    /// y relative to the parent.
    pub ob_y: i16,
    /// width in pixels.
    pub ob_width: i16,
    /// height in pixels.
    pub ob_height: i16,
}

// Graphic types (`obdefs.h`).
const G_BOX: u16 = 20;
const G_TEXT: u16 = 21;
const G_BOXTEXT: u16 = 22;
const G_IMAGE: u16 = 23;
const G_IBOX: u16 = 25;
const G_FTEXT: u16 = 29;
const G_FBOXTEXT: u16 = 30;
const G_BUTTON: u16 = 26;
const G_BOXCHAR: u16 = 27;
const G_STRING: u16 = 28;
const G_ICON: u16 = 31;
const G_TITLE: u16 = 32;

/// `BITBLK` — a raw monochrome raster for a `G_IMAGE` object
/// (`obdefs.h`). `bi_pdata` is a native pointer to big-endian 16-bit
/// raster words.
#[repr(C)]
pub struct BitBlk {
    /// pointer to the mono raster data (16-bit words).
    pub bi_pdata: u64,
    /// width of one row in bytes.
    pub bi_wb: i16,
    /// height in rows.
    pub bi_hl: i16,
    /// source x within the raster.
    pub bi_x: i16,
    /// source y within the raster.
    pub bi_y: i16,
    /// foreground colour (PEL) of set bits.
    pub bi_color: i16,
}

/// `TEDINFO` — editable/formatted text for the `G_TEXT`/`G_BOXTEXT`
/// object types (`obdefs.h`). Pointers are native 64-bit.
#[repr(C)]
pub struct TedInfo {
    /// pointer to the text string.
    pub te_ptext: u64,
    /// pointer to the template string.
    pub te_ptmplt: u64,
    /// pointer to the validation string.
    pub te_pvalid: u64,
    /// font id.
    pub te_font: i16,
    /// reserved.
    pub te_junk1: i16,
    /// justification (0 = left, 1 = right, 2 = centre).
    pub te_just: i16,
    /// colour word (`gr_crack` encoding).
    pub te_color: i16,
    /// reserved.
    pub te_junk2: i16,
    /// border thickness.
    pub te_thickness: i16,
    /// text length.
    pub te_txtlen: i16,
    /// template length.
    pub te_tmplen: i16,
}

/// `ICONBLK` — a monochrome icon (mask + data + optional char + text)
/// for a `G_ICON` object (`obdefs.h`). Pointers are native 64-bit.
#[repr(C)]
pub struct IconBlk {
    /// pointer to the mask raster (16-bit words).
    pub ib_pmask: u64,
    /// pointer to the data raster (16-bit words).
    pub ib_pdata: u64,
    /// pointer to the icon's text string.
    pub ib_ptext: u64,
    /// colour/char word: `[15:12]` fg, `[11:8]` bg, `[7:0]` character.
    pub ib_char: i16,
    /// character x/y.
    pub ib_xchar: i16,
    /// character y.
    pub ib_ychar: i16,
    /// icon image rect x/y/w/h (relative to the object).
    pub ib_xicon: i16,
    /// icon y.
    pub ib_yicon: i16,
    /// icon width.
    pub ib_wicon: i16,
    /// icon height.
    pub ib_hicon: i16,
    /// text rect x/y/w/h (relative to the object).
    pub ib_xtext: i16,
    /// text y.
    pub ib_ytext: i16,
    /// text width.
    pub ib_wtext: i16,
    /// text height.
    pub ib_htext: i16,
}

// Flag / state bits.
const LASTOB: u16 = 0x0020;
const HIDETREE: u16 = 0x0080;
const SELECTED: u16 = 0x0001;
const DEFAULT: u16 = 0x0002;
const CHECKED: u16 = 0x0004;
const DISABLED: u16 = 0x0008;

/// 50 % dither used to grey-out DISABLED objects (EmuTOS `IP_4PATT`).
const STIPPLE_50: [u16; 2] = [0xAAAA, 0x5555];

// 3D-object flags (`ob_flags`, EmuTOS `CONF_WITH_3D_OBJECTS`).
const FL3DIND: u16 = 0x0200; // indicator (recessed)
const FL3DBAK: u16 = 0x0400; // background panel (recessed)
const FL3DACT: u16 = 0x0600; // activator / button (raised)
const FL3DMASK: u16 = 0x0600;

const NIL: i16 = -1;

// VDI default palette pens.
const WHITE: Pel = 0;
const BLACK: Pel = 1;
const GRAY_LT: Pel = 8; // EmuTOS LWHITE — the 3D object face
const GRAY_DK: Pel = 9; // EmuTOS LBLACK — the 3D shadow edge

// Text justification (`te_just`).
const TE_LEFT: i16 = 0;
const TE_RIGHT: i16 = 1;
const TE_CNTR: i16 = 2;

// ── dispatch entry points ───────────────────────────────────────────

/// `objc_draw` (42) — draw the subtree rooted at `OB_DRAWOB`, `OB_DEPTH`
/// levels deep, clipped to `OB_XCLIP..OB_HCLIP`. `addr_in[0]` = tree.
pub fn objc_draw(pb: &Aespb) {
    let tree = pb.addrin(0) as *const Object;
    let drawob = pb.intin(0);
    let depth = pb.intin(1);

    let Some(h) = host() else {
        pb.set_ret(0);
        return;
    };
    let Some(backend) = h.raster() else {
        pb.set_ret(0);
        return;
    };
    if tree.is_null() || drawob < 0 {
        pb.set_ret(0);
        return;
    }

    // Clip to OB_XCLIP/YCLIP/WCLIP/HCLIP (int_in[2..5]); the backend
    // intersects with the screen edges. EmuTOS `objc_draw` sets exactly
    // this VDI clip before drawing, so a partial (damaged-rectangle)
    // redraw does not spill outside the caller's region.
    let (cx, cy, cw, ch) = (pb.intin(2), pb.intin(3), pb.intin(4), pb.intin(5));
    let clip = ClipRect { xmn: cx, ymn: cy, xmx: cx + cw - 1, ymx: cy + ch - 1 };

    // Absolute origin of `drawob` = sum of its ancestors' x/y. EmuTOS
    // starts drawing at the parent offset; we compute it via objc_offset.
    let (ox, oy) = abs_offset(tree, drawob);
    // SAFETY: the GEM caller guarantees a valid tree array.
    unsafe {
        let ob = &*tree.add(drawob as usize);
        draw_tree(tree, drawob, depth, ox - ob.ob_x, oy - ob.ob_y, clip, backend);
    }
    pb.set_ret(1);
}

/// `objc_offset` (44) — the object's absolute screen x/y (sum of parent
/// offsets). `int_in[0]` = object; `OB_XOFF`/`OB_YOFF` = `int_out[1..2]`.
pub fn objc_offset(pb: &Aespb) {
    let tree = pb.addrin(0) as *const Object;
    let obj = pb.intin(0);
    if tree.is_null() || obj < 0 {
        pb.set_ret(0);
        return;
    }
    let (x, y) = abs_offset(tree, obj);
    pb.set_intout(1, x);
    pb.set_intout(2, y);
    pb.set_ret(1);
}

/// `objc_add` (40) — link object `child` (`int_in[1]`) as the last child of
/// `parent` (`int_in[0]`). `addr_in[0]` = tree. EmuTOS `ob_add`.
pub fn objc_add(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let parent = pb.intin(0);
    let child = pb.intin(1);
    if tree.is_null() || parent < 0 || child < 0 {
        pb.set_ret(0);
        return;
    }
    // SAFETY: valid tree (GEM contract).
    unsafe { ob_add(tree, parent, child) };
    pb.set_ret(1);
}

/// `objc_delete` (41) — unlink object `int_in[0]` from its parent's child
/// list (the ROOT cannot be deleted). `addr_in[0]` = tree. EmuTOS `ob_delete`.
pub fn objc_delete(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let obj = pb.intin(0);
    if tree.is_null() || obj < 0 {
        pb.set_ret(0);
        return;
    }
    // SAFETY: valid tree (GEM contract).
    let ok = unsafe { ob_delete(tree, obj) };
    pb.set_ret(ok);
}

/// `objc_order` (45) — move object `int_in[0]` to sibling position
/// `int_in[1]` (0 = front/head, -1 = back/tail). `addr_in[0]` = tree.
/// EmuTOS `ob_order`.
pub fn objc_order(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let obj = pb.intin(0);
    let newpos = pb.intin(1);
    if tree.is_null() || obj < 0 {
        pb.set_ret(0);
        return;
    }
    // SAFETY: valid tree (GEM contract).
    let ok = unsafe { ob_order(tree, obj, newpos) };
    pb.set_ret(if ok { 1 } else { 0 });
}

/// Link `child` as the last child of `parent` (EmuTOS `ob_add`).
///
/// # Safety
/// `tree` is a valid OBJECT array; `parent`/`child` are valid indices.
unsafe fn ob_add(tree: *mut Object, parent: i16, child: i16) {
    if parent == NIL || child == NIL {
        return;
    }
    // SAFETY: caller guarantees a valid tree; each field write touches one
    // node, so no overlapping `&mut` is formed.
    unsafe {
        (*tree.add(child as usize)).ob_next = parent; // last sibling → parent
        let lastkid = (*tree.add(parent as usize)).ob_tail;
        if lastkid == NIL {
            (*tree.add(parent as usize)).ob_head = child; // first child
        } else {
            (*tree.add(lastkid as usize)).ob_next = child; // append to the end
        }
        (*tree.add(parent as usize)).ob_tail = child;
    }
}

/// Unlink `obj` from its parent's child list (EmuTOS `ob_delete`). Returns 1
/// on success, 0 for the ROOT or a not-found object.
///
/// # Safety
/// `tree` is a valid OBJECT array.
unsafe fn ob_delete(tree: *mut Object, obj: i16) -> i16 {
    if obj == 0 {
        return 0; // can't delete the ROOT
    }
    // SAFETY: caller guarantees a valid tree.
    unsafe {
        let mut nextsib = (*tree.add(obj as usize)).ob_next;
        let parent = parent_of(tree, obj);
        if parent == NIL {
            return 0;
        }
        if (*tree.add(parent as usize)).ob_head == obj {
            if (*tree.add(parent as usize)).ob_tail == obj {
                nextsib = NIL; // the only child — clear head + tail
                (*tree.add(parent as usize)).ob_tail = NIL;
            }
            (*tree.add(parent as usize)).ob_head = nextsib;
        } else {
            let prev = get_prev(tree, parent, obj);
            if prev == NIL {
                return 0; // not in this chain
            }
            (*tree.add(prev as usize)).ob_next = nextsib;
            if (*tree.add(parent as usize)).ob_tail == obj {
                (*tree.add(parent as usize)).ob_tail = prev;
            }
        }
    }
    1
}

/// Move `mov_obj` to sibling position `new_pos` (0 = head, NIL = tail) among
/// its siblings (EmuTOS `ob_order`). Defensive over the reference: a parent
/// left empty after the delete, or a `new_pos` past the end, is handled
/// without dereferencing NIL (which the reference would).
///
/// # Safety
/// `tree` is a valid OBJECT array.
unsafe fn ob_order(tree: *mut Object, mov_obj: i16, new_pos: i16) -> bool {
    if mov_obj == 0 {
        return false; // the ROOT has no siblings
    }
    // SAFETY: caller guarantees a valid tree.
    unsafe {
        let parent = parent_of(tree, mov_obj);
        if parent == NIL {
            return false;
        }
        ob_delete(tree, mov_obj);
        let head = (*tree.add(parent as usize)).ob_head;
        if head == NIL {
            // parent is now empty → mov_obj becomes its sole child
            (*tree.add(mov_obj as usize)).ob_next = parent;
            (*tree.add(parent as usize)).ob_head = mov_obj;
            (*tree.add(parent as usize)).ob_tail = mov_obj;
            return true;
        }
        let mut chg_obj = head;
        if new_pos == 0 {
            (*tree.add(mov_obj as usize)).ob_next = chg_obj;
            (*tree.add(parent as usize)).ob_head = mov_obj;
        } else {
            if new_pos == NIL {
                chg_obj = (*tree.add(parent as usize)).ob_tail;
            } else {
                let mut ii = 1;
                while ii < new_pos {
                    let n = (*tree.add(chg_obj as usize)).ob_next;
                    if n == parent || n == NIL {
                        break; // clamp to the end of the sibling list
                    }
                    chg_obj = n;
                    ii += 1;
                }
            }
            let cn = (*tree.add(chg_obj as usize)).ob_next;
            (*tree.add(mov_obj as usize)).ob_next = cn;
            (*tree.add(chg_obj as usize)).ob_next = mov_obj;
        }
        if (*tree.add(mov_obj as usize)).ob_next == parent {
            (*tree.add(parent as usize)).ob_tail = mov_obj;
        }
    }
    true
}

/// Previous sibling of `obj` under `parent`, or `NIL` (EmuTOS `get_prev`).
///
/// # Safety
/// `tree` is a valid OBJECT array.
unsafe fn get_prev(tree: *const Object, parent: i16, obj: i16) -> i16 {
    // SAFETY: caller guarantees a valid tree.
    unsafe {
        let mut pobj = (*tree.add(parent as usize)).ob_head;
        if pobj == obj {
            return NIL;
        }
        loop {
            let nobj = (*tree.add(pobj as usize)).ob_next;
            if nobj == obj {
                return pobj;
            }
            if nobj == parent || nobj == NIL {
                return NIL;
            }
            pobj = nobj;
        }
    }
}

/// `objc_change` (47) — set `obj`'s state to `OB_NEWSTATE` (`int_in[6]`)
/// and, if `OB_REDRAW` (`int_in[7]`) is set, redraw that object.
/// `addr_in[0]` = tree.
pub fn objc_change(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let obj = pb.intin(0);
    let newstate = pb.intin(6) as u16;
    let redraw = pb.intin(7) != 0;
    if tree.is_null() || obj < 0 {
        pb.set_ret(0);
        return;
    }
    change(tree, obj, newstate, redraw);
    pb.set_ret(1);
}

// ── objc_sysvar (3-D look) ──────────────────────────────────────────────
//
// The 3-D look system variables (EmuTOS `ob_sysvar`). Defaults match NewTOS's
// actual rendering: a 2px bevel, grey (LWHITE = 8) faces, no text-move or
// colour-change on press (the bevel carries the pressed state). SET stores
// them and GET reports them; `just_draw` still renders with fixed constants,
// so a SET is recorded/reported but does not yet re-style the draw — a
// documented follow-up, like `menu_register`.
static IND_TXTMOVE: AtomicI16 = AtomicI16::new(0);
static IND_COLCHANGE: AtomicI16 = AtomicI16::new(0);
static ACT_TXTMOVE: AtomicI16 = AtomicI16::new(0);
static ACT_COLCHANGE: AtomicI16 = AtomicI16::new(0);
static IND_BUTCOL: AtomicI16 = AtomicI16::new(GRAY_LT as i16);
static ACT_BUTCOL: AtomicI16 = AtomicI16::new(GRAY_LT as i16);
static BACKGR_COL: AtomicI16 = AtomicI16::new(GRAY_LT as i16);

// objc_sysvar `which` selectors (obdefs.h).
const LK3DIND: i16 = 1;
const LK3DACT: i16 = 2;
const INDBUTCOL: i16 = 3;
const ACTBUTCOL: i16 = 4;
const BACKGRCOL: i16 = 5;
const AD3DVALUE: i16 = 6;
const ADJ3DSTD: i16 = 2; // NewTOS's bevel is 2px

/// `objc_sysvar` (48) — get (`int_in[0]` = 0) or set (1) a 3-D look variable
/// `int_in[1]` from values `int_in[2..3]`. GET returns the pair in
/// `int_out[1..2]`; `int_out[0]` = 1 (ok) / 0 (bad which or out-of-range
/// colour). EmuTOS `ob_sysvar`.
pub fn objc_sysvar(pb: &Aespb) {
    let (mode, which, in1, in2) = (pb.intin(0), pb.intin(1), pb.intin(2), pb.intin(3));
    let ncol = host()
        .and_then(|h| h.raster())
        .map(|r| r.num_colors() as i16)
        .unwrap_or(16);
    let ok = if mode != 0 {
        // SET (-1 in either value leaves that field unchanged).
        match which {
            LK3DIND => {
                if in1 != -1 {
                    IND_TXTMOVE.store(in1, Ordering::Relaxed);
                }
                if in2 != -1 {
                    IND_COLCHANGE.store(in2, Ordering::Relaxed);
                }
                true
            }
            LK3DACT => {
                if in1 != -1 {
                    ACT_TXTMOVE.store(in1, Ordering::Relaxed);
                }
                if in2 != -1 {
                    ACT_COLCHANGE.store(in2, Ordering::Relaxed);
                }
                true
            }
            INDBUTCOL if in1 < ncol => {
                IND_BUTCOL.store(in1, Ordering::Relaxed);
                true
            }
            ACTBUTCOL if in1 < ncol => {
                ACT_BUTCOL.store(in1, Ordering::Relaxed);
                true
            }
            BACKGRCOL if in1 < ncol => {
                BACKGR_COL.store(in1, Ordering::Relaxed);
                true
            }
            _ => false, // unknown which, or colour >= ws_ncolors
        }
    } else {
        // GET
        let (o1, o2, valid) = match which {
            LK3DIND => (IND_TXTMOVE.load(Ordering::Relaxed), IND_COLCHANGE.load(Ordering::Relaxed), true),
            LK3DACT => (ACT_TXTMOVE.load(Ordering::Relaxed), ACT_COLCHANGE.load(Ordering::Relaxed), true),
            INDBUTCOL => (IND_BUTCOL.load(Ordering::Relaxed), 0, true),
            ACTBUTCOL => (ACT_BUTCOL.load(Ordering::Relaxed), 0, true),
            BACKGRCOL => (BACKGR_COL.load(Ordering::Relaxed), 0, true),
            AD3DVALUE => (ADJ3DSTD, ADJ3DSTD, true),
            _ => (0, 0, false),
        };
        pb.set_intout(1, o1);
        pb.set_intout(2, o2);
        valid
    };
    pb.set_ret(if ok { 1 } else { 0 });
}

/// Set `obj`'s state and optionally redraw just that object. Shared by
/// `objc_change` and the Form Manager (to show a pressed button).
pub(crate) fn change(tree: *mut Object, obj: i16, newstate: u16, redraw: bool) {
    // SAFETY: valid tree (GEM contract); the caller owns the array.
    unsafe {
        (*tree.add(obj as usize)).ob_state = newstate;
    }
    if !redraw {
        return;
    }
    if let Some(backend) = host().and_then(|h| h.raster()) {
        let clip = ClipRect::full(backend.width(), backend.height());
        let (ox, oy) = abs_offset(tree, obj);
        // SAFETY: valid tree; depth 0 redraws only this object.
        unsafe {
            let ob = &*tree.add(obj as usize);
            draw_tree(tree, obj, 0, ox - ob.ob_x, oy - ob.ob_y, clip, backend);
        }
    }
}

// objc_edit kinds (`obdefs.h`).
const ED_INIT: i16 = 1;
const ED_CHAR: i16 = 2;
const ED_END: i16 = 3;

/// `objc_edit` (46) — edit the EDITABLE `TEDINFO` text field `obj`.
/// `int_in[1]` = character, `int_in[2]` = cursor index, `int_in[3]` =
/// kind (EDSTART/EDINIT/EDCHAR/EDEND). The new cursor index is returned
/// in `OB_ODX` (`int_out[1]`).
///
/// Batch-6 scope: plain in-place editing of `te_ptext` (insert printable,
/// backspace; cursor shown as a caret). The template (`te_ptmplt`) and
/// validation (`te_pvalid`) of formatted fields land in a later batch.
pub fn objc_edit(pb: &Aespb) {
    let tree = pb.addrin(0) as *const Object;
    let obj = pb.intin(0);
    if tree.is_null() || obj < 0 {
        pb.set_intout(1, pb.intin(2));
        pb.set_ret(0);
        return;
    }
    let idx = edit_field(tree, obj, pb.intin(1), pb.intin(2), pb.intin(3));
    pb.set_intout(1, idx);
    pb.set_ret(1);
}

/// Edit the EDITABLE `TEDINFO` field `obj` in place and return the new
/// cursor index. Shared by `objc_edit` (the trap) and the Form Manager's
/// modal loop. `kind` = EDINIT (cursor to end) / EDCHAR (insert/delete at
/// the cursor) / EDEND (finish, no caret). Redraws the field + caret.
pub(crate) fn edit_field(tree: *const Object, obj: i16, in_char: i16, mut idx: i16, kind: i16) -> i16 {
    // SAFETY: valid tree.
    let ob = unsafe { &*tree.add(obj as usize) };
    if ob.ob_spec == 0 {
        return idx;
    }
    // SAFETY: an editable field's ob_spec points at a TEDINFO.
    let ted = unsafe { &*(ob.ob_spec as *const TedInfo) };
    let txtlen = ted.te_txtlen.max(1);
    let text = ted.te_ptext as *mut u8;

    match kind {
        ED_INIT => idx = cstr_len_p(text) as i16,
        ED_CHAR => edit_char(text, txtlen, &mut idx, in_char),
        _ => {} // EDSTART / EDEND
    }

    // Redraw the field, then the caret (unless we are ending the edit).
    if let Some(backend) = host().and_then(|h| h.raster()) {
        let clip = ClipRect::full(backend.width(), backend.height());
        let (ox, oy) = abs_offset(tree, obj);
        // SAFETY: valid tree; depth 0 redraws only this field.
        unsafe { draw_tree(tree, obj, 0, ox - ob.ob_x, oy - ob.ob_y, clip, backend) };
        if kind != ED_END {
            // Caret: a 1-px black bar at the cursor cell, text height.
            let cy = oy + (ob.ob_height - crate::vdi::font::CELL_H) / 2;
            backend.fill_rect(ox + idx * crate::vdi::font::CELL_W, cy, 1, crate::vdi::font::CELL_H, BLACK, clip);
        }
    }
    idx
}

/// Apply one keystroke to the in-place text buffer at the cursor.
fn edit_char(text: *mut u8, txtlen: i16, idx: &mut i16, ch: i16) {
    let c = (ch & 0xff) as u8;
    let len = cstr_len_p(text) as i16;
    match c {
        0x08 | 0x7F => {
            // backspace: delete the char before the cursor
            if *idx > 0 {
                // SAFETY: shifting within the NUL-terminated buffer.
                unsafe {
                    let mut i = *idx - 1;
                    while i < len {
                        *text.add(i as usize) = *text.add((i + 1) as usize);
                        i += 1;
                    }
                }
                *idx -= 1;
            }
        }
        0x20..=0x7E => {
            // insert a printable character at the cursor
            if len < txtlen - 1 {
                // SAFETY: room exists (len < txtlen-1); shift incl. the NUL.
                unsafe {
                    let mut i = len;
                    while i >= *idx {
                        *text.add((i + 1) as usize) = *text.add(i as usize);
                        i -= 1;
                    }
                    *text.add(*idx as usize) = c;
                }
                *idx += 1;
            }
        }
        _ => {}
    }
}

/// Length of a NUL-terminated string at `p` (cap 256).
fn cstr_len_p(p: *const u8) -> usize {
    if p.is_null() {
        return 0;
    }
    // SAFETY: GEM contract — valid NUL-terminated buffer.
    unsafe {
        let mut n = 0;
        while n < 256 && *p.add(n) != 0 {
            n += 1;
        }
        n
    }
}

/// `objc_find` (43) — hit-test: index of the deepest visible, non-disabled
/// object under `(OB_MX, OB_MY)` within `OB_DEPTH` levels of `OB_DRAWOB`,
/// or `-1`. `addr_in[0]` = tree.
pub fn objc_find(pb: &Aespb) {
    let tree = pb.addrin(0) as *const Object;
    let startob = pb.intin(0);
    let depth = pb.intin(1);
    let mx = pb.intin(2);
    let my = pb.intin(3);
    if tree.is_null() || startob < 0 {
        pb.set_ret(NIL);
        return;
    }
    let (ox, oy) = abs_offset(tree, startob);
    // SAFETY: valid tree (GEM contract).
    let found = unsafe {
        let ob = &*tree.add(startob as usize);
        find_hit(tree, startob, depth, ox - ob.ob_x, oy - ob.ob_y, mx, my)
    };
    pb.set_ret(found);
}

/// Hit-test entry for internal callers (the Form Manager's modal loop):
/// deepest visible, enabled object under `(mx, my)` within `depth` levels
/// of `root`, or `NIL`. Mirrors `objc_find`.
///
/// # Safety
/// `tree` is a valid OBJECT array.
pub(crate) unsafe fn hit_test(
    tree: *const Object,
    root: i16,
    depth: i16,
    mx: i16,
    my: i16,
) -> i16 {
    // SAFETY: caller guarantees a valid tree.
    unsafe {
        let (ox, oy) = abs_offset(tree, root);
        let ob = &*tree.add(root as usize);
        find_hit(tree, root, depth, ox - ob.ob_x, oy - ob.ob_y, mx, my)
    }
}

/// Absolute screen rectangle `(x, y, w, h)` of `obj`.
pub(crate) fn abs_rect(tree: *const Object, obj: i16) -> (i16, i16, i16, i16) {
    let (x, y) = abs_offset(tree, obj);
    // SAFETY: valid tree.
    let ob = unsafe { &*tree.add(obj as usize) };
    (x, y, ob.ob_width, ob.ob_height)
}

/// First child of `parent` whose absolute rectangle contains `(mx, my)`,
/// scanning only direct children; `NIL` if none. Used for menu hit-tests.
pub(crate) fn child_at(tree: *const Object, parent: i16, mx: i16, my: i16) -> i16 {
    // SAFETY: valid tree; the sibling chain ends back at `parent`.
    unsafe {
        let mut c = (*tree.add(parent as usize)).ob_head;
        while c != NIL && c != parent {
            let (x, y, w, h) = abs_rect(tree, c);
            if mx >= x && mx < x + w && my >= y && my < y + h {
                return c;
            }
            c = (*tree.add(c as usize)).ob_next;
        }
    }
    NIL
}

/// Set or clear one `ob_state` bit on `obj` and redraw it. Thin wrapper
/// the Menu Manager shares for title/item highlighting.
pub(crate) fn set_state_bit(tree: *mut Object, obj: i16, bit: u16, set: bool) {
    // SAFETY: valid tree.
    let st = unsafe { (*tree.add(obj as usize)).ob_state };
    let new = if set { st | bit } else { st & !bit };
    change(tree, obj, new, true);
}

/// Draw the subtree rooted at `obj` (any object), `depth` levels deep, at
/// its absolute screen position. Used by the Menu Manager to draw the bar.
pub(crate) fn draw_obj(tree: *const Object, obj: i16, depth: i16, backend: &dyn RasterBackend) {
    let clip = ClipRect::full(backend.width(), backend.height());
    let (ox, oy) = abs_offset(tree, obj);
    // SAFETY: valid tree (GEM contract).
    unsafe {
        let ob = &*tree.add(obj as usize);
        draw_tree(tree, obj, depth, ox - ob.ob_x, oy - ob.ob_y, clip, backend);
    }
}

/// Draw a whole tree (used by the Form Manager after building an alert).
///
/// # Safety
/// `tree` is a valid OBJECT array; `backend` is the live screen.
pub(crate) unsafe fn draw_root(tree: *const Object, depth: i16, backend: &dyn RasterBackend) {
    let clip = ClipRect::full(backend.width(), backend.height());
    // SAFETY: valid tree; root is index 0, its ob_x/ob_y are absolute
    // (no parent), so the parent offset is 0.
    unsafe {
        draw_tree(tree, 0, depth, 0, 0, clip, backend);
    }
}

// ── tree helpers ────────────────────────────────────────────────────

/// Absolute screen origin of `obj` = sum of x/y from the root down. Walks
/// parents by scanning sibling chains (EmuTOS `ob_offset`).
fn abs_offset(tree: *const Object, obj: i16) -> (i16, i16) {
    let mut x = 0i16;
    let mut y = 0i16;
    let mut cur = obj;
    // SAFETY: GEM contract — the tree array is valid and the parent walk
    // terminates at the root (parent == NIL).
    unsafe {
        while cur != NIL {
            let ob = &*tree.add(cur as usize);
            x += ob.ob_x;
            y += ob.ob_y;
            cur = parent_of(tree, cur);
        }
    }
    (x, y)
}

/// `obj`'s parent index, or `NIL` for the root (EmuTOS `get_par`). Exposed
/// for the Form Manager's radio-button sibling sweep.
///
/// # Safety
/// `tree` is a valid OBJECT array (GEM contract).
pub(crate) unsafe fn parent(tree: *const Object, obj: i16) -> i16 {
    // SAFETY: forwarded from the caller's contract.
    unsafe { parent_of(tree, obj) }
}

/// Index of `obj`'s parent, or `NIL` for the root. The parent is the node
/// whose child chain reaches `obj` (TOS finds it via `ob_next` wrapping
/// back to the parent at the end of the sibling list).
///
/// # Safety
/// `tree` is a valid OBJECT array (GEM contract).
unsafe fn parent_of(tree: *const Object, obj: i16) -> i16 {
    // Follow ob_next from obj until it points to a node that lists obj as
    // a child range end — TOS sets the last sibling's ob_next to the
    // parent index. Walk ob_next until we find a node whose ob_tail chain
    // includes obj.
    unsafe {
        let mut last = obj;
        loop {
            let n = (*tree.add(last as usize)).ob_next;
            if n == NIL {
                return NIL; // root
            }
            // If n's tail is `last`, then n is the parent.
            if (*tree.add(n as usize)).ob_tail == last {
                return n;
            }
            last = n;
        }
    }
}

/// Recursively draw `obj` and up to `depth` levels of descendants. `(px,
/// py)` is the absolute origin of `obj`'s parent.
fn draw_tree(
    tree: *const Object,
    obj: i16,
    depth: i16,
    px: i16,
    py: i16,
    clip: ClipRect,
    backend: &dyn RasterBackend,
) {
    // SAFETY: valid tree.
    let ob = unsafe { &*tree.add(obj as usize) };
    if ob.ob_flags & HIDETREE != 0 {
        return;
    }
    let x = px + ob.ob_x;
    let y = py + ob.ob_y;
    just_draw(ob, x, y, clip, backend);

    if depth > 0 {
        let mut child = ob.ob_head;
        // SAFETY: valid tree; the sibling chain ends at ob_tail.
        unsafe {
            while child != NIL {
                draw_tree(tree, child, depth - 1, x, y, clip, backend);
                if child == ob.ob_tail {
                    break;
                }
                child = (*tree.add(child as usize)).ob_next;
            }
        }
    }
}

/// Render a single object by type at absolute `(x, y)` (Batch-1 types).
// The 8 AES "inside patterns" IP_HOLLOW(0)..IP_SOLID(7) as 4-word dithers
// (EmuTOS `vdi/vdi_fill.c` DITHER; IP_4PATT = 50 %).
const IP_PATTERNS: [[u16; 4]; 8] = [
    [0x0000, 0x0000, 0x0000, 0x0000], // IP_HOLLOW
    [0x0000, 0x4444, 0x0000, 0x1111],
    [0x0000, 0x5555, 0x0000, 0x5555],
    [0x8888, 0x5555, 0x2222, 0x5555],
    [0xAAAA, 0x5555, 0xAAAA, 0x5555], // IP_4PATT (50 %)
    [0xAAAA, 0xDDDD, 0xAAAA, 0x7777],
    [0xEEEE, 0xFFFF, 0xBBBB, 0xFFFF],
    [0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF], // IP_SOLID
];
const IP_HOLLOW: u16 = 0;
const IP_SOLID: u16 = 7;

/// Decode a GEM object colour word (EmuTOS `gr_crack`): returns
/// `(border, text, pattern, inside, replace_mode)`.
fn gr_crack(color: u16) -> (Pel, Pel, u16, Pel, bool) {
    (
        ((color >> 12) & 0x0f) as Pel, // border colour
        ((color >> 8) & 0x0f) as Pel,  // text colour
        (color >> 4) & 0x07,           // inside pattern
        (color & 0x0f) as Pel,         // inside colour
        color & 0x80 != 0,             // 1 = REPLACE, 0 = TRANSPARENT
    )
}

/// Complementary colour for a SELECTED object (EmuTOS `xor_color`, reduced to
/// the palette pairs NewTOS actually uses: white↔black, lwhite↔lblack, …).
fn xor_color(c: Pel) -> Pel {
    match c {
        WHITE => BLACK,
        BLACK => WHITE,
        _ => c ^ 1,
    }
}

/// Fill `(x,y,w,h)` with the AES inside pattern `ipat` in colour `icol`
/// (EmuTOS `gr_rect`): HOLLOW draws nothing, SOLID a flat fill, the rest a
/// dither of `icol` on white.
fn gr_rect(backend: &dyn RasterBackend, x: i16, y: i16, w: i16, h: i16, icol: Pel, ipat: u16, clip: ClipRect) {
    if w <= 0 || h <= 0 {
        return;
    }
    match ipat {
        IP_HOLLOW => {}
        IP_SOLID => backend.fill_rect(x, y, w, h, icol, clip),
        p => {
            let pat = PatternFill {
                fg: icol,
                bg: WHITE,
                pattern: &IP_PATTERNS[(p & 7) as usize],
                opaque: false,
            };
            backend.fill_pattern(x, y, w, h, pat, clip);
        }
    }
}

/// Maximum formatted-string length (EmuTOS `obdefs.h` `MAX_LEN`).
const MAX_LEN: usize = 81;

/// Merge a raw text string into a template (EmuTOS `ob_format`): each `_`
/// in the template consumes the next character of `raw` (blanked back to
/// `_` once `raw` is exhausted); every other template character is copied
/// verbatim. A leading `@` in `raw` means "empty". `just == TE_RIGHT`
/// fills from the right end. The formatted result (always template length,
/// capped at `out.len()`) is written into `out`; the length is returned.
fn ob_format(just: i16, raw: &[u8], tmpl: &[u8], out: &mut [u8]) -> usize {
    // A leading '@' marks an empty raw string (EmuTOS clears it to "").
    let raw: &[u8] = if raw.first() == Some(&b'@') { &[] } else { raw };
    let ptlen = tmpl.len().min(out.len());
    let prlen = raw.len();

    if just == TE_RIGHT {
        // Walk template and raw from their right ends.
        let mut r = prlen; // one past the next raw char to consume
        for k in 0..ptlen {
            let ti = ptlen - 1 - k;
            out[ti] = if tmpl[ti] != b'_' {
                tmpl[ti]
            } else if r > 0 {
                r -= 1;
                raw[r]
            } else {
                b'_'
            };
        }
    } else {
        let mut r = 0usize; // next raw char to consume
        for i in 0..ptlen {
            out[i] = if tmpl[i] != b'_' {
                tmpl[i]
            } else if r < prlen {
                let c = raw[r];
                r += 1;
                c
            } else {
                b'_'
            };
        }
    }
    ptlen
}

fn just_draw(ob: &Object, x: i16, y: i16, clip: ClipRect, backend: &dyn RasterBackend) {
    let selected = ob.ob_state & SELECTED != 0;
    let disabled = ob.ob_state & DISABLED != 0;
    let d3 = ob.ob_flags & FL3DMASK;

    // EmuTOS `just_draw` grows a 3D object by ADJ3DSTD (2px) on every side, so
    // its bevel sits in the margin *outside* the nominal rectangle instead of
    // eating the object's own edge (which shrank every 3D button/panel by 4px
    // and let the bevel overlap the border/text). Grow here, then everything
    // below — fill, border, text, bevel, DISABLED dither, DEFAULT frame —
    // draws against the grown rectangle.
    let (x, y, w, h) = if d3 != 0 {
        (x - ADJ3DSTD, y - ADJ3DSTD, ob.ob_width + 2 * ADJ3DSTD, ob.ob_height + 2 * ADJ3DSTD)
    } else {
        (x, y, ob.ob_width, ob.ob_height)
    };

    // Black-on-white look. SELECTED inverts fill/text. A 3D object keeps a
    // grey face (the pressed state shows via the bevel, drawn after).
    let (fill, ink) = if d3 != 0 {
        (GRAY_LT, BLACK)
    } else if selected {
        (BLACK, WHITE)
    } else {
        (WHITE, BLACK)
    };

    match ob.ob_type {
        G_BOX | G_IBOX | G_BUTTON | G_BOXCHAR => {
            // Crack the colour word: box types pack it in `ob_spec`'s low 16
            // bits (EmuTOS `gr_crack`); a G_BUTTON's `ob_spec` is a text
            // pointer, so it takes the standard black-border / white-interior
            // button colours. Border thickness is in `ob_spec` bits 16..23 for
            // the box types; a button uses 1px.
            let (bcol, tcol, mut ipat, mut icol, _mode) = if ob.ob_type == G_BUTTON {
                (BLACK, BLACK, IP_HOLLOW, WHITE, false)
            } else {
                gr_crack(ob.ob_spec as u16)
            };
            let th = if d3 != 0 {
                0 // 3D objects use a bevel (drawn after), not a flat border
            } else if ob.ob_type == G_BUTTON {
                1
            } else {
                ((ob.ob_spec >> 16) as u8 as i8) as i16
            };
            // 3D colour override (EmuTOS): a hollow-white box/button takes a
            // solid 3D face colour so the bevel reads against it.
            if ob.ob_type != G_IBOX && ipat == IP_HOLLOW && icol == WHITE && d3 != 0 {
                ipat = IP_SOLID;
                icol = match d3 {
                    FL3DACT => ACT_BUTCOL.load(Ordering::Relaxed) as Pel,
                    FL3DIND => IND_BUTCOL.load(Ordering::Relaxed) as Pel,
                    _ => BACKGR_COL.load(Ordering::Relaxed) as Pel, // FL3DBAK
                };
            }
            // SELECTED (EmuTOS): a hollow box goes solid black, else xor the
            // interior colour.
            if selected {
                if ipat == IP_HOLLOW {
                    ipat = IP_SOLID;
                    icol = BLACK;
                } else {
                    icol = xor_color(icol);
                }
            }
            let t = th.unsigned_abs() as i16;
            if th != 0 {
                draw_border(backend, x, y, w, h, t, bcol, clip);
            }
            if ob.ob_type != G_IBOX {
                gr_rect(backend, x + t, y + t, w - 2 * t, h - 2 * t, icol, ipat, clip);
            }
            // Text colour, xor'd when selected (EmuTOS).
            let ink = if selected { xor_color(tcol) } else { tcol };
            if ob.ob_type == G_BOXCHAR {
                let ch = ((ob.ob_spec >> 24) & 0xFF) as u8;
                if ch != 0 {
                    draw_text(backend, x, y, w, h, &[ch], TE_CNTR, ink, clip);
                }
            } else if ob.ob_type == G_BUTTON {
                let s = read_cstr(ob.ob_spec);
                draw_text(backend, x, y, w, h, s, TE_CNTR, ink, clip);
            }
        }
        G_STRING => {
            // Menu items / plain strings. SELECTED inverts (menu
            // highlight); CHECKED draws a check glyph at the left.
            if selected {
                backend.fill_rect(x, y, w, h, BLACK, clip);
            }
            let cw = crate::vdi::font::CELL_W;
            if ob.ob_state & CHECKED != 0 {
                backend.draw_glyph(
                    x,
                    y + (h - crate::vdi::font::CELL_H) / 2,
                    crate::vdi::font::glyph(0x08), // ST-font check mark
                    ink,
                    clip,
                );
            }
            let s = read_cstr(ob.ob_spec);
            // indent past the check column (2 cells), as GEM menu items do
            draw_text(backend, x + 2 * cw, y, w, h, s, TE_LEFT, ink, clip);
        }
        G_TITLE => {
            // A menu-bar title: text with a small left pad; SELECTED
            // (menu dropped) inverts to a highlighted look.
            backend.fill_rect(x, y, w, h, fill, clip);
            let s = read_cstr(ob.ob_spec);
            draw_text(backend, x + crate::vdi::font::CELL_W, y, w, h, s, TE_LEFT, ink, clip);
        }
        G_TEXT | G_BOXTEXT | G_FTEXT | G_FBOXTEXT => {
            if ob.ob_spec != 0 {
                // SAFETY: ob_spec of a *TEXT object points at a TEDINFO.
                let ted = unsafe { &*(ob.ob_spec as *const TedInfo) };
                // Crack the TEDINFO colour word (EmuTOS `gr_crack`): border,
                // text, inside pattern, inside colour.
                let (bcol, tcol, mut ipat, mut icol, _mode) = gr_crack(ted.te_color as u16);
                let mut is_box = ob.ob_type == G_BOXTEXT || ob.ob_type == G_FBOXTEXT;
                let mut th = if d3 != 0 { 0 } else { ted.te_thickness };
                // 3D background (EmuTOS): a hollow-white text background takes
                // the solid 3D background colour; a plain TEXT/FTEXT becomes a
                // BOXTEXT/FBOXTEXT with zero border so the fill is drawn.
                if d3 != 0 && ipat == IP_HOLLOW && icol == WHITE {
                    ipat = IP_SOLID;
                    icol = BACKGR_COL.load(Ordering::Relaxed) as Pel;
                    is_box = true;
                    th = 0;
                }
                // SELECTED (EmuTOS `changecol`): xor the interior + text.
                if selected {
                    if ipat == IP_HOLLOW {
                        ipat = IP_SOLID;
                        icol = BLACK;
                    } else {
                        icol = xor_color(icol);
                    }
                }
                let t = th.unsigned_abs() as i16;
                if is_box {
                    if th != 0 {
                        draw_border(backend, x, y, w, h, t, bcol, clip);
                    }
                    gr_rect(backend, x + t, y + t, w - 2 * t, h - 2 * t, icol, ipat, clip);
                }
                // Format the string: the F-variants merge `te_ptext` into the
                // `te_ptmplt` template (EmuTOS `ob_format`); the plain variants
                // draw `te_ptext` directly.
                let raw = read_cstr(ted.te_ptext);
                let mut fmtbuf = [0u8; MAX_LEN];
                let s: &[u8] = if ob.ob_type == G_FTEXT || ob.ob_type == G_FBOXTEXT {
                    let tmpl = read_cstr(ted.te_ptmplt);
                    let n = ob_format(ted.te_just, raw, tmpl, &mut fmtbuf);
                    &fmtbuf[..n]
                } else {
                    raw
                };
                // Text colour from te_color, xor'd when SELECTED (EmuTOS).
                let ink = if selected { xor_color(tcol) } else { tcol };
                // Text sits inside the box, shrunk by the border thickness.
                draw_text(backend, x + t, y + t, w - 2 * t, h - 2 * t, s, ted.te_just, ink, clip);
            }
        }
        G_IMAGE => {
            if ob.ob_spec != 0 {
                // SAFETY: ob_spec of a G_IMAGE points at a BITBLK.
                let bi = unsafe { &*(ob.ob_spec as *const BitBlk) };
                blit_image(backend, bi, x, y, clip);
            }
        }
        G_ICON => {
            if ob.ob_spec != 0 {
                // SAFETY: ob_spec of a G_ICON points at an ICONBLK.
                let ib = unsafe { &*(ob.ob_spec as *const IconBlk) };
                draw_icon(backend, ib, x, y, selected, clip);
            }
        }
        // Remaining types (TEDINFO text, colour icons, userdef) land in
        // later batches.
        _ => {}
    }

    // 3D bevel overlay (the modern / Falcon look): a raised activator
    // (unless pressed), a recessed indicator/background panel.
    match d3 {
        FL3DACT => add_3d_effect(backend, x, y, w, h, !selected, clip),
        FL3DIND | FL3DBAK => add_3d_effect(backend, x, y, w, h, false, clip),
        _ => {}
    }

    // DISABLED: grey the object out with a 50 % transparent dither
    // (EmuTOS `bb_fill(IP_4PATT)`), over the 3D-background colour for a
    // 3D panel, else white.
    if disabled {
        let fg = if d3 == FL3DBAK { GRAY_LT } else { WHITE };
        let pat = PatternFill { fg, bg: WHITE, pattern: &STIPPLE_50, opaque: false };
        backend.fill_pattern(x, y, w, h, pat, clip);
    }

    // DEFAULT object: a thick outline a few pixels outside it (the
    // "fat frame" around the default exit button).
    if ob.ob_flags & DEFAULT != 0 {
        draw_border(backend, x - 3, y - 3, w + 6, h + 6, 1, BLACK, clip);
        draw_border(backend, x - 4, y - 4, w + 8, h + 8, 1, BLACK, clip);
    }
}

/// Draw a 2-pixel bevel around a rectangle for the 3D look: `raised` gives
/// a white top/left highlight and a dark-grey bottom/right shadow;
/// `!raised` (recessed) swaps them. EmuTOS `add_3d_effect`.
fn add_3d_effect(backend: &dyn RasterBackend, x: i16, y: i16, w: i16, h: i16, raised: bool, clip: ClipRect) {
    if w < 2 || h < 2 {
        return;
    }
    let (tl, br) = if raised { (WHITE, GRAY_DK) } else { (GRAY_DK, WHITE) };
    for i in 0..2 {
        // top + left = highlight; bottom + right = shadow
        backend.fill_rect(x + i, y + i, w - 2 * i, 1, tl, clip);
        backend.fill_rect(x + i, y + i, 1, h - 2 * i, tl, clip);
        backend.fill_rect(x + i, y + h - 1 - i, w - 2 * i, 1, br, clip);
        backend.fill_rect(x + w - 1 - i, y + i, 1, h - 2 * i, br, clip);
    }
}

/// Draw a `t`-pixel border frame around `(x, y, w, h)` as four strips.
fn draw_border(
    backend: &dyn RasterBackend,
    x: i16,
    y: i16,
    w: i16,
    h: i16,
    t: i16,
    col: Pel,
    clip: ClipRect,
) {
    let t = t.max(1);
    backend.fill_rect(x, y, w, t, col, clip); // top
    backend.fill_rect(x, y + h - t, w, t, col, clip); // bottom
    backend.fill_rect(x, y, t, h, col, clip); // left
    backend.fill_rect(x + w - t, y, t, h, col, clip); // right
}

/// Draw `text` justified inside `(x, y, w, h)`, vertically centred, one
/// 8×16 glyph per byte.
fn draw_text(
    backend: &dyn RasterBackend,
    x: i16,
    y: i16,
    w: i16,
    h: i16,
    text: &[u8],
    just: i16,
    fg: Pel,
    clip: ClipRect,
) {
    let len = text.len() as i16;
    let tw = len * crate::vdi::font::CELL_W;
    let tx = match just {
        TE_CNTR => x + (w - tw) / 2,
        TE_RIGHT => x + w - tw,
        _ => x, // TE_LEFT
    };
    let ty = y + (h - crate::vdi::font::CELL_H) / 2;
    for (i, &c) in text.iter().enumerate() {
        let gx = tx + i as i16 * crate::vdi::font::CELL_W;
        backend.draw_glyph(gx, ty, crate::vdi::font::glyph(c as u16), fg, clip);
    }
}

/// Read a NUL-terminated C string from a native pointer in `ob_spec`
/// (trusted GEM pointer, like the other AES managers). Capped at 256.
fn read_cstr(spec: u64) -> &'static [u8] {
    if spec == 0 {
        return &[];
    }
    let p = spec as *const u8;
    // SAFETY: GEM contract — `ob_spec` of a string/button is a valid
    // pointer to a NUL-terminated string in the caller's memory.
    unsafe {
        let mut n = 0usize;
        while n < 256 && *p.add(n) != 0 {
            n += 1;
        }
        core::slice::from_raw_parts(p, n)
    }
}

/// Blit a `G_IMAGE` BITBLK at `(x, y)`: a transparent mono blit in
/// `bi_color` (set bits drawn, clear bits leave the background).
fn blit_image(backend: &dyn RasterBackend, bi: &BitBlk, x: i16, y: i16, clip: ClipRect) {
    let stride = (bi.bi_wb / 2).max(1) as u16; // bytes → 16-bit words
    blit_plane(
        backend,
        bi.bi_pdata,
        stride,
        bi.bi_x,
        bi.bi_y,
        Rect { x, y, w: bi.bi_wb * 8, h: bi.bi_hl },
        bi.bi_color as Pel,
        clip,
    );
}

/// Draw a `G_ICON` ICONBLK at object origin `(ox, oy)`: the mask plane in
/// the background colour, the data plane in the foreground colour, then
/// the character and text. SELECTED swaps fg/bg (EmuTOS `gr_gicon`).
fn draw_icon(
    backend: &dyn RasterBackend,
    ib: &IconBlk,
    ox: i16,
    oy: i16,
    selected: bool,
    clip: ClipRect,
) {
    let chword = ib.ib_char;
    let mut fg = ((chword >> 12) & 0x0f) as Pel;
    let mut bg = ((chword >> 8) & 0x0f) as Pel;
    if selected {
        core::mem::swap(&mut fg, &mut bg);
    }
    let stride = (ib.ib_wicon / 16).max(1) as u16; // icon width in words
    let rect = Rect {
        x: ox + ib.ib_xicon,
        y: oy + ib.ib_yicon,
        w: ib.ib_wicon,
        h: ib.ib_hicon,
    };
    blit_plane(backend, ib.ib_pmask, stride, 0, 0, rect, bg, clip);
    blit_plane(backend, ib.ib_pdata, stride, 0, 0, rect, fg, clip);

    let c = (chword & 0xff) as u8;
    if c != 0 {
        backend.draw_glyph(
            ox + ib.ib_xchar,
            oy + ib.ib_ychar,
            crate::vdi::font::glyph(c as u16),
            fg,
            clip,
        );
    }
    if ib.ib_ptext != 0 {
        let s = read_cstr(ib.ib_ptext);
        if !s.is_empty() {
            // Label background (EmuTOS `gr_gicon`): a solid `bg`-colour
            // rectangle behind the label so it reads against a patterned
            // desktop, and a SELECTED icon's label shows the swapped
            // (inverted) background. Then the label text in `fg`.
            gr_rect(
                backend,
                ox + ib.ib_xtext,
                oy + ib.ib_ytext,
                ib.ib_wtext,
                ib.ib_htext,
                bg,
                IP_SOLID,
                clip,
            );
            draw_text(
                backend,
                ox + ib.ib_xtext,
                oy + ib.ib_ytext,
                ib.ib_wtext,
                ib.ib_htext,
                s,
                TE_CNTR,
                fg,
                clip,
            );
        }
    }
}

/// Transparent mono blit of one bit-plane (`addr` → big-endian 16-bit
/// words, `stride` words per row) into `dst`, set bits drawn in `fg`.
fn blit_plane(
    backend: &dyn RasterBackend,
    addr: u64,
    stride: u16,
    src_x: i16,
    src_y: i16,
    dst: Rect,
    fg: Pel,
    clip: ClipRect,
) {
    if addr == 0 || dst.w <= 0 || dst.h <= 0 {
        return;
    }
    let rows = src_y.max(0) as usize + dst.h as usize;
    let total = stride as usize * rows;
    // SAFETY: GEM contract — `addr` points at >= `total` raster words.
    let bits = unsafe { core::slice::from_raw_parts(addr as *const u16, total) };
    backend.blit_mono(
        dst,
        MonoSrc {
            bits,
            stride_words: stride,
            x: src_x,
            y: src_y,
        },
        fg,
        WHITE,
        false,
        clip,
    );
}

/// Recursive hit-test for `objc_find`. Returns the deepest matching child
/// index, else `obj` if `(mx, my)` is inside it, else `NIL`.
fn find_hit(
    tree: *const Object,
    obj: i16,
    depth: i16,
    px: i16,
    py: i16,
    mx: i16,
    my: i16,
) -> i16 {
    // SAFETY: valid tree.
    let ob = unsafe { &*tree.add(obj as usize) };
    if ob.ob_flags & HIDETREE != 0 {
        return NIL;
    }
    let x = px + ob.ob_x;
    let y = py + ob.ob_y;
    let inside = mx >= x && mx < x + ob.ob_width && my >= y && my < y + ob.ob_height;
    if !inside {
        return NIL;
    }

    if depth > 0 {
        // Search children last-to-first (topmost sibling wins).
        let mut found = NIL;
        let mut child = ob.ob_head;
        // SAFETY: valid tree; chain terminates at ob_tail.
        unsafe {
            while child != NIL {
                let hit = find_hit(tree, child, depth - 1, x, y, mx, my);
                if hit != NIL {
                    found = hit; // keep the last (topmost) match
                }
                if child == ob.ob_tail {
                    break;
                }
                child = (*tree.add(child as usize)).ob_next;
            }
        }
        if found != NIL {
            return found;
        }
    }

    // Hit on `obj` itself — but DISABLED objects are not selectable.
    if ob.ob_state & DISABLED != 0 {
        NIL
    } else {
        obj
    }
}

// Keep LASTOB referenced (used by tree-building callers / future batches).
const _: u16 = LASTOB;
