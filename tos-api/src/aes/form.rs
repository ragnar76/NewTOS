//! AES Form Manager (`form_*`) — Batch 2: `form_alert` (52).
//!
//! `form_alert(defbut, "[icon][msg][buttons]")` builds an alert tree
//! internally, centres + draws it via the Object Manager, runs a modal
//! loop, and returns the 1-based button pressed. Spec:
//! [`docs/api/aes/object-form.md`]; layout extracted from EmuTOS
//! `aes/gemfmalt.c` (`fm_parse`/`fm_build`/`fm_alert`).
//!
//! **Batch-2 scope (documented limits, not stubs):** the modal loop
//! handles **buttons + keyboard** (Enter = default button, a digit = that
//! button) and a mouse click; the full `fm_do` (editable fields, radio
//! groups) is a later batch. The modal `form_alert` now **saves and
//! restores the screen behind the alert** (EmuTOS `fm_alert`'s
//! `bb_save`/`bb_restore`, a `vro_cpyfm` Screen↔Memory copy over a heap
//! buffer, see [`crate::vdi::bb_save`]), so the desktop under it is
//! reinstated without a full repaint.

use super::object::{self, Object, ObjTree};
use super::Aespb;
use crate::host::host;
use crate::vdi::backend::{ClipRect, RasterBackend};

/// Alert colours (`vdi` palette indices).
const BLACK: crate::vdi::backend::Pel = 1;

/// The three 32×32 alert icons (EmuTOS `aes/gem.rsc` BITBLKs NOTEBB /
/// QUESTBB / STOPBB — the `[1]`/`[2]`/`[3]` fields of a `form_alert`
/// string). One `u32` per row, bit 0x8000_0000 = leftmost pixel; set bits
/// are drawn black (a transparent mono blit, like EmuTOS `G_IMAGE`).
///
/// **Verified byte-exact against `references/emutos/aes/gem.rsc` (AUD-P4-019):**
/// `rsh_bitblk` @ file offset 0x7CC holds 3 BITBLKs (`rsh_nbb == 3`); indices
/// 0/1/2 = NOTEBB/QUESTBB/STOPBB (order per `gem.def`), each `bi_wb == 4`,
/// `bi_hl == 32`, raster data at 0xEBE / 0xF3E / 0xFBE. Reading each 4-byte row
/// big-endian (leftmost pixel = MSB) reproduces the values below exactly, so
/// the embedded bitmaps match the reference RSC 1:1 — no runtime `gem.rsc` load
/// is needed.
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
///
/// `iconnum` is the raw `alert[1] - '0'` value (EmuTOS `fm_parse`): 0 draws no
/// icon; otherwise the EmuTOS `fm_build` switch selects the bitmap — 1 → NOTEBB,
/// 2 → QUESTBB, anything else (including negatives / >2) → STOPBB
/// (gemfmalt.c:260-270).
fn draw_alert_icon(iconnum: i16, x: i16, y: i16, backend: &dyn RasterBackend) {
    if iconnum == 0 {
        return;
    }
    let clip = ClipRect::full(backend.width(), backend.height());
    let idx = match iconnum {
        1 => 0usize, // NOTE
        2 => 1usize, // QUEST
        _ => 2usize, // STOP (switch default)
    };
    let bmp = &ALERT_ICONS[idx];
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
const HIDETREE: u16 = 0x0080; // ob_flags: subtree hidden
const SELECTED: u16 = 0x0001;
const MAX_DEPTH: i16 = 16;

// objc_edit kinds.
const ED_INIT: i16 = 1;
const ED_CHAR: i16 = 2;
const ED_END: i16 = 3;

const CW: i16 = 8; // system font cell width
const CH: i16 = 16; // system font cell height

// EmuTOS alert-string grammar limits (tools/erd.c:571-572 + generated
// gem_rsc.h): MAX_LINENUM=5 message lines, MAX_BUTNUM=3 buttons, MAX_LINELEN=40
// chars per message substring, MAX_BUTLEN=20 chars per button substring. A 6th
// line / an overlong substring is swallowed by `fm_strbrk` (gemfmalt.c:78-134).
const MAX_MSG: usize = 5; // EmuTOS MAX_LINENUM
const MAX_BTN: usize = 3; // EmuTOS MAX_BUTNUM
const MAX_LINELEN: usize = 40; // EmuTOS MAX_LINELEN (message substring cap)
const MAX_BUTLEN: usize = 20; // EmuTOS MAX_BUTLEN (button substring cap)
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

    let (g1s, _g1e, p1) = group(s, 0); // icon field ([1] = the raw char)
    let (g2s, g2e, p2) = group(s, p1);
    let (g3s, g3e, _p3) = group(s, p2);

    // Icon number = `alert[1] - '0'` (EmuTOS `fm_parse`, gemfmalt.c:163),
    // UNGUARDED: `s[g1s]` is the raw char right after the leading `[`. 0 = no
    // icon; any nonzero value draws one (1 → NOTE, 2 → QUEST, else → STOP, per
    // `fm_build`). A non-digit thus still draws an icon — e.g. `[ ]` gives
    // `' ' - '0' = -16` ≠ 0 → STOP; `[]` gives `']' - '0' = 45` ≠ 0 → STOP.
    let iconnum = if g1s < s.len() {
        (s[g1s] as i16) - (b'0' as i16)
    } else {
        0
    };

    // fm_strbrk caps each message substring at MAX_LINELEN (40) and each button
    // at MAX_BUTLEN (20), eating the overflow (gemfmalt.c:108-115). An empty
    // button group yields one empty button — EmuTOS draws it blank (it never
    // substitutes "OK"); `numbut` therefore stays >= 1 straight from fm_strbrk.
    let nummsg = split_copy(&s[g2s..g2e], MAX_LINELEN, &mut buf, &mut used, &mut msg);
    let numbut = split_copy(&s[g3s..g3e], MAX_BUTLEN, &mut buf, &mut used, &mut btn).max(1);

    let mlenmsg = msg.iter().take(nummsg).map(|m| m.1).max().unwrap_or(0);
    let mlenbut = btn.iter().take(numbut).map(|b| b.1).max().unwrap_or(0);

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
    let icon_w = if iconnum != 0 { 32 + CW } else { 0 };
    let total_btn = numbut as i16 * btn_w + (numbut as i16 - 1) * gap;
    let content_w = (icon_w + mlenmsg * CW).max(total_btn);
    let box_w = content_w + 2 * padx;

    // Content height is the taller of the message block and the icon; the
    // messages are centred against it when the icon is taller.
    let msg_area_h = nummsg as i16 * CH;
    let content_h = msg_area_h.max(if iconnum != 0 { 32 } else { 0 });
    let msg_y0 = pady + (content_h - msg_area_h) / 2;
    let btn_y = pady + content_h + CH / 2; // gap below the content
    let box_h = btn_y + btn_h + pady;

    let box_x = (sw - box_w) / 2;
    // Centre BELOW the menu bar (EmuTOS `ob_center`, gemobed.c:78:
    // `yd = gl_hbox + (gl_height - gl_hbox - hd) / 2`), consistent with our own
    // `form_center`. gl_hbox == window::GAD == 19 px (the menu BAR_H).
    let bar = super::window::GAD;
    let box_y = bar + (sh - bar - box_h) / 2;
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
    // root box (absolute position) — 3D grey background panel with the
    // classic GEM double frame (OUTLINED).
    tree[0] = Object {
        ob_next: -1, ob_head: 1, ob_tail: (nobj - 1) as i16, ob_type: G_BOX,
        ob_flags: FL3DBAK, ob_state: OUTLINED, ob_spec: 1 << 16,
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

    // Flag the default button (EmuTOS `fm_alert`, gemfmalt.c:343-347:
    // `tree[BUTOFF+defbut-1].ob_flags |= DEFAULT` after the tree is built) so it
    // draws with the heavier "this is Enter" frame (`ob_sst` gives a DEFAULT
    // button an extra -1 thickness; object.rs renders it). Guarded to a valid
    // 1-based button so an out-of-range `defbut` can never index past the tree.
    if defbut >= 1 && (defbut as usize) <= numbut {
        tree[first_btn + (defbut as usize - 1)].ob_flags |= DEFAULT;
    }

    // ── save the screen under the alert, draw, run, restore ──
    let tp = tree.as_ptr();
    // The alert array is a native (LP64, 32-byte) `Object` array built on this
    // kernel stack, so it must be read at the LP64 width even when an ILP32 app
    // (e.g. QED) is the current process and `current_abi()` would report ILP32.
    // Build one LP64 accessor and thread it through every draw / hit-test /
    // state helper this alert touches (`draw_root`, `hit_test_t`, `watchbox_t`).
    let t = ObjTree::lp64(tp);
    // Save the pixels the alert will cover — the box plus its 3-px OUTLINED
    // double frame (the extent EmuTOS `ob_center` reports for OUTLINED) —
    // so the modal `form_alert` can reinstate the desktop on close without a
    // full repaint (EmuTOS `fm_alert`: `bb_save` before draw, `bb_restore`
    // after). The draw-only showcase (`!modal`) skips this and leaves the
    // alert on screen deliberately.
    let saver = if modal {
        BbSave::capture(backend, box_x - 3, box_y - 3, box_w + 6, box_h + 6)
    } else {
        None
    };

    // Hide the cursor while drawing the alert so its save-under does not
    // capture (or get overdrawn by) alert pixels; show it afterwards with a
    // fresh backing. Otherwise the cursor would smear the alert as it moves,
    // and closing the box would leave a stale square behind.
    crate::vdi::cursor_hide(backend);
    object::draw_root(t, 16, backend);
    draw_alert_icon(iconnum, icon_x, icon_y, backend);
    crate::vdi::cursor_show(backend);

    if !modal {
        return defbut.max(1); // draw-only (showcase): no waiting
    }
    let chosen = modal_loop(h, tree.as_mut_ptr(), first_btn as i16, numbut as i16, defbut);

    // Show the chosen button selected (transient click feedback, as EmuTOS
    // `fm_do` does before returning). The bb_restore below then reinstates
    // the covered desktop.
    if chosen >= 1 {
        let idx = first_btn + (chosen as usize - 1);
        tree[idx].ob_state |= SELECTED;
        crate::vdi::cursor_hide(backend);
        object::draw_root(t, 16, backend);
        draw_alert_icon(iconnum, icon_x, icon_y, backend);
        crate::vdi::cursor_show(backend);
    }

    // Restore the screen the alert covered (EmuTOS `bb_restore`): the
    // desktop under the alert reappears from the saved bitmap, no full
    // redraw required. The buffer is freed when `saver` drops.
    if let Some(s) = saver {
        crate::vdi::cursor_hide(backend);
        s.restore(backend);
        crate::vdi::cursor_show(backend);
    }
    chosen
}

/// A saved copy of the screen rectangle behind a modal alert (AES
/// `bb_save`): a heap-allocated 32-bpp device-format bitmap plus its
/// (clamped) screen rectangle. [`restore`](Self::restore) blits it back;
/// the buffer is released when the value drops.
struct BbSave {
    buf: alloc::vec::Vec<u32>,
    x: i16,
    y: i16,
    w: i16,
    h: i16,
}

impl BbSave {
    /// Save the screen rectangle `(x, y, w, h)` — clamped to the visible
    /// screen — into a fresh device-format buffer (`vro_cpyfm`
    /// Screen→Memory). `None` when the clamped rectangle is empty.
    fn capture(backend: &dyn RasterBackend, x: i16, y: i16, w: i16, h: i16) -> Option<BbSave> {
        let sw = backend.width() as i32;
        let sh = backend.height() as i32;
        let x0 = (x as i32).max(0);
        let y0 = (y as i32).max(0);
        let x1 = (x as i32 + w as i32 - 1).min(sw - 1);
        let y1 = (y as i32 + h as i32 - 1).min(sh - 1);
        if x1 < x0 || y1 < y0 {
            return None;
        }
        let (cx, cy) = (x0 as i16, y0 as i16);
        let cw = (x1 - x0 + 1) as i16;
        let ch = (y1 - y0 + 1) as i16;
        let words = crate::vdi::bb_save_words(cw, ch);
        if words == 0 {
            return None;
        }
        let mut buf = alloc::vec![0u32; words];
        // SAFETY: `buf` holds `bb_save_words(cw, ch)` u32 words.
        unsafe { crate::vdi::bb_save(backend, buf.as_mut_ptr(), cx, cy, cw, ch) };
        Some(BbSave { buf, x: cx, y: cy, w: cw, h: ch })
    }

    /// Blit the saved bitmap back to the screen (`vro_cpyfm` Memory→Screen).
    fn restore(&self, backend: &dyn RasterBackend) {
        let clip = ClipRect::full(backend.width(), backend.height());
        // SAFETY: `buf` holds `bb_save_words(w, h)` u32 words (from capture).
        unsafe {
            crate::vdi::bb_restore(backend, self.buf.as_ptr(), self.x, self.y, self.w, self.h, clip);
        }
    }
}

/// Modal wait: Enter fires the DEFAULT button (only when one is set — EmuTOS
/// `fm_keybd`; no digit shortcuts), or a left-mouse click on a button. A mouse
/// press shows the button pressed and only fires on **release inside** it
/// (EmuTOS `gr_watchbox`); releasing outside cancels. Returns the 1-based button.
fn modal_loop(
    h: &dyn crate::host::Host,
    tree: *mut Object,
    first_btn: i16,
    numbut: i16,
    defbut: i16,
) -> i16 {
    let tp = tree as *const Object;
    // Kernel-built alert array: read it at the native LP64 width regardless of
    // the current app's ABI (see `do_alert`).
    let t = ObjTree::lp64(tp);
    let mut prev_btn = h_mouse_buttons();
    loop {
        // keyboard
        if let Some(c) = h.console_try_read() {
            match c {
                // EmuTOS fm_keybd RETURN → find_obj(DEFLT): Enter fires the
                // DEFAULT button only when one exists (defbut in [1..numbut]).
                // With none (defbut == 0) find_obj returns 0, cont stays TRUE
                // and the loop keeps waiting — Enter does nothing (gemfmalt.c:343
                // sets DEFAULT only when defbut != 0). A false guard falls
                // through to `_` (Enter ignored).
                b'\r' | b'\n' if defbut >= 1 && defbut <= numbut => return defbut,
                // EmuTOS `fm_keybd` handles no digit shortcuts — a digit is a
                // plain character routed to `objc_edit`, which an alert button
                // (not EDITABLE) drops. So '1'..'9' select no button (AUD-P4-015).
                _ => {}
            }
        }
        // mouse: left-button press edge → hit-test, then watch the button
        let cur = h_mouse_buttons();
        if cur & 0x01 != 0 && prev_btn & 0x01 == 0 {
            let (mx, my) = crate::vdi::mouse_position();
            let hit = object::hit_test_t(t, 0, 16, mx, my);
            if hit >= first_btn && hit < first_btn + numbut {
                // Show it pressed while held; act only on release inside.
                if super::graf::watchbox_t(t, hit, SELECTED, 0) {
                    return hit - first_btn + 1;
                }
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
///
/// A doubled `]]` inside the group is a **literal** `]` (EmuTOS `fm_strbrk`
/// `isduplicate`, gemfmalt.c:94), not the group terminator: both bytes are
/// skipped and scanning continues. The content span therefore still contains
/// the raw `]]`/`||` pairs; [`split_copy`] collapses them.
fn group(s: &[u8], from: usize) -> (usize, usize, usize) {
    let mut i = from;
    while i < s.len() && s[i] != b'[' {
        i += 1;
    }
    let start = if i < s.len() { i + 1 } else { s.len() };
    let mut j = start;
    while j < s.len() {
        if s[j] == b']' {
            if j + 1 < s.len() && s[j + 1] == b']' {
                j += 2; // literal ']]' — not the terminator
                continue;
            }
            break; // a lone ']' ends the group
        }
        j += 1;
    }
    let after = if j < s.len() { j + 1 } else { s.len() };
    (start, j, after)
}

/// Split `seg` into up to `out.len()` substrings, each capped at `maxlen`
/// characters, copying each NUL-terminated into `buf` and recording
/// `(offset, char-len)`. Returns the number of substrings.
///
/// Faithful to EmuTOS `fm_strbrk` (gemfmalt.c:78-134):
/// - `'|'` / `']'` / end-of-segment break a substring, but a doubled `'||'` /
///   `']]'` is a **literal** `|` / `]` that is copied and skipped, not a break
///   (`group` has already stopped the enclosing scan at the first lone `']'`,
///   so any `']'` here is the first byte of a `']]'` pair);
/// - a substring longer than `maxlen` is truncated at `maxlen` and its overflow
///   is eaten up to the next separator (the eat loop makes **no** literal
///   exception — matching EmuTOS, a `||` inside the overflow tail still breaks
///   the eating);
/// - the loop always yields at least one (possibly empty) substring — an empty
///   `seg` gives one zero-length piece (EmuTOS `*pnum = 1`, a blank button).
fn split_copy(
    seg: &[u8],
    maxlen: usize,
    buf: &mut [u8],
    used: &mut usize,
    out: &mut [(usize, i16)],
) -> usize {
    let maxnum = out.len();
    // EmuTOS endsubstring() = '|' | ']' | '\0'; endstring() = ']' | '\0'.
    let endsub = |i: usize| i >= seg.len() || seg[i] == b'|' || seg[i] == b']';
    let endstr = |i: usize| i >= seg.len() || seg[i] == b']';

    let mut count = 0usize;
    let mut i = 0usize;
    loop {
        if count >= maxnum {
            break;
        }
        let off = *used;
        let mut n = 0usize;
        // Copy up to `maxlen` output chars, breaking on an unescaped separator.
        while n < maxlen {
            if endsub(i) {
                // isduplicate(): a doubled separator is a literal, not a break.
                if i + 1 < seg.len() && seg[i + 1] == seg[i] {
                    i += 1; // skip one of the pair; the other is copied below
                } else {
                    break;
                }
            }
            if off + n + 1 < buf.len() {
                buf[off + n] = seg[i];
                n += 1;
            }
            i += 1;
        }
        if off + n < buf.len() {
            buf[off + n] = 0;
        }
        out[count] = (off, n as i16);
        *used += n + 1;
        count += 1;
        // Overlong substring: eat the rest up to the next separator (plain
        // endsubstring, no literal exception — EmuTOS gemfmalt.c:108-115).
        while !endsub(i) {
            i += 1;
        }
        if endstr(i) {
            break; // ']' or end-of-segment → no more substrings
        }
        i += 1; // skip the '|' separator
    }
    count
}

/// `form_do` (50) — run a modal dialog on the tree at `addr_in[0]` (FM_FORM)
/// starting at edit field `int_in[0]` (FM_START). Returns the index of the
/// `EXIT`/`TOUCHEXIT` object that ended the dialog (EmuTOS `fm_do`).
pub fn form_do(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    if tree.is_null() {
        pb.set_ret(-1);
        return;
    }
    // FM_START (int_in[0]) is the starting field, not ignored (EmuTOS
    // gemsuper.c:240 `fm_do(FM_FORM, FM_START)`).
    let start_fld = pb.intin(0);
    pb.set_ret(run_form(tree, start_fld));
}

/// The starting edit field for `fm_do` (EmuTOS `fm_inifld`, gemfmlib.c:153-160):
/// `start_fld` itself, or the first editable field forward from ROOT when it is
/// 0 (`find_obj(tree, 0, FORWARD)`).
fn fm_inifld(tree: *mut Object, start_fld: i16) -> i16 {
    if start_fld == 0 {
        find_obj(tree, 0, FORWARD)
    } else {
        start_fld
    }
}

/// The modal loop (EmuTOS `fm_do`, gemfmlib.c:278-353): position the cursor on
/// the starting field, then wait for keyboard / mouse events until an
/// `EXIT`/`TOUCHEXIT` object (or Enter on the DEFAULT button) ends the dialog.
/// Returns the object index that caused the exit (bit 15 set = double-click on a
/// TOUCHEXIT). Editable `TEDINFO` fields are edited in place through `ob_edit`.
///
/// Out of scope here (separate finding): `fm_own` screen/mouse ownership
/// (AUD-P4-011).
fn run_form(tree: *mut Object, start_fld: i16) -> i16 {
    let Some(h) = host() else {
        // Unreachable in the running kernel (the host is always present); report
        // the field we would have started on.
        return fm_inifld(tree, start_fld);
    };
    let tp = tree as *const Object;

    // Flush the keyboard before the loop (EmuTOS `fq()`, gemfmlib.c:290 →
    // geminput.c:272 — dequeue every buffered key), so stale type-ahead entered
    // before the dialog opened does not leak into it.
    while h.console_try_read_key().is_some() {}

    // Determine the starting field to edit (EmuTOS fm_inifld).
    let mut next_obj = fm_inifld(tree, start_fld);
    let mut edit_obj = 0i16;
    let mut idx = 0i16;
    let mut cont = true;
    let mut prev = crate::vdi::mouse_buttons();

    while cont {
        // Position the cursor on the selected editing field (EDINIT). ob_edit
        // ignores obj <= 0 (the ROOT is a box, not a TEDINFO), so guard it.
        if next_obj != 0 && edit_obj != next_obj {
            edit_obj = next_obj;
            next_obj = 0;
            if edit_obj > 0 {
                idx = object::edit_field(tp, edit_obj, 0, 0, ED_INIT);
            }
        }

        // ── keyboard event ─────────────────────────────────────────────
        if let Some(w) = h.console_try_read_key() {
            let thechar = w as i16; // scancode << 8 | ascii (EV_KRET)
            // fm_keybd handles RETURN/ARROW navigation; a residual character is
            // passed to the edit field (EmuTOS fm_do:320-325). `next_obj` is the
            // preinitialised *pnew_obj (left untouched by a plain character).
            let (c, newchar, no) = fm_keybd(tree, edit_obj, thechar, next_obj);
            cont = c;
            next_obj = no;
            if newchar != 0 && edit_obj > 0 {
                idx = object::edit_field(tp, edit_obj, newchar, idx, ED_CHAR);
            }
        }

        // ── button event ───────────────────────────────────────────────
        let cur = crate::vdi::mouse_buttons();
        if cont && cur & 0x01 != 0 && prev & 0x01 == 0 {
            let (mx, my) = crate::vdi::mouse_position();
            // SAFETY: valid tree (GEM contract).
            let hit = unsafe { object::hit_test(tp, 0, MAX_DEPTH, mx, my) };
            if hit == NIL {
                // Click into empty space: ring the bell and select no object
                // (EmuTOS fm_do, gemfmlib.c:332-335 `Bconout(2, 0x07)`).
                crate::bios::chardev::bconout_handle(2, 0x07);
                next_obj = 0;
            } else {
                // The click count only matters for a TOUCHEXIT object (fm_button
                // ORs 0x8000 on a double click); resolve single-vs-double only
                // there, within the double-click window (EmuTOS fm_do waits on
                // ev_multi `0x0002ff01L` = up to 2 left clicks). Ordinary buttons
                // keep their immediate response — EmuTOS gets the count for free
                // from the continuous mouse driver; we resolve it on demand.
                let (_, flags) = ob_fs(tp, hit);
                let clks = if flags & TOUCHEXIT != 0 {
                    super::event::count_form_clicks(h)
                } else {
                    1
                };
                let (c, no) = fm_button(tree, hit, clks);
                cont = c;
                next_obj = no;
            }
        }

        // Handle end-of-field clean up (EDEND) when leaving or switching fields.
        if (!cont || (next_obj != 0 && next_obj != edit_obj)) && edit_obj > 0 {
            object::edit_field(tp, edit_obj, 0, idx, ED_END);
        }

        prev = cur;
        h.pump_input();
    }

    next_obj
}

/// EmuTOS `fm_keybd` (gemfmlib.c:163-198): route the key word `thechar`
/// (`scancode << 8 | ascii`) for the current edit object `obj`. Returns
/// `(cont, newchar, newobj)`: `cont` = keep the dialog running (`false` = the
/// DEFAULT button exited it), `newchar` = the character left for the edit field
/// (0 when a navigation key was consumed), `newobj` = the field to focus / the
/// exiting object. `newobj` is preinitialised by the caller (FM_INXTOB, or the
/// running `next_obj`) and is left untouched for a non-navigation key.
fn fm_keybd(tree: *mut Object, obj: i16, thechar: i16, newobj: i16) -> (bool, i16, i16) {
    let ascii = thechar & 0xFF;
    let scan = (thechar >> 8) & 0xFF;
    // RETURN/ENTER reset obj to 0 and look for the DEFAULT return field;
    // ARROW_UP navigates backward, TAB / ARROW_DOWN forward.
    let (obj, direction) = if ascii == K_RETURN || ascii == 0x0A {
        (0, DEFLT)
    } else if scan == SC_UP {
        (obj, BACKWARD)
    } else if ascii == K_TAB || scan == SC_DOWN {
        (obj, FORWARD)
    } else {
        (obj, -1)
    };

    let mut newchar = thechar;
    let mut newobj = newobj;
    if direction != -1 {
        newchar = 0;
        newobj = find_obj(tree, obj, direction);
        // The DEFAULT button (if any) is selected and exits the dialog.
        if direction == DEFLT && newobj != 0 {
            press(tree, newobj);
            return (false, newchar, newobj);
        }
    }
    (true, newchar, newobj)
}

/// EmuTOS `find_obj` (gemfmlib.c:114-150): the next EDITABLE field forward /
/// backward from `start_obj`, or the DEFAULT return field. **No wrap** — the
/// scan runs until LASTOB (or an index below 0) and returns `start_obj` when
/// nothing matches; HIDETREE (`ob_flags`) and DISABLED (`ob_state`) objects are
/// skipped. `which` is [`FORWARD`], [`BACKWARD`] or [`DEFLT`].
fn find_obj(tree: *mut Object, start_obj: i16, which: i16) -> i16 {
    // App-provided tree at `addr_in[0]`: address each record at the caller's ABI
    // width (24-byte ILP32 / 32-byte LP64) via `ObjTree::new`.
    let t = ObjTree::new(tree as *const Object);
    let (mut obj, flag, inc) = match which {
        BACKWARD => (start_obj - 1, EDITABLE, -1i16),
        FORWARD => (start_obj + 1, EDITABLE, 1i16),
        // DEFLT: scan from ROOT for the DEFAULT-flagged object.
        _ => (0, DEFAULT, 1i16),
    };

    while obj >= 0 {
        let theflag = t.flags(obj);
        let state = t.state(obj);
        if theflag & HIDETREE == 0 && state & DISABLED == 0 && theflag & flag != 0 {
            return obj;
        }
        if theflag & LASTOB != 0 {
            obj = -1;
        } else {
            obj += inc;
        }
    }
    start_obj
}

/// EmuTOS `fm_button` (gemfmlib.c:201-270): process a click on `new_obj`
/// (`clks` presses) in the dialog. Returns `(cont, newobj)` where `cont` =
/// keep the dialog running and `newobj` = the resulting object (bit 15 set =
/// double-click on a TOUCHEXIT). Shared by the `form_button` trap and
/// `run_form`.
fn fm_button(tree: *mut Object, new_obj: i16, clks: i16) -> (bool, i16) {
    let (mut state, flags) = ob_fs(tree as *const Object, new_obj);
    let mut cont = true;
    let mut orword: u16 = 0;

    // TOUCHEXIT: a double click sets the high bit; either way it exits.
    if flags & TOUCHEXIT != 0 {
        if clks == 2 {
            orword = 0x8000;
        }
        cont = false;
    }

    // SELECTABLE (and enabled): a radio button selects itself + deselects its
    // siblings; any other is watched while held (commit on release inside).
    if flags & SELECTABLE != 0 && state & DISABLED == 0 {
        if flags & RBUTTON != 0 {
            select_radio(tree, new_obj);
            state |= SELECTED;
        } else if super::graf::watchbox(tree, new_obj, state ^ SELECTED, state) {
            state ^= SELECTED;
        }
        // If not a touchexit, wait for the button to come up.
        if cont && flags & (SELECTABLE | EDITABLE) != 0 {
            wait_up();
        }
    }

    // A selected EXIT object ends the dialog.
    if state & SELECTED != 0 && flags & EXIT != 0 {
        cont = false;
    }

    // A click that continues and is not an editable field selects no object.
    let new_obj = if cont && flags & EDITABLE == 0 { 0 } else { new_obj };
    (cont, ((new_obj as u16) | orword) as i16)
}

/// Show `obj` selected (pressed) and redraw it (EmuTOS `ob_change(…SELECTED…)`).
fn press(tree: *mut Object, obj: i16) {
    // App-provided tree: read `ob_state` at the caller's ABI width.
    let t = ObjTree::new(tree as *const Object);
    let state = t.state(obj) | SELECTED;
    object::change(tree, obj, state, true);
}

/// `form_error` (53) — the canned alert for the **positive DOS error number**
/// in `int_in[0]` (EmuTOS `fm_error`, gemfmlib.c:436-472; the error number is
/// a positive DOS code, e.g. the desktop's `form_error(2)` = "file not found").
///
/// EmuTOS maps `n` through a `switch` to a gem.rsc AL*ERR alert (each with its
/// own icon + buttons), shows it via `fm_show(string, …, level=1)`, and returns
/// `(button != 1)` — so dismissing the (single-button) alert yields `FALSE`(0).
/// `n > 63` is out of range: return `FALSE` without drawing anything.
pub fn form_error(pb: &Aespb) {
    let n = pb.intin(0);

    // EmuTOS `if (n > 63) return FALSE;` — no alert for an out-of-range code.
    if n > 63 {
        pb.set_ret(0);
        return;
    }

    use crate::i18n::ErrAlert;
    // EmuTOS `switch (n)` → the AL*ERR resource string (gemfmlib.c:443-467).
    let class = match n {
        2 | 18 | 3 => ErrAlert::NotFound, // AL18ERR
        4 => ErrAlert::NoRoom,            // AL04ERR
        5 => ErrAlert::Exists,            // AL05ERR
        15 => ErrAlert::NoDrive,          // AL15ERR
        8 | 10 | 11 => ErrAlert::NoMem,   // AL08ERR
        _ => ErrAlert::Generic,           // ALXXERR (with the number in %u)
    };

    // fm_show(string, (string == ALXXERR) ? &n : 0, level = 1). Only the
    // generic ALXXERR carries a `%u` to fill with the error number.
    let button = match class {
        ErrAlert::Generic => {
            let mut buf = [0u8; 64];
            let len = format_err(crate::i18n::err_alert(class), n, &mut buf);
            do_alert(1, &buf[..len], true)
        }
        _ => do_alert(1, crate::i18n::err_alert(class), true),
    };

    // EmuTOS `return (ret != 1)`: clicking button 1 yields FALSE(0).
    pb.set_ret(i16::from(button != 1));
}

/// Substitute the first `%u`/`%d` in the alert template `tmpl` with `n` in
/// decimal, into `buf`; returns the byte length written. Reproduces EmuTOS
/// `fm_show`'s `sprintf(alert_str, ad_alert, *pwd)` for the `ALXXERR`
/// ("TOS error #%u.") generic error string.
fn format_err(tmpl: &[u8], n: i16, buf: &mut [u8]) -> usize {
    let mut out = 0usize;
    let mut i = 0usize;
    while i < tmpl.len() {
        if tmpl[i] == b'%' && i + 1 < tmpl.len() && matches!(tmpl[i + 1], b'u' | b'd') {
            out += write_decimal(n, &mut buf[out..]);
            i += 2;
        } else {
            if out < buf.len() {
                buf[out] = tmpl[i];
                out += 1;
            }
            i += 1;
        }
    }
    out
}

/// Write `n` (as an unsigned 16-bit value, like C `%u`) in decimal into `buf`;
/// returns the number of bytes written.
fn write_decimal(n: i16, buf: &mut [u8]) -> usize {
    let mut v = n as u16;
    if v == 0 {
        if !buf.is_empty() {
            buf[0] = b'0';
        }
        return 1;
    }
    let mut tmp = [0u8; 5]; // u16 has at most 5 decimal digits
    let mut len = 0usize;
    while v > 0 {
        tmp[len] = b'0' + (v % 10) as u8;
        v /= 10;
        len += 1;
    }
    for k in 0..len {
        if k < buf.len() {
            buf[k] = tmp[len - 1 - k];
        }
    }
    len
}

// form_dial types (aesbind.h).
const FMD_START: i16 = 0;
const FMD_GROW: i16 = 1;
const FMD_SHRINK: i16 = 2;
const FMD_FINISH: i16 = 3;
/// ob_state OUTLINED / SHADOWED bits (obdefs.h).
const OUTLINED: u16 = 0x0010;
const SHADOWED: u16 = 0x0020;

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
    // App-provided tree at `addr_in[0]`: read/write the ROOT (index 0) through
    // the caller-ABI `ObjTree` accessor so an ILP32 app's 24-byte OBJECT record
    // is addressed at its own field offsets, not the native LP64 ones. For an
    // LP64/kernel caller `current_abi()` is `Lp64`, so the offsets are
    // byte-identical to the previous `*mut Object` access.
    let t = ObjTree::new(tree as *const Object);
    let (mut wd, mut hd) = (t.width(0), t.height(0));
    let mut xd = (sw - wd) / 2;
    // Centre BELOW the menu bar (EmuTOS ob_center, gemobed.c:76-80:
    // `yd = gl_hbox + (gl_height - gl_hbox - hd) / 2`), so a centred dialog
    // never lands under the menu bar. gl_hbox = gl_hchar + 3 = 19 pixels
    // (== window::GAD == the menu BAR_H).
    let bar = super::window::GAD;
    let mut yd = bar + (sh - bar - hd) / 2;
    t.set_x(0, xd);
    t.set_y(0, yd);
    let state = t.state(0);
    // Account for a 3px OUTLINE in the reported rectangle (EmuTOS ob_center):
    // the OUTLINED double frame reaches 3 px past the object on every side.
    if state & OUTLINED != 0 {
        xd = (xd - 3).max(0);
        yd = (yd - 3).max(0);
        wd += 6;
        hd += 6;
    }
    // Account for a SHADOWED drop shadow (EmuTOS ob_center, gemobed.c:96-103):
    // the reported rect grows by 2*|th|, where `th` is the ROOT's `ob_sst`
    // border thickness — determined per object type (G_TITLE / the G_TEXT
    // family / the G_BOX family / G_BUTTON), not just the box spec byte, exactly
    // as EmuTOS `ob_sst` (gemobjop.c:41-70) supplies it to `ob_center`.
    if state & SHADOWED != 0 {
        let th = t.sst_th(0);
        let s = 2 * th.abs();
        wd += s;
        hd += s;
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
        // EmuTOS `fm_dial` FMD_FINISH (gemfmlib.c:375-379): `w_drawdesk(pt)` +
        // `w_update(DESKWH, pt, …)` — repaint only the `pt` (big) rectangle the
        // dialog covered and fan WM_REDRAW to overlapping windows, not the whole
        // desktop.
        FMD_FINISH => super::window::redraw_dialog_area(big.0, big.1, big.2, big.3),
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
    // App-provided tree: read `ob_state`/`ob_flags` at the caller's ABI width.
    let t = ObjTree::new(tree);
    (t.state(obj), t.flags(obj))
}

/// Deselect the currently-selected radio sibling of `obj` and select `obj`
/// (EmuTOS `fm_button` RBUTTON path).
fn select_radio(tree: *mut Object, obj: i16) {
    // App-provided tree: walk the sibling chain (`ob_head`/`ob_next`) and read
    // state/flags at the caller's ABI width. `object::parent`/`object::change`
    // are already ABI-aware, so pass them the raw pointer.
    // SAFETY: valid tree (GEM contract).
    let parent = unsafe { object::parent(tree as *const Object, obj) };
    if parent < 0 {
        return;
    }
    let t = ObjTree::new(tree as *const Object);
    let mut cur = t.head(parent);
    while cur != NIL && cur != parent {
        let tflags = t.flags(cur);
        let tstate = t.state(cur);
        if tflags & RBUTTON != 0 && (tstate & SELECTED != 0 || cur == obj) {
            let ns = if cur == obj { tstate | SELECTED } else { tstate & !SELECTED };
            object::change(tree, cur, ns, true);
        }
        cur = t.next(cur);
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
    let obj = pb.intin(0); // FM_OBJ
    let clks = pb.intin(1); // FM_CLKS
    if tree.is_null() || obj < 0 {
        pb.set_intout(1, obj.max(0));
        pb.set_ret(1);
        return;
    }
    let (cont, newobj) = fm_button(tree, obj, clks);
    pb.set_intout(1, newobj); // FM_ONXTOB
    pb.set_ret(if cont { 1 } else { 0 });
}

/// `form_keybd` (55) — route the key `int_in[1]` (FM_ICHAR) in the dialog at
/// `addr_in[0]` (current object `int_in[0]` = FM_OBJ): RETURN → the DEFAULT
/// button (exit), TAB/down → next field, up → previous field. `int_out[1]`
/// (FM_ONXTOB) = the next object, `int_out[2]` (FM_OCHAR) = the (possibly
/// cleared) char; `int_out[0]` = continue (1) / exit (0). EmuTOS `fm_keybd`.
///
/// `FM_ONXTOB` is preinitialised from `FM_INXTOB` (int_in[2]) before the call
/// (EmuTOS gemsuper.c:256-258), so a non-navigation key returns int_in[2], not
/// int_in[0].
pub fn form_keybd(pb: &Aespb) {
    let tree = pb.addrin(0) as *mut Object;
    let obj = pb.intin(0); // FM_OBJ
    let thechar = pb.intin(1); // FM_ICHAR
    let inxtob = pb.intin(2); // FM_INXTOB (preinit of FM_ONXTOB)
    if tree.is_null() {
        pb.set_intout(1, inxtob);
        pb.set_intout(2, thechar);
        pb.set_ret(1);
        return;
    }
    let (cont, newchar, newobj) = fm_keybd(tree, obj, thechar, inxtob);
    pb.set_intout(1, newobj);
    pb.set_intout(2, newchar);
    pb.set_ret(if cont { 1 } else { 0 });
}
