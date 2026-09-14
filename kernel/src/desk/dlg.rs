//! `desk::dlg` — a minimal editable name-input dialog for the desktop's own
//! New-folder and Rename actions (EmuTOS `deskfun.c:fun_mkdir` /
//! `deskinf.c:inf_file_folder`'s rename field).
//!
//! The full Show-Info panel (size/date/attributes) and the preferences/config
//! editors live in `deskinf.c` and are **Task #14**. This module provides only
//! the single reusable primitive both `fun_mkdir` and the rename action need: a
//! centred `[label][editable field][ OK ][ Cancel ]` dialog run through
//! `form_do`, returning the typed string (or `None` on Cancel).
//!
//! It builds its own `OBJECT`/`TEDINFO` tree on the stack (not the desktop
//! arena), paints it with `objc_draw`, runs `form_do`, and reads the result
//! back — the same pattern EmuTOS uses for these boxes, minus the resource
//! file. It is interactive-only (needs a live keyboard), so it is exercised on
//! real hardware (see `docs/api/desktop/_hw-test-checklist.md`).

use alloc::string::String;

use tos_api::aes::object::{Object, TedInfo};

use super::global::Grect;

// OBJECT graphic types (`obdefs.h`).
const G_BOX: u16 = 20;
const G_BOXTEXT: u16 = 22;
const G_BUTTON: u16 = 26;
const G_STRING: u16 = 28;

// ob_flags bits (`obdefs.h`).
const SELECTABLE: u16 = 0x0001;
const DEFAULT: u16 = 0x0002;
const EXIT: u16 = 0x0004;
const EDITABLE: u16 = 0x0008;
const LASTOB: u16 = 0x0020;

/// Object index of the OK button (its `form_do` return value).
const OB_OK: i16 = 3;

/// The maximum edited name length (an 8.3 name is 12 chars; one slack byte).
const NAME_MAX: usize = 13;

/// Build one dialog `OBJECT`.
#[allow(clippy::too_many_arguments)]
fn mk(ty: u16, fl: u16, spec: u64, x: i16, y: i16, w: i16, h: i16, next: i16, head: i16, tail: i16) -> Object {
    Object {
        ob_next: next,
        ob_head: head,
        ob_tail: tail,
        ob_type: ty,
        ob_flags: fl,
        ob_state: 0,
        ob_spec: spec,
        ob_x: x,
        ob_y: y,
        ob_width: w,
        ob_height: h,
    }
}

/// Show a centred name-input dialog titled `prompt` (a NUL-terminated string),
/// pre-filled with `initial`. Returns the entered name on OK (empty rejected),
/// or `None` on Cancel / no change. The caller repaints the screen underneath
/// afterwards (the dialog painted over it).
pub fn edit_name(prompt: &[u8], initial: &str) -> Option<String> {
    let (sw, sh) = screen_dims();

    // The editable buffer — `form_do` edits this in place through the TEDINFO.
    let mut textbuf = [0u8; NAME_MAX + 2];
    let n = initial.len().min(NAME_MAX);
    textbuf[..n].copy_from_slice(&initial.as_bytes()[..n]);

    static EMPTY: [u8; 1] = [0];
    static OK: &[u8] = b" OK \0";
    static CANCEL: &[u8] = b" Cancel \0";

    let ted = TedInfo {
        te_ptext: textbuf.as_mut_ptr() as u64,
        te_ptmplt: EMPTY.as_ptr() as u64,
        te_pvalid: EMPTY.as_ptr() as u64,
        te_font: 3,
        te_junk1: 0,
        te_just: 0,
        te_color: 1 << 8,
        te_junk2: 0,
        te_thickness: 1,
        te_txtlen: NAME_MAX as i16,
        te_tmplen: 0,
    };

    // Centre a 290x150 dialog.
    let (dw, dh) = (290i16, 150i16);
    let bx = ((sw - dw) / 2).max(0);
    let by = ((sh - dh) / 2).max(0);

    // Colour word `(1<<16)|0x1070` = black border + solid-white interior, so the
    // dialog is opaque over the desktop (matches the menu-drop-down box).
    let box_spec: u64 = (1 << 16) | 0x1070;

    let tree = [
        mk(G_BOX, 0, box_spec, bx, by, dw, dh, -1, 1, 4),
        mk(G_STRING, 0, prompt.as_ptr() as u64, 24, 18, dw - 48, 16, 2, -1, -1),
        mk(G_BOXTEXT, EDITABLE, &ted as *const _ as u64, 24, 46, 200, 24, 3, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | DEFAULT, OK.as_ptr() as u64, 28, 104, 90, 28, 4, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | LASTOB, CANCEL.as_ptr() as u64, 172, 104, 90, 28, 0, -1, -1),
    ];

    let clip = Grect { x: 0, y: 0, w: sw, h: sh };
    super::objc_draw_dialog(tree.as_ptr() as u64, clip);
    let clicked = super::form_do(tree.as_ptr() as u64);

    if clicked != OB_OK {
        return None;
    }
    let m = textbuf.iter().position(|&b| b == 0).unwrap_or(textbuf.len());
    if m == 0 {
        return None; // empty name rejected
    }
    core::str::from_utf8(&textbuf[..m]).ok().map(String::from)
}

/// The current screen dimensions.
fn screen_dims() -> (i16, i16) {
    let r = super::screen_rect();
    (r.w, r.h)
}
