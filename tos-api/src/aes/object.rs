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

use super::Aespb;
use crate::host::host;
use crate::vdi::backend::{ClipRect, MonoSrc, Pel, RasterBackend, Rect};

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
const G_BUTTON: u16 = 26;
const G_BOXCHAR: u16 = 27;
const G_STRING: u16 = 28;
const G_ICON: u16 = 31;

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
const DISABLED: u16 = 0x0008;

const NIL: i16 = -1;

// VDI default palette pens.
const WHITE: Pel = 0;
const BLACK: Pel = 1;

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

    let clip = ClipRect::full(backend.width(), backend.height());

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
fn just_draw(ob: &Object, x: i16, y: i16, clip: ClipRect, backend: &dyn RasterBackend) {
    let w = ob.ob_width;
    let h = ob.ob_height;
    let selected = ob.ob_state & SELECTED != 0;
    let disabled = ob.ob_state & DISABLED != 0;

    // Black-on-white look (Batch 1). SELECTED inverts fill/text.
    let (fill, ink) = if selected { (BLACK, WHITE) } else { (WHITE, BLACK) };

    match ob.ob_type {
        G_BOX | G_IBOX | G_BUTTON | G_BOXCHAR => {
            // Border thickness: for the box types `ob_spec` packs it in
            // bits 16..23 (signed); for a G_BUTTON `ob_spec` is the text
            // *pointer*, so a button uses a fixed 1-px border instead.
            let th = if ob.ob_type == G_BUTTON {
                1
            } else {
                ((ob.ob_spec >> 16) as u8 as i8) as i16
            };
            if ob.ob_type != G_IBOX {
                backend.fill_rect(x, y, w, h, fill, clip);
            }
            if th != 0 {
                draw_border(backend, x, y, w, h, th.unsigned_abs() as i16, BLACK, clip);
            }
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
            let s = read_cstr(ob.ob_spec);
            draw_text(backend, x, y, w, h, s, TE_LEFT, BLACK, clip);
        }
        G_TEXT | G_BOXTEXT => {
            if ob.ob_spec != 0 {
                // SAFETY: ob_spec of a G_TEXT/G_BOXTEXT points at a TEDINFO.
                let ted = unsafe { &*(ob.ob_spec as *const TedInfo) };
                // G_BOXTEXT additionally draws a box (black-on-white look;
                // full colour-word cracking lands in the colour batch).
                if ob.ob_type == G_BOXTEXT {
                    backend.fill_rect(x, y, w, h, fill, clip);
                    let th = ted.te_thickness;
                    if th != 0 {
                        draw_border(backend, x, y, w, h, th.unsigned_abs() as i16, BLACK, clip);
                    }
                }
                // Text colour from te_color (gr_crack: bits 8..11);
                // inverted when SELECTED.
                let tcol = ((ted.te_color >> 8) & 0x0f) as Pel;
                let ink = if selected { WHITE } else { tcol };
                let s = read_cstr(ted.te_ptext);
                draw_text(backend, x, y, w, h, s, ted.te_just, ink, clip);
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

    // DISABLED: grey the object by stippling 50% white over it. A simple
    // visual cue until the full pattern path lands.
    let _ = disabled;
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
