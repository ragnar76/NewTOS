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
//! `#G`/`#Y`/`#P`/`#F` tag), the full-path-vs-file-name tail choice
//! (`AF_ISFULL`) — and, since DESK20, the **function key** (`APFUNKEY`,
//! `desk_rsc.h:119`), with `get_funkey`'s validation and its two alerts.
//! The autorun/app-dir/arguments fields are **not** offered here (autorun is a
//! separate desktop feature that is not yet wired; app-dir defaults to "top
//! window", `AF_APPDIR` clear); the model and file format carry them, so they
//! round-trip untouched when present. Text entry needs a real keyboard, so the
//! dialog is driven interactively on hardware; the association model + launch
//! path are covered by a headless selftest (see `super::inf`/`super::main`).

use alloc::string::String;

use tos_types::aes::{
    Object, TedInfo, DEFAULT, DISABLED, EDITABLE, EXIT, FL3DACT, FL3DBAK, FL3DIND, G_BOXTEXT,
    G_BUTTON, G_STRING, LASTOB, SELECTABLE, SELECTED,
};

// `AppFlavor` plus the dialog-box spec, the TEDINFO colour word and the 3D
// roles, shared with the other desktop dialogs.
use super::inf::{mk_root, AppFlavor, BOX_SPEC, TED_COLOR};

// OBJECT graphic types (`obdefs.h`).

// ob_flags bits (`obdefs.h`).

// ob_state bit (`obdefs.h`).

/// The maximum edited extension length (a TOS 8.3 extension is 3 chars; a few
/// slack bytes allow a `*` all-files entry too).
const EXT_MAX: usize = 4;

/// The number of digits the function-key field takes. `desk_rs_tedinfo[19]`'s
/// `te_txtlen` is 3 (`desk_rsc.c:130-133`) and a `te_txtlen` counts the NUL,
/// so the field holds **two** digits — which is what F1…F20 needs. The
/// `te_txtlen` below is therefore `FKEY_MAX + 1`, exactly as the neighbouring
/// document-type field's `EXT_MAX` (3 characters) is given as 4.
const FKEY_MAX: usize = 2;

/// `FIRST_FUNKEY` / `LAST_FUNKEY` (`deskins.h:16-17`) — the range `get_funkey`
/// accepts; anything else raises [`STINVKEY`].
const FIRST_FUNKEY: u8 = 1;
const LAST_FUNKEY: u8 = 20;

// Object indices in the dialog tree.
const O_GEM: i16 = 9;
const O_TOS: i16 = 10;
const O_GTP: i16 = 11;
const O_TTP: i16 = 12;
const O_FULL: i16 = 14;
const O_FILE: i16 = 15;
const O_INSTALL: i16 = 16;
const O_REMOVE: i16 = 17;
const O_LAST: usize = 18;
/// The editable document-type field — `form_do`'s start object, as EmuTOS
/// passes `APARGS` (`deskins.c:507`).
const O_DOCTYPE: i16 = 5;
/// The editable function-key field (`APFUNKEY`, `desk_rsc.h:119`).
const O_FUNKEY: i16 = 7;

/// `STINVKEY` (`desk_rsc.c:2525`) — verbatim, buttons included, because
/// `fun_alert` hands the whole resource string to `form_alert`.
static STINVKEY: &[u8] = b"[1][Invalid function key value.][  OK  ]\0";
/// `STDUPKEY` (`desk_rsc.c:2526-2527`) — two buttons, default 1 (`OK`).
static STDUPKEY: &[u8] =
    b"[2][Function key already assigned.|Do you want to reassign it?][  OK  |Cancel]\0";

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
        /// The validated function key (`pa->a_funkey`, `deskins.c:531`);
        /// `0` = none.
        funkey: u8,
    },
    /// Remove an existing association for the selected application.
    Remove,
    /// Cancel — leave the model unchanged.
    Cancel,
}

/// `atow` (`deskins.c:380-393`) — leading decimal digits of `s` as a number;
/// stops at the first non-digit, so a leading blank or letter yields 0.
fn atow(s: &[u8]) -> u16 {
    let mut n: u16 = 0;
    for &c in s {
        if c.is_ascii_digit() {
            n = n.saturating_mul(10).saturating_add((c - b'0') as u16);
        } else {
            break;
        }
    }
    n
}

/// `get_funkey(tree, pa, installed)` (`deskins.c:404-444`) — "checks for
/// invalid or duplicate function key and issues appropriate alert".
///
/// `fkey` is the raw text of `APFUNKEY`, `app_path` the application being
/// installed (EmuTOS's `pa`, which its walk skips by pointer identity) and
/// `current` its stored `a_funkey` — only meaningful when `installed`.
/// Returns the key to store, or [`None`] for EmuTOS's `-1` (the caller then
/// clears the field and asks the user to try again).
fn get_funkey(fkey: &[u8], app_path: &str, current: u8, installed: bool) -> Option<u8> {
    // `if (fkey[0] == '\0') return 0;` — an empty field means "no key".
    if fkey.first().copied().unwrap_or(0) == 0 {
        return Some(0);
    }

    let funkey = atow(fkey);
    // `if (installed && (funkey == pa->a_funkey)) return funkey;` — unchanged,
    // so it is not re-validated. That is deliberate in EmuTOS: a key a
    // hand-written `.INF` put outside the range survives an edit of the other
    // fields.
    if installed && funkey == current as u16 {
        return Some(current);
    }

    // `if ((funkey < FIRST_FUNKEY) || (funkey > LAST_FUNKEY)) { … STINVKEY … }`
    if funkey < FIRST_FUNKEY as u16 || funkey > LAST_FUNKEY as u16 {
        super::form_alert(1, STINVKEY.as_ptr());
        return None;
    }
    let funkey = funkey as u8;

    // The walk over the other `ANODE`s (`:430-441`), split around the question
    // so the session lock is not held across the trap (see
    // [`super::inf::funkey_owner`]).
    if super::inf::funkey_owner(funkey, app_path) {
        if super::form_alert(1, STDUPKEY.as_ptr()) != 1 {
            return None;
        }
        super::inf::release_funkey(funkey, app_path);
    }

    Some(funkey)
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
            tree[obj as usize].ob_state |= SELECTED;
        } else {
            tree[obj as usize].ob_state &= !SELECTED;
        }
    }
}

/// Set the two parameter radio buttons so only the chosen one is `SELECTED`.
fn set_param_radio(tree: &mut [Object], full_path: bool) {
    let (on, off) = if full_path { (O_FULL, O_FILE) } else { (O_FILE, O_FULL) };
    tree[on as usize].ob_state |= SELECTED;
    tree[off as usize].ob_state &= !SELECTED;
}

/// Run the Install-Application dialog for the program `app_name` (its bare file
/// name, shown for reference). `initial_*` pre-fill the fields (for editing an
/// already-installed application); `installed` enables the Remove button.
/// EmuTOS `ins_app_dialog` (`deskins.c:455-600`). Returns the user's [`Outcome`].
/// The screen underneath is restored by `end_dialog`'s `form_dial(FMD_FINISH)`.
pub(super) fn ins_app_dialog(
    app_name: &str,
    app_path: &str,
    initial_ext: &str,
    initial_flavor: AppFlavor,
    initial_full: bool,
    initial_funkey: u8,
    installed: bool,
) -> Outcome {
    // Editable extension buffer — `form_do` edits this in place via the TEDINFO.
    let mut extbuf = [0u8; EXT_MAX + 2];
    let n = initial_ext.len().min(EXT_MAX);
    extbuf[..n].copy_from_slice(&initial_ext.as_bytes()[..n]);

    // Editable function-key buffer, pre-filled exactly as `ins_app_dialog`
    // does (`deskins.c:474-477`): `sprintf(name, "%u ", pa->a_funkey)` when the
    // node has a key, empty otherwise, and empty for an application that is not
    // installed at all. `inf_sset` truncates to the field width, which for 20
    // drops the trailing blank.
    let mut fkeybuf = [0u8; FKEY_MAX + 2];
    if installed && initial_funkey != 0 {
        let mut tmp = [0u8; 4];
        let mut k = 0usize;
        if initial_funkey >= 10 {
            tmp[k] = b'0' + initial_funkey / 10;
            k += 1;
        }
        tmp[k] = b'0' + initial_funkey % 10;
        k += 1;
        tmp[k] = b' ';
        k += 1;
        let m = k.min(FKEY_MAX);
        fkeybuf[..m].copy_from_slice(&tmp[..m]);
    }

    // The application-name value string (shown, not editable). Copied into an
    // owned NUL-terminated buffer so it outlives the borrow.
    let mut namebuf = [0u8; 16];
    let an = app_name.len().min(namebuf.len() - 1);
    namebuf[..an].copy_from_slice(&app_name.as_bytes()[..an]);

    static EMPTY: [u8; 1] = [0];
    static TITLE: &[u8] = b" Install Application \0";
    static APP_LBL: &[u8] = b"Application:\0";
    static DOC_LBL: &[u8] = b"Document type:   *.\0";
    /// `desk_rs_tedinfo[19]`'s template without its two `_` placeholders —
    /// the `G_BOXTEXT` field below is the placeholder (`desk_rsc.c:131`).
    static FKEY_LBL: &[u8] = b"Install as:       F\0";
    /// `te_pvalid` of `APFUNKEY` — digits only (`desk_rsc.c:132`).
    static V_DIGIT: &[u8] = b"9\0";
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
        te_color: TED_COLOR,
        te_junk2: 0,
        te_thickness: 1,
        te_txtlen: EXT_MAX as i16,
        te_tmplen: 0,
    };
    let ted_fkey = TedInfo {
        te_ptext: fkeybuf.as_mut_ptr() as u64,
        te_ptmplt: EMPTY.as_ptr() as u64,
        te_pvalid: V_DIGIT.as_ptr() as u64,
        te_font: 3,
        te_junk1: 0,
        te_just: 0,
        te_color: TED_COLOR,
        te_junk2: 0,
        te_thickness: 1,
        te_txtlen: (FKEY_MAX + 1) as i16,
        te_tmplen: 0,
    };

    // Built at the origin; `start_dialog`'s `form_center` writes the centred
    // `ob_x`/`ob_y` into the ROOT.
    let (dw, dh) = (360i16, 272i16);

    // The 3D roles are `ADINSAPP`'s, object by object (`desk_rsc.c:1027-1195`):
    // `FL3DBAK` + `OUTLINED` ROOT; `APDOCTYP` is an `EDITABLE | FL3DBAK` field;
    // the flavour group (`APTOS`/`APTTP`/`APGEM`/`APGTP`) and the parameter
    // group (`APPMFULL`/`APPMFILE`) are `FL3DIND` indicators;
    // `APINSTAL`/`APREMOVE`/`APCANCEL` are `FL3DBAK | FL3DIND` = `FL3DACT`.
    // Two mapped-by-role notes. (1) `ADINSAPP`'s groups are true `RBUTTON`s
    // under a `G_IBOX`, while these six carry `EXIT` and the group state is
    // re-asserted in the loop below (`set_flavor_radio`/`set_param_radio`).
    // `FL3DIND` is still the resource's answer for that combination, not an
    // analogy: `ADBKGND`'s `BGDESK`/`BGWIN` are
    // `SELECTABLE | EXIT | RBUTTON | FL3DIND` (`desk_rsc.c:2254,2260`).
    // (2) `APNAME` is a `FL3DBAK` `G_FBOXTEXT` where NewTOS shows the
    // application name as a plain `G_STRING`, which has no interior for a 3D
    // background to fill.
    //
    // The flavour row is spaced 54 px for a 44 px button (was 52 for 48): an
    // `FL3DIND`/`FL3DACT` object is grown by `ADJ3DSTD` (2 px per side) and its
    // `th = -2` border is laid outside *that* (`aes/gemoblib.c:504`), so an
    // exit button claims 4 px more per side than its rectangle. `ADINSAPP`
    // leaves the same room: 40 px buttons on a 48 px pitch (`desk_rsc.c:1098-
    // 1124`), an 8 px gap. `APPMFILE` moves 240 → 244 for the same reason.
    let mut tree = [
        // 0: root box.
        mk_root(BOX_SPEC, dw, dh, O_LAST as i16),
        // 1: title.
        mk(G_STRING, 0, TITLE.as_ptr() as u64, 24, 14, dw - 48, 16, 2, -1, -1),
        // 2: application label, 3: application name value.
        mk(G_STRING, 0, APP_LBL.as_ptr() as u64, 24, 44, 104, 16, 3, -1, -1),
        mk(G_STRING, 0, namebuf.as_ptr() as u64, 144, 44, 180, 16, 4, -1, -1),
        // 4: document-type label ("Document type:  *."), 5: editable extension.
        mk(G_STRING, 0, DOC_LBL.as_ptr() as u64, 24, 72, 160, 16, 5, -1, -1),
        mk(G_BOXTEXT, EDITABLE | FL3DBAK, &ted as *const _ as u64, 200, 68, 56, 22, 6, -1, -1),
        // 6: function-key label ("Install as:       F"), 7: editable number
        // (`APFUNKEY`, `desk_rsc.c:1059-1063` — an `EDITABLE | FL3DBAK`
        // field, one row under `APDOCTYP`, as it is there).
        mk(G_STRING, 0, FKEY_LBL.as_ptr() as u64, 24, 104, 160, 16, O_FUNKEY, -1, -1),
        mk(G_BOXTEXT, EDITABLE | FL3DBAK, &ted_fkey as *const _ as u64, 200, 100, 40, 22, 8, -1, -1),
        // 8: program-type label, 9-12: flavour radio buttons.
        mk(G_STRING, 0, TYPE_LBL.as_ptr() as u64, 24, 136, 104, 16, 9, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DIND, GEM.as_ptr() as u64, 144, 132, 44, 22, 10, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DIND, TOS.as_ptr() as u64, 198, 132, 44, 22, 11, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DIND, GTP.as_ptr() as u64, 252, 132, 44, 22, 12, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DIND, TTP.as_ptr() as u64, 306, 132, 44, 22, 13, -1, -1),
        // 13: argument label, 14-15: full-path/file-name radio.
        mk(G_STRING, 0, PARM_LBL.as_ptr() as u64, 24, 168, 104, 16, 14, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DIND, FULL.as_ptr() as u64, 144, 164, 88, 22, 15, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DIND, FILE.as_ptr() as u64, 244, 164, 88, 22, 16, -1, -1),
        // 16: Install (default), 17: Remove, 18: Cancel (last).
        mk(G_BUTTON, SELECTABLE | EXIT | DEFAULT | FL3DACT, INSTALL.as_ptr() as u64, 24, 222, 90, 30, 17, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DACT, REMOVE.as_ptr() as u64, 136, 222, 90, 30, 18, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | LASTOB | FL3DACT, CANCEL.as_ptr() as u64, 248, 222, 90, 30, 0, -1, -1),
    ];

    // A not-installed application cannot be removed (EmuTOS `deskins.c:585-586`).
    if !installed {
        tree[O_REMOVE as usize].ob_state |= DISABLED;
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
                // `funkey = get_funkey(tree,pa,installed); if (funkey < 0) {
                //  inf_sset(tree, APFUNKEY, ""); tree[APINSTAL].ob_state &=
                //  ~SELECTED; … draw_dial(tree); exitobj = -1; break; }`
                // (`deskins.c:520-530`) — the *only* retry the dialog has.
                let Some(key) = get_funkey(&fkeybuf, app_path, initial_funkey, installed) else {
                    fkeybuf.fill(0);
                    tree[O_INSTALL as usize].ob_state &= !SELECTED;
                    super::dir::draw_dial(tree_ptr);
                    continue;
                };
                // Read the document-type field and finish (the dialog is torn
                // down and the screen repainted, so no need to clear SELECTED).
                let m = extbuf.iter().position(|&b| b == 0).unwrap_or(extbuf.len());
                let ext = core::str::from_utf8(&extbuf[..m]).unwrap_or("").trim();
                break Outcome::Install {
                    doc_ext: String::from(ext),
                    flavor,
                    full_path,
                    funkey: key,
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
