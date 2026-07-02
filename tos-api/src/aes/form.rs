//! AES Form Manager (`form_*`) — Batch 2: `form_alert` (52).
//!
//! `form_alert(defbut, "[icon][msg][buttons]")` builds an alert tree
//! internally, centres + draws it via the Object Manager, runs a modal
//! loop, and returns the 1-based button pressed. Spec:
//! [`docs/api/aes/object-form.md`]; layout extracted from EmuTOS
//! `aes/gemfmalt.c` (`fm_parse`/`fm_build`/`fm_alert`).
//!
//! **Batch-2 scope (documented limits, not stubs):** no alert **icon**
//! graphic yet (the `[N]` field is parsed but not drawn — icons land with
//! the icon batch); the modal loop handles **buttons + keyboard** (Enter
//! = default button, a digit = that button) and a mouse click; the full
//! `fm_do` (editable fields, radio groups) is a later batch; the screen
//! under the alert is not saved/restored (the alert stays drawn).

use super::object::{self, Object};
use super::Aespb;
use crate::host::host;
use crate::vdi::backend::{ClipRect, RasterBackend};

/// Alert colours (`vdi` palette indices).
const BLACK: crate::vdi::backend::Pel = 1;

/// The three 32×32 alert icons (EmuTOS `aes/gem.rsc` BITBLKs NOTEBB /
/// QUESTBB / STOPBB — the `[1]`/`[2]`/`[3]` fields of a `form_alert`
/// string). One `u32` per row, bit 0x8000_0000 = leftmost pixel; set bits
/// are drawn black (a transparent mono blit, like EmuTOS `G_IMAGE`).
const ALERT_ICONS: [[u32; 32]; 3] = [
    // 1 = NOTE (exclamation)
    [
        0x00000000, 0x00018000, 0x00024000, 0x00024000,
        0x00042000, 0x0005A000, 0x00099000, 0x000BD000,
        0x0013C800, 0x0017E800, 0x00266400, 0x002C3400,
        0x004C3200, 0x005C3A00, 0x009C3900, 0x00BC3D00,
        0x013C3C80, 0x017E7E80, 0x027E7E40, 0x02FE7F40,
        0x04FE7F20, 0x05FE7FA0, 0x09FFFF90, 0x0BFFFFD0,
        0x13FE7FC8, 0x17FC3FE8, 0x27FC3FE4, 0x2FFE7FF4,
        0x4FFFFFF2, 0x40000002, 0x7FFFFFFE, 0x00000000,
    ],
    // 2 = QUEST (question mark)
    [
        0x00000000, 0x7FFFFFFE, 0x40000002, 0x4FFFFFF2,
        0x2FFFFFF4, 0x27F83FE4, 0x17E01FE8, 0x13C00FC8,
        0x0BC307D0, 0x09E78790, 0x05FF87A0, 0x04FF8720,
        0x02FF0F40, 0x027E1E40, 0x017C3E80, 0x013C7C80,
        0x00BC7D00, 0x009FF900, 0x005E7A00, 0x004C3200,
        0x002C3400, 0x00266400, 0x0017E800, 0x0013C800,
        0x000BD000, 0x00099000, 0x0005A000, 0x00042000,
        0x00024000, 0x00024000, 0x00018000, 0x00000000,
    ],
    // 3 = STOP (stop sign)
    [
        0x00000000, 0x00000000, 0x003FFC00, 0x00400200,
        0x009FF900, 0x013FFC80, 0x027FFE40, 0x04FFFF20,
        0x09FFFF90, 0x13FFFFC8, 0x27EFF7E4, 0x2FC7E3F4,
        0x2FE3C7F4, 0x2FF18FF4, 0x2FF81FF4, 0x2FFC3FF4,
        0x2FFC3FF4, 0x2FF81FF4, 0x2FF18FF4, 0x2FE3C7F4,
        0x2FC7E3F4, 0x27EFF7E4, 0x13FFFFC8, 0x09FFFF90,
        0x04FFFF20, 0x027FFE40, 0x013FFC80, 0x009FF900,
        0x00400200, 0x003FFC00, 0x00000000, 0x00000000,
    ],
];

/// Blit a 32×32 alert icon (transparent, set bits in black) at `(x, y)`.
fn draw_alert_icon(icon: i16, x: i16, y: i16, backend: &dyn RasterBackend) {
    if icon < 1 || icon > 3 {
        return;
    }
    let clip = ClipRect::full(backend.width(), backend.height());
    let bmp = &ALERT_ICONS[(icon - 1) as usize];
    for (row, &bits) in bmp.iter().enumerate() {
        for col in 0..32i16 {
            if bits & (0x8000_0000u32 >> col) != 0 {
                backend.fill_rect(x + col, y + row as i16, 1, 1, BLACK, clip);
            }
        }
    }
}

const G_BOX: u16 = 20;
const G_BUTTON: u16 = 26;
const G_STRING: u16 = 28;

const SELECTABLE: u16 = 0x0001;
const DEFAULT: u16 = 0x0002;
const EXIT: u16 = 0x0004;
const EDITABLE: u16 = 0x0008;
const FL3DBAK: u16 = 0x0400; // 3D background panel
const FL3DACT: u16 = 0x0600; // 3D activator (button)
const LASTOB: u16 = 0x0020;
const TOUCHEXIT: u16 = 0x0040;
const SELECTED: u16 = 0x0001;
const MAX_DEPTH: i16 = 16;

// objc_edit kinds.
const ED_INIT: i16 = 1;
const ED_CHAR: i16 = 2;
const ED_END: i16 = 3;

const CW: i16 = 8; // system font cell width
const CH: i16 = 16; // system font cell height

const MAX_MSG: usize = 6;
const MAX_BTN: usize = 3;
const STRBUF: usize = 512;

/// `form_alert` (52). `int_in[0]` = default button (1-based, 0 = none),
/// `addr_in[0]` = the alert string. Returns the button pressed.
pub fn form_alert(pb: &Aespb) {
    let defbut = pb.intin(0);
    let palstr = pb.addrin(0) as *const u8;
    if palstr.is_null() {
        pb.set_ret(1);
        return;
    }
    // SAFETY: trusted GEM pointer to a NUL-terminated alert string.
    let s = unsafe { cstr_slice(palstr) };
    pb.set_ret(do_alert(defbut, s, true));
}

/// Draw an alert string (`"[icon][msg][buttons]"`) without running the
/// modal loop — for non-interactive showcases. Returns once drawn.
pub fn draw_alert(s: &[u8]) {
    do_alert(1, s, false);
}

/// Parse an alert string, build + draw the alert, optionally run the modal
/// loop, and return the 1-based button pressed (the default when `modal`
/// is false). Shared by `form_alert` / `form_error` / `draw_alert`.
fn do_alert(defbut: i16, s: &[u8], modal: bool) -> i16 {
    // ── parse the three [..] groups: icon, messages, buttons ──
    let mut buf = [0u8; STRBUF];
    let mut used = 0usize;
    let mut msg = [(0usize, 0i16); MAX_MSG]; // (offset, char-len)
    let mut btn = [(0usize, 0i16); MAX_BTN];

    let (g1s, g1e, p1) = group(s, 0); // icon number [0..3]
    let (g2s, g2e, p2) = group(s, p1);
    let (g3s, g3e, _p3) = group(s, p2);

    // Icon number (0 = none, 1 = NOTE, 2 = QUEST, 3 = STOP).
    let icon = if g1e > g1s && s[g1s].is_ascii_digit() {
        ((s[g1s] - b'0') as i16).clamp(0, 3)
    } else {
        0
    };

    let nummsg = split_copy(&s[g2s..g2e], &mut buf, &mut used, &mut msg);
    let numbut = split_copy(&s[g3s..g3e], &mut buf, &mut used, &mut btn).max(1);
    // If no buttons were given, default to a single localised "OK".
    if numbut == 1 && btn[0].1 == 0 {
        let ok = crate::i18n::sys_string(crate::i18n::Sys::Ok);
        btn[0] = (used, ok.len() as i16);
        buf[used..used + ok.len()].copy_from_slice(ok);
        used += ok.len() + 1;
    }

    let mlenmsg = msg.iter().take(nummsg).map(|m| m.1).max().unwrap_or(0);
    let mlenbut = btn.iter().take(numbut).map(|b| b.1).max().unwrap_or(2);

    // ── layout (pixels), centred on screen ──
    let Some(h) = host() else {
        return 1;
    };
    let Some(backend) = h.raster() else {
        return 1;
    };
    let sw = backend.width() as i16;
    let sh = backend.height() as i16;

    let padx = 2 * CW;
    let pady = CH;
    let btn_w = (mlenbut + 2) * CW;
    let btn_h = CH + 6;
    let gap = 2 * CW;

    // Icon column on the left (32-px icon + a one-char gap), EmuTOS
    // `fm_build`: the messages sit to its right, the box grows to fit.
    let icon_w = if icon != 0 { 32 + CW } else { 0 };
    let total_btn = numbut as i16 * btn_w + (numbut as i16 - 1) * gap;
    let content_w = (icon_w + mlenmsg * CW).max(total_btn);
    let box_w = content_w + 2 * padx;

    // Content height is the taller of the message block and the icon; the
    // messages are centred against it when the icon is taller.
    let msg_area_h = nummsg as i16 * CH;
    let content_h = msg_area_h.max(if icon != 0 { 32 } else { 0 });
    let msg_y0 = pady + (content_h - msg_area_h) / 2;
    let btn_y = pady + content_h + CH / 2; // gap below the content
    let box_h = btn_y + btn_h + pady;

    let box_x = (sw - box_w) / 2;
    let box_y = (sh - box_h) / 2;
    // Icon position (screen-absolute), vertically centred in the content.
    let icon_x = box_x + padx;
    let icon_y = box_y + pady + (content_h - 32) / 2;

    // ── build the tree: [0]=box, [1..nummsg]=strings, then buttons ──
    let nobj = 1 + nummsg + numbut;
    let mut tree = [Object {
        ob_next: -1, ob_head: -1, ob_tail: -1, ob_type: G_BOX,
        ob_flags: 0, ob_state: 0, ob_spec: 0,
        ob_x: 0, ob_y: 0, ob_width: 0, ob_height: 0,
    }; 1 + MAX_MSG + MAX_BTN];

    let first_btn = 1 + nummsg;
    // root box (absolute position) — 3D grey background panel
    tree[0] = Object {
        ob_next: -1, ob_head: 1, ob_tail: (nobj - 1) as i16, ob_type: G_BOX,
        ob_flags: FL3DBAK, ob_state: 0, ob_spec: 1 << 16,
        ob_x: box_x, ob_y: box_y, ob_width: box_w, ob_height: box_h,
    };
    // message strings
    for i in 0..nummsg {
        tree[1 + i] = Object {
            ob_next: (2 + i) as i16, ob_head: -1, ob_tail: -1, ob_type: G_STRING,
            ob_flags: 0, ob_state: 0, ob_spec: buf.as_ptr() as u64 + msg[i].0 as u64,
            ob_x: padx + icon_w, ob_y: msg_y0 + i as i16 * CH, ob_width: msg[i].1 * CW, ob_height: CH,
        };
    }
    // buttons
    let btn_x0 = (box_w - total_btn) / 2;
    for j in 0..numbut {
        let last = j == numbut - 1;
        tree[first_btn + j] = Object {
            ob_next: if last { 0 } else { (first_btn + j + 1) as i16 },
            ob_head: -1, ob_tail: -1, ob_type: G_BUTTON,
            ob_flags: SELECTABLE | EXIT | FL3DACT | if last { LASTOB } else { 0 },
            ob_state: 0, ob_spec: buf.as_ptr() as u64 + btn[j].0 as u64,
            ob_x: btn_x0 + j as i16 * (btn_w + gap), ob_y: btn_y,
            ob_width: btn_w, ob_height: btn_h,
        };
    }

    // ── draw + modal loop ──
    let tp = tree.as_ptr();
    // SAFETY: locally-built valid tree; backend is the live screen.
    unsafe { object::draw_root(tp, 16, backend) };
    draw_alert_icon(icon, icon_x, icon_y, backend);

    if !modal {
        return defbut.max(1); // draw-only (showcase): no waiting
    }
    let chosen = modal_loop(h, tp, first_btn as i16, numbut as i16, defbut);

    // Show the chosen button selected, then report it (1-based).
    if chosen >= 1 {
        let idx = first_btn + (chosen as usize - 1);
        tree[idx].ob_state |= SELECTED;
        // SAFETY: valid tree.
        unsafe { object::draw_root(tp, 16, backend) };
        draw_alert_icon(icon, icon_x, icon_y, backend);
    }
    chosen
}

/// Modal wait: a key (Enter = default button, '1'..='9' = that button) or
/// a left-mouse click on an `EXIT` button. Returns the 1-based button.
fn modal_loop(
    h: &dyn crate::host::Host,
    tree: *const Object,
    first_btn: i16,
    numbut: i16,
    defbut: i16,
) -> i16 {
    let mut prev_btn = h_mouse_buttons();
    loop {
        // keyboard
        if let Some(c) = h.console_try_read() {
            match c {
                b'\r' | b'\n' => {
                    if defbut >= 1 && defbut <= numbut {
                        return defbut;
                    }
                    return 1;
                }
                b'1'..=b'9' => {
                    let n = (c - b'0') as i16;
                    if n >= 1 && n <= numbut {
                        return n;
                    }
                }
                _ => {}
            }
        }
        // mouse: left-button press edge → hit-test
        let cur = h_mouse_buttons();
        if cur & 0x01 != 0 && prev_btn & 0x01 == 0 {
            let (mx, my) = crate::vdi::mouse_position();
            // SAFETY: valid tree.
            let hit = unsafe { object::hit_test(tree, 0, 16, mx, my) };
            if hit >= first_btn && hit < first_btn + numbut {
                return hit - first_btn + 1;
            }
        }
        prev_btn = cur;
        h.pump_input();
    }
}

fn h_mouse_buttons() -> u8 {
    crate::vdi::mouse_buttons()
}

// ── parsing helpers ─────────────────────────────────────────────────

/// A `&[u8]` view of the NUL-terminated string at `p` (cap [`STRBUF`]).
///
/// # Safety
/// `p` points at a NUL-terminated string the caller owns.
unsafe fn cstr_slice(p: *const u8) -> &'static [u8] {
    // SAFETY: GEM contract — valid NUL-terminated alert string.
    unsafe {
        let mut n = 0;
        while n < STRBUF && *p.add(n) != 0 {
            n += 1;
        }
        core::slice::from_raw_parts(p, n)
    }
}

/// Locate the next `[..]` group at/after `from`. Returns
/// `(content_start, content_end, after_close)`.
fn group(s: &[u8], from: usize) -> (usize, usize, usize) {
    let mut i = from;
    while i < s.len() && s[i] != b'[' {
        i += 1;
    }
    let start = if i < s.len() { i + 1 } else { s.len() };
    let mut j = start;
    while j < s.len() && s[j] != b']' {
        j += 1;
    }
    let after = if j < s.len() { j + 1 } else { s.len() };
    (start, j, after)
}

/// Split `seg` on `'|'` and copy each piece (NUL-terminated) into `buf`,
/// recording `(offset, char-len)`. Returns the number of pieces.
fn split_copy(
    seg: &[u8],
    buf: &mut [u8],
    used: &mut usize,
    out: &mut [(usize, i16)],
) -> usize {
    let mut count = 0;
    let mut start = 0;
    let mut i = 0;
    loop {
        let at_end = i >= seg.len();
        if at_end || seg[i] == b'|' {
            if count < out.len() {
                let piece = &seg[start..i];
                let off = *used;
                let n = piece.len().min(buf.len() - *used - 1);
                buf[off..off + n].copy_from_slice(&piece[..n]);
                buf[off + n] = 0;
                out[count] = (off, n as i16);
                *used += n + 1;
                count += 1;
            }
            if at_end {
                break;
            }
            start = i + 1;
        }
        i += 1;
    }
    count
}

/// `form_do` (50) — run a modal dialog on the tree at `addr_in[0]`
/// starting at edit object `int_in[0]`. Returns the index of the
/// `EXIT`/`TOUCHEXIT` object that ended the dialog.
///
/// **Batch-5 scope:** handles button clicks (`EXIT`/`TOUCHEXIT`) and the
/// keyboard (Enter = the `DEFAULT` button). Editable `TEDINFO` fields
/// (`objc_edit`) and full keyboard navigation land in a later batch.
pub fn form_do(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    if tree.is_null() {
        pb.set_ret(-1);
        return;
    }
    pb.set_ret(run_form(tree));
}

/// The modal loop: wait for a click on an `EXIT`/`TOUCHEXIT` object or
/// Enter (the default button); show it pressed and return its index.
fn run_form(tree: *mut Object) -> i16 {
    let def = find_flag(tree, DEFAULT);
    let Some(h) = host() else {
        return def;
    };
    let tp = tree as *const Object;

    // Focus the first editable field (cursor to its end).
    let mut focus = find_flag(tree, EDITABLE);
    let mut idx = 0i16;
    if focus >= 0 {
        idx = object::edit_field(tp, focus, 0, 0, ED_INIT);
    }

    let mut prev = crate::vdi::mouse_buttons();
    loop {
        if let Some(w) = h.console_try_read_key() {
            let c = (w & 0xff) as u8; // ascii (0 for arrow / function keys)
            match c {
                b'\r' | b'\n' => {
                    if def >= 0 {
                        press(tree, def);
                        return def;
                    }
                }
                b'\t' if focus >= 0 => {
                    // Tab: move to the next editable field.
                    object::edit_field(tp, focus, 0, idx, ED_END);
                    focus = next_editable(tree, focus);
                    idx = object::edit_field(tp, focus, 0, 0, ED_INIT);
                }
                _ if focus >= 0 => {
                    // Route the whole key word (scancode << 8 | ascii) to the
                    // field, so the arrow keys move the caret.
                    idx = object::edit_field(tp, focus, w as i16, idx, ED_CHAR);
                }
                _ => {}
            }
        }
        let cur = crate::vdi::mouse_buttons();
        if cur & 0x01 != 0 && prev & 0x01 == 0 {
            let (mx, my) = crate::vdi::mouse_position();
            // SAFETY: valid tree (GEM contract).
            let hit = unsafe { object::hit_test(tp, 0, MAX_DEPTH, mx, my) };
            if hit >= 0 {
                let (state, flags) = ob_fs(tp, hit);
                if flags & DISABLED != 0 {
                    // A disabled object ignores the click.
                } else if flags & SELECTABLE != 0 {
                    // EmuTOS `fm_button`: a radio button selects itself and
                    // deselects its siblings; any other SELECTABLE object
                    // toggles. Then wait for the button release.
                    let new_state = if flags & RBUTTON != 0 {
                        select_radio(tree, hit);
                        state | SELECTED
                    } else {
                        let ns = state ^ SELECTED;
                        object::change(tree, hit, ns, true);
                        ns
                    };
                    wait_up();
                    // A now-selected EXIT object (e.g. an OK button) leaves.
                    if new_state & SELECTED != 0 && flags & EXIT != 0 {
                        return hit;
                    }
                } else if flags & (EXIT | TOUCHEXIT) != 0 {
                    // A non-selectable exit/touchexit object leaves at once.
                    press(tree, hit);
                    return hit;
                } else if flags & EDITABLE != 0 && hit != focus {
                    // Click into another editable field → refocus it.
                    if focus >= 0 {
                        object::edit_field(tp, focus, 0, idx, ED_END);
                    }
                    focus = hit;
                    idx = object::edit_field(tp, hit, 0, 0, ED_INIT);
                }
            }
        }
        prev = cur;
        h.pump_input();
    }
}

/// The next `EDITABLE` field after `from` (wrapping), or `from` if none.
fn next_editable(tree: *mut Object, from: i16) -> i16 {
    let n = tree_len(tree);
    if n <= 0 {
        return from;
    }
    let mut i = from;
    for _ in 0..n {
        i = (i + 1) % n;
        // SAFETY: i is within [0, n).
        if unsafe { (*tree.add(i as usize)).ob_flags } & EDITABLE != 0 {
            return i;
        }
    }
    from
}

/// Number of objects in the flat tree array (up to and including LASTOB).
fn tree_len(tree: *mut Object) -> i16 {
    let mut i = 0i16;
    // SAFETY: the array terminates at the LASTOB object.
    unsafe {
        loop {
            if (*tree.add(i as usize)).ob_flags & LASTOB != 0 {
                return i + 1;
            }
            i += 1;
            if i > 256 {
                return i; // safety cap
            }
        }
    }
}

/// Show `obj` selected (pressed) and redraw it.
fn press(tree: *mut Object, obj: i16) {
    // SAFETY: valid index.
    let state = unsafe { (*tree.add(obj as usize)).ob_state } | SELECTED;
    object::change(tree, obj, state, true);
}

/// Index of the first object carrying `flag` (walking to `LASTOB`), or -1.
fn find_flag(tree: *mut Object, flag: u16) -> i16 {
    let mut i = 0i16;
    // SAFETY: the flat tree array terminates at the LASTOB object.
    unsafe {
        loop {
            let ob = &*tree.add(i as usize);
            if ob.ob_flags & flag != 0 {
                return i;
            }
            if ob.ob_flags & LASTOB != 0 {
                return -1;
            }
            i += 1;
        }
    }
}

/// `form_error` (53) — a canned alert for the GEMDOS error number in
/// `int_in[0]`. Builds `"[3]<localised message>[ OK ]"` and runs it as a
/// modal alert; returns the button (1 = OK). First real i18n use.
pub fn form_error(pb: &Aespb) {
    let errnum = pb.intin(0);
    let msg = crate::i18n::err_string(errnum);

    let mut buf = [0u8; 160];
    let mut n = 0usize;
    for &b in b"[3][" {
        buf[n] = b;
        n += 1;
    }
    for &b in msg {
        if n < buf.len() - 8 {
            buf[n] = b;
            n += 1;
        }
    }
    for &b in b"][ OK ]" {
        buf[n] = b;
        n += 1;
    }
    pb.set_ret(do_alert(1, &buf[..n], true));
}

// form_dial types (aesbind.h).
const FMD_START: i16 = 0;
const FMD_GROW: i16 = 1;
const FMD_SHRINK: i16 = 2;
const FMD_FINISH: i16 = 3;
/// ob_state OUTLINED bit (obdefs.h).
const OUTLINED: u16 = 0x0010;

/// `form_center` (54) — centre the tree at `addr_in[0]` on screen (EmuTOS
/// `ob_center`) and report its outer rectangle in `int_out[1..4]`.
pub fn form_center(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    if tree.is_null() {
        pb.set_ret(0);
        return;
    }
    let Some(backend) = host().and_then(|h| h.raster()) else {
        pb.set_ret(0);
        return;
    };
    let (sw, sh) = (backend.width() as i16, backend.height() as i16);
    // SAFETY: valid tree; the ROOT is index 0.
    let (mut xd, mut yd, mut wd, mut hd, outlined) = unsafe {
        let root = &mut *tree;
        let (wd, hd) = (root.ob_width, root.ob_height);
        let xd = (sw - wd) / 2;
        let yd = (sh - hd) / 2;
        root.ob_x = xd;
        root.ob_y = yd;
        (xd, yd, wd, hd, root.ob_state & OUTLINED != 0)
    };
    // Account for a 3px OUTLINE in the reported rectangle (EmuTOS ob_center).
    if outlined {
        xd = (xd - 3).max(0);
        yd = (yd - 3).max(0);
        wd += 6;
        hd += 6;
    }
    pb.set_intout(1, xd);
    pb.set_intout(2, yd);
    pb.set_intout(3, wd);
    pb.set_intout(4, hd);
    pb.set_ret(1);
}

/// `form_dial` (51) — reserve/animate/release the dialog screen area (EmuTOS
/// `fm_dial`). `int_in[0]` = type; `int_in[1..4]` = the small rect,
/// `int_in[5..8]` = the big rect.
pub fn form_dial(pb: &Aespb) {
    let fmd = pb.intin(0);
    let little = (pb.intin(1), pb.intin(2), pb.intin(3), pb.intin(4));
    let big = (pb.intin(5), pb.intin(6), pb.intin(7), pb.intin(8));
    match fmd {
        FMD_GROW => super::graf::animate(little, big),
        FMD_SHRINK => super::graf::animate(big, little),
        FMD_FINISH => super::window::redraw(), // repaint under the closed dialog
        // FMD_START: grab the screen for the dialog — no window rectangle list
        // to lock at our altitude, so a successful no-op.
        FMD_START => {}
        _ => {}
    }
    pb.set_ret(1);
}

// form_keybd navigation directions (EmuTOS fm_keybd).
const DEFLT: i16 = 0;
const FORWARD: i16 = 1;
const BACKWARD: i16 = 2;
// ob_flags RBUTTON (radio button); ob_state DISABLED; NIL sentinel.
const RBUTTON: u16 = 0x0010;
const DISABLED: u16 = 0x0008;
const NIL: i16 = -1;
// Keys routed by form_keybd (NewTOS ASCII low byte + scancode high byte).
const K_RETURN: i16 = 0x0D;
const K_TAB: i16 = 0x09;
const SC_UP: i16 = 0x48; // arrow-up scancode
const SC_DOWN: i16 = 0x50; // arrow-down scancode

/// Read an object's `(state, flags)` (EmuTOS `ob_fs`).
fn ob_fs(tree: *const Object, obj: i16) -> (u16, u16) {
    // SAFETY: valid tree (GEM contract).
    unsafe {
        let o = &*tree.add(obj as usize);
        (o.ob_state, o.ob_flags)
    }
}

/// Deselect the currently-selected radio sibling of `obj` and select `obj`
/// (EmuTOS `fm_button` RBUTTON path).
fn select_radio(tree: *mut Object, obj: i16) {
    // SAFETY: valid tree (GEM contract).
    unsafe {
        let parent = object::parent(tree as *const Object, obj);
        if parent < 0 {
            return;
        }
        let mut t = (*tree.add(parent as usize)).ob_head;
        while t != NIL && t != parent {
            let tflags = (*tree.add(t as usize)).ob_flags;
            let tstate = (*tree.add(t as usize)).ob_state;
            if tflags & RBUTTON != 0 && (tstate & SELECTED != 0 || t == obj) {
                let ns = if t == obj { tstate | SELECTED } else { tstate & !SELECTED };
                object::change(tree, t, ns, true);
            }
            t = (*tree.add(t as usize)).ob_next;
        }
    }
}

/// Previous EDITABLE field before `from` (wrapping), or `from` if none.
fn prev_editable(tree: *mut Object, from: i16) -> i16 {
    let n = tree_len(tree);
    if n <= 0 {
        return from;
    }
    let mut i = from;
    for _ in 0..n {
        i = (i - 1 + n) % n;
        // SAFETY: i is within [0, n).
        if unsafe { (*tree.add(i as usize)).ob_flags } & EDITABLE != 0 {
            return i;
        }
    }
    from
}

/// The object a `form_keybd` direction navigates to (EmuTOS `find_obj`).
fn find_obj(tree: *mut Object, obj: i16, direction: i16) -> i16 {
    match direction {
        DEFLT => {
            let d = find_flag(tree, DEFAULT);
            if d < 0 {
                0
            } else {
                d
            }
        }
        FORWARD => next_editable(tree, obj),
        BACKWARD => prev_editable(tree, obj),
        _ => obj,
    }
}

/// Block until the left mouse button is released (the `ev_button` up-wait
/// after a select).
fn wait_up() {
    if let Some(h) = host() {
        while crate::vdi::mouse_buttons() & 0x01 != 0 {
            h.pump_input();
        }
    }
}

/// `form_button` (56) — process a click on object `int_in[0]` (click count
/// `int_in[1]`) in the dialog at `addr_in[0]`. `int_out[1]` = the next object
/// (bit 15 set = double-click on a TOUCHEXIT); `int_out[0]` = continue (1) /
/// exit (0). EmuTOS `fm_button`.
pub fn form_button(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let obj = pb.intin(0);
    let clks = pb.intin(1);
    if tree.is_null() || obj < 0 {
        pb.set_intout(1, obj.max(0));
        pb.set_ret(1);
        return;
    }
    let (mut state, flags) = ob_fs(tree as *const Object, obj);
    let mut cont = true;
    let mut orword: u16 = 0;
    if flags & TOUCHEXIT != 0 {
        if clks == 2 {
            orword = 0x8000; // double-click indicator (high bit of the object)
        }
        cont = false;
    }
    if flags & SELECTABLE != 0 && state & DISABLED == 0 {
        if flags & RBUTTON != 0 {
            select_radio(tree, obj);
            state |= SELECTED;
        } else if super::graf::watchbox(tree, obj, state ^ SELECTED, state) {
            state ^= SELECTED;
        }
        if cont && flags & (SELECTABLE | EDITABLE) != 0 {
            wait_up();
        }
    }
    if state & SELECTED != 0 && flags & EXIT != 0 {
        cont = false;
    }
    let newobj = if cont && flags & EDITABLE == 0 { 0 } else { obj };
    pb.set_intout(1, ((newobj as u16) | orword) as i16);
    pb.set_ret(if cont { 1 } else { 0 });
}

/// `form_keybd` (55) — route a key `int_in[1]` in the dialog at `addr_in[0]`
/// (current object `int_in[0]`): RETURN → the DEFAULT button (exit),
/// TAB/down → next field, up → previous field. `int_out[1]` = the next
/// object, `int_out[2]` = the (possibly cleared) char; `int_out[0]` =
/// continue (1) / exit (0). EmuTOS `fm_keybd`.
pub fn form_keybd(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let obj = pb.intin(0);
    let thechar = pb.intin(1);
    if tree.is_null() {
        pb.set_intout(1, obj);
        pb.set_intout(2, thechar);
        pb.set_ret(1);
        return;
    }
    let ascii = thechar & 0xFF;
    let scan = (thechar >> 8) & 0xFF;
    let direction = if ascii == K_RETURN || ascii == 0x0A {
        DEFLT
    } else if ascii == K_TAB || scan == SC_DOWN {
        FORWARD
    } else if scan == SC_UP {
        BACKWARD
    } else {
        -1
    };
    let (mut newobj, mut newchar) = (obj, thechar);
    if direction != -1 {
        newchar = 0;
        newobj = find_obj(tree, obj, direction);
        if direction == DEFLT && newobj != 0 {
            press(tree, newobj);
            pb.set_intout(1, newobj);
            pb.set_intout(2, newchar);
            pb.set_ret(0); // the default button exits the dialog
            return;
        }
    }
    pb.set_intout(1, newobj);
    pb.set_intout(2, newchar);
    pb.set_ret(1);
}
