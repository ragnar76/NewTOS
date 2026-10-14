//! `desktop::dlg` — a minimal editable name-input dialog for the desktop's own
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
//! arena) and hands it to [`super::inf::inf_show`], i.e. the same
//! `form_center` → `form_dial(FMD_START)` → `objc_draw` → `form_do` →
//! `form_dial(FMD_FINISH)` chain EmuTOS runs for these boxes, minus the
//! resource file. It is interactive-only (needs a live keyboard), so it is
//! exercised on real hardware (see
//! `docs/api/desktop/_hw-test-checklist.md`).

use alloc::string::String;

use tos_types::aes::{
    Object, TedInfo, DEFAULT, EDITABLE, EXIT, FL3DACT, FL3DBAK, G_BOXTEXT, G_BUTTON, G_STRING,
    LASTOB, ROOT, SELECTABLE,
};

// The dialog-box spec, the TEDINFO colour word and the 3D roles, shared with
// the other desktop dialogs (see `super::inf`).
use super::inf::{mk_root, BOX_SPEC, TED_COLOR};

// OBJECT graphic types (`obdefs.h`).

// ob_flags bits (`obdefs.h`).

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
/// or `None` on Cancel / no change.
///
/// The screen underneath is restored by the `form_dial(FMD_FINISH)` inside
/// [`super::inf::inf_show`]; the caller does **not** repaint the desktop.
pub fn edit_name(prompt: &[u8], initial: &str) -> Option<String> {
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
        te_color: TED_COLOR,
        te_junk2: 0,
        te_thickness: 1,
        te_txtlen: NAME_MAX as i16,
        te_tmplen: 0,
    };

    // A 290x150 dialog built at the origin: `form_center` (inside `inf_show`)
    // writes the centred `ob_x`/`ob_y` into the ROOT, exactly as EmuTOS builds
    // its dialogs from a resource file whose ROOT carries no screen position.
    let (dw, dh) = (290i16, 150i16);

    // The 3D roles are `ADMKDBOX`'s and `ADSEARCH`'s, object by object — the two
    // `desk_rsc.c` trees this dialog stands in for (`desk_rsc.c:1282-1346`):
    // `FL3DBAK` + `OUTLINED` ROOT over the `0x1100` colour word, a plain
    // `G_STRING` title, an `EDITABLE | FL3DBAK` name field (`MKNAME`/`SFNAME`),
    // and `FL3DBAK | FL3DIND` = `FL3DACT` on `MKOK`/`MKCNCL`.
    let tree = [
        mk_root(BOX_SPEC, dw, dh, 4),
        mk(G_STRING, 0, prompt.as_ptr() as u64, 24, 18, dw - 48, 16, 2, -1, -1),
        mk(G_BOXTEXT, EDITABLE | FL3DBAK, &ted as *const _ as u64, 24, 46, 200, 24, 3, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | DEFAULT | FL3DACT, OK.as_ptr() as u64, 28, 104, 90, 28, 4, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | LASTOB | FL3DACT, CANCEL.as_ptr() as u64, 172, 104, 90, 28, 0, -1, -1),
    ];

    // `inf_show(tree, ROOT)` (`deskinf.c:396-402`): start_dialog → form_do →
    // end_dialog, i.e. the whole centre/reserve/draw/run/release/drain chain.
    let clicked = super::inf::inf_show(tree.as_ptr() as u64, ROOT);

    if clicked != OB_OK {
        return None;
    }
    let m = textbuf.iter().position(|&b| b == 0).unwrap_or(textbuf.len());
    if m == 0 {
        return None; // empty name rejected
    }
    core::str::from_utf8(&textbuf[..m]).ok().map(String::from)
}
