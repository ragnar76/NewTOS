//! `desk::insapp` — the "Install Application" dialog (EmuTOS `deskins.c`
//! `ins_app` / `ins_app_dialog`, `docs/api/desktop/deskins.md` §`ins_app`).
//!
//! Associates the currently-selected program with a document type so that
//! double-clicking a matching document later launches the program with the
//! document as its command tail. This module is the **dialog** half: it builds
//! the `OBJECT`/`TEDINFO` tree on the stack and runs it through the EmuTOS
//! dialog chain (`start_dialog` → `form_do` → `end_dialog`, `deskins.c:504,
//! 597`), returning the user's choice; the caller
//! ([`super::main::do_install_app`]) resolves the selected program and commits
//! the result into the [`super::inf`] `NEWDESK.INF` association model.
//!
//! ## Faithful subset (documented divergence)
//!
//! EmuTOS `ins_app_dialog` (`deskins.c:455-600`) exposes, besides the document
//! type: a fixed-arguments field, a function-key field, an autorun radio, an
//! application-directory-vs-top-window radio, and the GEM/TOS/GTP/TTP flavour
//! radio. NewTOS implements the fields that change the *launch contract* — the
//! document-type mask, the GEM/TOS/GTP/TTP flavour (which selects the persisted
//! `#G`/`#Y`/`#P`/`#F` tag), and the full-path-vs-file-name tail choice
//! (`AF_ISFULL`). The function-key/autorun/app-dir fields are **not** offered
//! here (function keys and autorun are separate desktop features that are not
//! yet wired; app-dir defaults to "top window", `AF_APPDIR` clear); the model
//! and file format carry them, so they round-trip untouched when present. Text
//! entry needs a real keyboard, so the dialog is driven interactively on
//! hardware; the association model + launch path are covered by a headless
//! selftest (see `super::inf`/`super::main`).

use alloc::string::String;

use tos_api::aes::object::{Object, TedInfo};

use super::inf::AppFlavor;

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

// ob_state bit (`obdefs.h`).
const OB_SELECTED: u16 = 0x0001;

/// Opaque dialog box colour word (black border + solid-white interior), as used
/// by the other desktop dialogs and the menu drop-downs.
const BOX_SPEC: u64 = (1 << 16) | 0x1070;

/// The maximum edited extension length (a TOS 8.3 extension is 3 chars; a few
/// slack bytes allow a `*` all-files entry too).
const EXT_MAX: usize = 4;

// Object indices in the dialog tree.
const O_GEM: i16 = 7;
const O_TOS: i16 = 8;
const O_GTP: i16 = 9;
const O_TTP: i16 = 10;
const O_FULL: i16 = 12;
const O_FILE: i16 = 13;
const O_INSTALL: i16 = 14;
const O_REMOVE: i16 = 15;
const O_LAST: usize = 16;
/// The editable document-type field — `form_do`'s start object, as EmuTOS
/// passes `APARGS` (`deskins.c:507`).
const O_DOCTYPE: i16 = 5;

/// What the user chose in the Install-Application dialog.
pub(super) enum Outcome {
    /// Install/re-install this application for a document type.
    Install {
        /// The document-type extension the user entered (e.g. `TXT`).
        doc_ext: String,
        /// The GEM/TOS/GTP/TTP flavour (selects the persisted `#`-line tag).
        flavor: AppFlavor,
        /// `AF_ISFULL`: pass the document's full path in the tail (else name).
        full_path: bool,
    },
    /// Remove an existing association for the selected application.
    Remove,
    /// Cancel — leave the model unchanged.
    Cancel,
}

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

/// Set the four flavour radio buttons so only `flavor`'s is `SELECTED`.
fn set_flavor_radio(tree: &mut [Object], flavor: AppFlavor) {
    let sel = match flavor {
        AppFlavor::Gem => O_GEM,
        AppFlavor::Tos => O_TOS,
        AppFlavor::Gtp => O_GTP,
        AppFlavor::Ttp => O_TTP,
    };
    for obj in [O_GEM, O_TOS, O_GTP, O_TTP] {
        if obj == sel {
            tree[obj as usize].ob_state |= OB_SELECTED;
        } else {
            tree[obj as usize].ob_state &= !OB_SELECTED;
        }
    }
}

/// Set the two parameter radio buttons so only the chosen one is `SELECTED`.
fn set_param_radio(tree: &mut [Object], full_path: bool) {
    let (on, off) = if full_path { (O_FULL, O_FILE) } else { (O_FILE, O_FULL) };
    tree[on as usize].ob_state |= OB_SELECTED;
    tree[off as usize].ob_state &= !OB_SELECTED;
}

/// Run the Install-Application dialog for the program `app_name` (its bare file
/// name, shown for reference). `initial_*` pre-fill the fields (for editing an
/// already-installed application); `installed` enables the Remove button.
/// EmuTOS `ins_app_dialog` (`deskins.c:455-600`). Returns the user's [`Outcome`].
/// The screen underneath is restored by `end_dialog`'s `form_dial(FMD_FINISH)`.
pub(super) fn ins_app_dialog(
    app_name: &str,
    initial_ext: &str,
    initial_flavor: AppFlavor,
    initial_full: bool,
    installed: bool,
) -> Outcome {
    // Editable extension buffer — `form_do` edits this in place via the TEDINFO.
    let mut extbuf = [0u8; EXT_MAX + 2];
    let n = initial_ext.len().min(EXT_MAX);
    extbuf[..n].copy_from_slice(&initial_ext.as_bytes()[..n]);

    // The application-name value string (shown, not editable). Copied into an
    // owned NUL-terminated buffer so it outlives the borrow.
    let mut namebuf = [0u8; 16];
    let an = app_name.len().min(namebuf.len() - 1);
    namebuf[..an].copy_from_slice(&app_name.as_bytes()[..an]);

    static EMPTY: [u8; 1] = [0];
    static TITLE: &[u8] = b" Install Application \0";
    static APP_LBL: &[u8] = b"Application:\0";
    static DOC_LBL: &[u8] = b"Document type:   *.\0";
    static TYPE_LBL: &[u8] = b"Program type:\0";
    static GEM: &[u8] = b" GEM \0";
    static TOS: &[u8] = b" TOS \0";
    static GTP: &[u8] = b" GTP \0";
    static TTP: &[u8] = b" TTP \0";
    static PARM_LBL: &[u8] = b"Argument:\0";
    static FULL: &[u8] = b" Full path \0";
    static FILE: &[u8] = b" File name \0";
    static INSTALL: &[u8] = b" Install \0";
    static REMOVE: &[u8] = b" Remove \0";
    static CANCEL: &[u8] = b" Cancel \0";

    let ted = TedInfo {
        te_ptext: extbuf.as_mut_ptr() as u64,
        te_ptmplt: EMPTY.as_ptr() as u64,
        te_pvalid: EMPTY.as_ptr() as u64,
        te_font: 3,
        te_junk1: 0,
        te_just: 0,
        te_color: 1 << 8,
        te_junk2: 0,
        te_thickness: 1,
        te_txtlen: EXT_MAX as i16,
        te_tmplen: 0,
    };

    // Built at the origin; `start_dialog`'s `form_center` writes the centred
    // `ob_x`/`ob_y` into the ROOT.
    let (dw, dh) = (360i16, 240i16);

    let mut tree = [
        // 0: root box.
        mk(G_BOX, 0, BOX_SPEC, 0, 0, dw, dh, -1, 1, O_LAST as i16),
        // 1: title.
        mk(G_STRING, 0, TITLE.as_ptr() as u64, 24, 14, dw - 48, 16, 2, -1, -1),
        // 2: application label, 3: application name value.
        mk(G_STRING, 0, APP_LBL.as_ptr() as u64, 24, 44, 104, 16, 3, -1, -1),
        mk(G_STRING, 0, namebuf.as_ptr() as u64, 144, 44, 180, 16, 4, -1, -1),
        // 4: document-type label ("Document type:  *."), 5: editable extension.
        mk(G_STRING, 0, DOC_LBL.as_ptr() as u64, 24, 72, 160, 16, 5, -1, -1),
        mk(G_BOXTEXT, EDITABLE, &ted as *const _ as u64, 200, 68, 56, 22, 6, -1, -1),
        // 6: program-type label, 7-10: flavour radio buttons.
        mk(G_STRING, 0, TYPE_LBL.as_ptr() as u64, 24, 104, 104, 16, 7, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT, GEM.as_ptr() as u64, 144, 100, 48, 22, 8, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT, TOS.as_ptr() as u64, 196, 100, 48, 22, 9, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT, GTP.as_ptr() as u64, 248, 100, 48, 22, 10, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT, TTP.as_ptr() as u64, 300, 100, 48, 22, 11, -1, -1),
        // 11: argument label, 12-13: full-path/file-name radio.
        mk(G_STRING, 0, PARM_LBL.as_ptr() as u64, 24, 136, 104, 16, 12, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT, FULL.as_ptr() as u64, 144, 132, 88, 22, 13, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT, FILE.as_ptr() as u64, 240, 132, 88, 22, 14, -1, -1),
        // 14: Install (default), 15: Remove, 16: Cancel (last).
        mk(G_BUTTON, SELECTABLE | EXIT | DEFAULT, INSTALL.as_ptr() as u64, 24, 190, 90, 30, 15, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT, REMOVE.as_ptr() as u64, 136, 190, 90, 30, 16, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | LASTOB, CANCEL.as_ptr() as u64, 248, 190, 90, 30, 0, -1, -1),
    ];

    // A not-installed application cannot be removed (EmuTOS `deskins.c:585-586`).
    if !installed {
        tree[O_REMOVE as usize].ob_state |= super::global::DISABLED;
    }

    // Pre-select the flavour + parameter radios.
    let mut flavor = initial_flavor;
    let mut full_path = initial_full;
    set_flavor_radio(&mut tree, flavor);
    set_param_radio(&mut tree, full_path);

    // `start_dialog(tree)` … `do { form_do(tree, APARGS) } while(retry)` …
    // `end_dialog(tree)` — EmuTOS `ins_app_dialog` (`deskins.c:504-597`), with
    // `draw_dial(tree)` between turns exactly as its retry path does (`:527`).
    let tree_ptr = tree.as_ptr() as u64;
    super::inf::start_dialog(tree_ptr);
    let outcome = loop {
        let clicked = super::form_do(tree_ptr, O_DOCTYPE);
        match clicked {
            O_GEM => flavor = AppFlavor::Gem,
            O_TOS => flavor = AppFlavor::Tos,
            O_GTP => flavor = AppFlavor::Gtp,
            O_TTP => flavor = AppFlavor::Ttp,
            O_FULL => full_path = true,
            O_FILE => full_path = false,
            O_INSTALL => {
                // Read the document-type field and finish (the dialog is torn
                // down and the screen repainted, so no need to clear SELECTED).
                let m = extbuf.iter().position(|&b| b == 0).unwrap_or(extbuf.len());
                let ext = core::str::from_utf8(&extbuf[..m]).unwrap_or("").trim();
                break Outcome::Install {
                    doc_ext: String::from(ext),
                    flavor,
                    full_path,
                };
            }
            O_REMOVE if installed => break Outcome::Remove,
            _ => break Outcome::Cancel, // Cancel / default / disabled Remove
        }
        // A radio button was clicked: re-assert the group selection and redraw.
        set_flavor_radio(&mut tree, flavor);
        set_param_radio(&mut tree, full_path);
        super::dir::draw_dial(tree_ptr);
    };
    super::inf::end_dialog(tree_ptr);

    outcome
}
