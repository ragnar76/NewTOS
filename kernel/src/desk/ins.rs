//! `desk::ins` — EmuTOS `deskins.c`: *Install devices*, *Install icon* and
//! *Remove desktop icon*, i.e. the three Options-menu entries that edit the
//! desktop's own icon list `G.g_ahead` ([`Anode`]).
//!
//! Reimplements, per [`docs/api/desktop/deskins.md`]:
//!
//! * the grid placement helpers `grid_free` / `ins_posdisk` (`deskins.c:110-221`)
//!   — `snap_icon` itself lives with the other grid metrics in [`super::app`];
//! * `install_drive` / `remove_drive` / `ins_devices` (`:228-333`), which
//!   reconcile the icon list with the live drive map;
//! * `install_desktop_icon` (`:781-978`), the `ADINSDSK` dialog, built here on
//!   the stack and run through the shared `start_dialog`/`form_do`/`end_dialog`
//!   chain in [`super::inf`];
//! * `is_executable` / `set_icon` / `allocate_window_anode` /
//!   `install_window_icon` (`:988-1222`, `CONF_WITH_WINDOW_ICONS`), the
//!   `ADINSWIN` dialog that binds an icon to a **file or folder name**; the
//!   three list primitives live with the `ANODE` store they edit, in
//!   [`super::inf`];
//! * `ins_icon` / `rmv_icon` (`:1234-1335`), the two menu entry points.
//!
//! ## What is **not** here, and why
//!
//! EmuTOS's full build also offers *desktop shortcuts*
//! (`CONF_WITH_DESKTOP_SHORTCUTS`, `ins_shortcut` + the `AT_ISFILE`/`AT_ISFOLD`
//! arms of `install_desktop_icon`): a file or folder icon parked on the desktop
//! surface itself. That needs an `AT_ISFILE`/`AT_ISFOLD` variant of the
//! *desktop* icon list `G.g_ahead` ([`Anode`]'s doc-comment), which NewTOS does
//! not have, plus `fun_win2win`'s drop-on-a-program branch — a separate
//! work item (DESK30), not the missing reader this module used to report.
//! `CONF_WITH_DESKTOP_SHORTCUTS` is 1 in our Falcon030/TOS4 `make 512` profile
//! (`include/config.h:1782-1784`; the `0` at `:325-327` is inside
//! `#ifdef TARGET_192`), so it is a real gap, not a feature that is off in the
//! reference build.

use alloc::string::String;
use alloc::vec::Vec;

use tos_api::aes::object::{IconBlk, Object, TedInfo};
use tos_api::vdi::font::{CELL_H, CELL_W};

use super::dosbind::{dos_gdrv, dos_sdrv};
use super::global::{self, Anode, DROOT, NIL};
use super::icons::{build_iconblk, icon_kind, ig_of, IconKind, BUILTIN_ICONS, IG_HARD};
use super::inf::{
    disk_label, end_dialog, mk_root, start_dialog, BOX_SPEC, FL3DACT, FL3DBAK, FL3DIND, TED_COLOR,
};
use super::model::AType;

// ── OBJECT graphic types / flags (`obdefs.h`), as in the sibling dialogs ────

const G_BOX: u16 = 20;
const G_IBOX: u16 = 25;
const G_BUTTON: u16 = 26;
const G_STRING: u16 = 28;
const G_FBOXTEXT: u16 = 30;
const G_ICON: u16 = 31;

const SELECTABLE: u16 = 0x0001;
const DEFAULT: u16 = 0x0002;
const EXIT: u16 = 0x0004;
const EDITABLE: u16 = 0x0008;
const RBUTTON: u16 = 0x0010;
const LASTOB: u16 = 0x0020;
const TOUCHEXIT: u16 = 0x0040;

const OB_SELECTED: u16 = 0x0001;

/// `BLKDEVNUM` (`bios/blkdev.h`) — the number of GEMDOS drive letters.
const BLKDEVNUM: u32 = 26;

// ── Object indices, the numbers `desk_rsc.h:100-112` gives `ADINSDSK` ───────

/// `ID_ID` — the drive-identifier field (`desk_rsc.h:101`).
const ID_ID: i16 = 2;
/// `ID_LABEL` — the editable label field (`:102`).
const ID_LABEL: i16 = 3;
/// `ID_DRIVE` — first of the three item-type radio buttons (`:103`).
const ID_DRIVE: i16 = 6;
/// `ID_IBOX` — the box holding the icon preview and its two steppers (`:106`);
/// the field `draw_fld` refreshes after a step (`deskins.c:962`).
const ID_IBOX: i16 = 9;
/// `ID_ICON` — the icon preview (`:107`).
const ID_ICON: i16 = 10;
/// `ID_UP` — step to the previous icon (`:108`).
const ID_UP: i16 = 11;
/// `ID_DOWN` — step to the next icon (`:109`).
const ID_DOWN: i16 = 12;
/// `ID_OK` (`:110`); `ID_SKIP` and `ID_CNCL` follow it.
const ID_OK: i16 = 14;
/// `ID_SKIP` (`:111`).
const ID_SKIP: i16 = 15;
/// `ID_CNCL` (`:112`).
const ID_CNCL: i16 = 16;

// ── Object indices, the numbers `desk_rsc.h:225-237` gives `ADINSWIN` ───────

/// `IW_NAME` — the file/folder name field (`desk_rsc.h:226`).
const IW_NAME: i16 = 2;
/// `IW_RBOX` — the `G_IBOX` holding the two item-type radio buttons (`:227`).
const IW_RBOX: i16 = 4;
/// `IW_FOLD` — the *Folder* radio button (`:228`).
const IW_FOLD: i16 = 5;
/// `IW_FILE` — the *File* radio button (`:229`).
const IW_FILE: i16 = 6;
/// `IW_IBOX` — the box holding the icon preview and its steppers (`:230`); the
/// field `draw_fld` refreshes after a step (`deskins.c:1205`).
const IW_IBOX: i16 = 7;
/// `IW_ICON` — the icon preview (`:231`).
const IW_ICON: i16 = 8;
/// `IW_UP` — step to the previous icon (`:232`).
const IW_UP: i16 = 9;
/// `IW_DOWN` — step to the next icon (`:233`).
const IW_DOWN: i16 = 10;
/// `IW_INST` — install / re-install (`:234`).
const IW_INST: i16 = 12;
/// `IW_REMV` — remove (`:235`).
const IW_REMV: i16 = 13;
/// `IW_SKIP` (`:236`).
const IW_SKIP: i16 = 14;
/// `IW_CNCL` (`:237`).
const IW_CNCL: i16 = 15;

/// `LEN_ZFNAME` (`include/sysconf.h:52`) — `NAME.EXT` plus the NUL; the
/// capacity of `install_window_icon`'s name buffer (`deskins.c:1070`).
const LEN_ZFNAME: usize = 13;

/// The editable label's capacity: `MAX_ICONTEXT_WIDTH` characters
/// (`deskapp.c:74`) — the width the icon cell reserves for it.
const LABEL_MAX: usize = 12;

/// `ID_IBOX`'s `ob_spec`: `4352L` = `0x1100` (`desk_rsc.c:977`) — the same
/// colour word as [`BOX_SPEC`] but with the border-thickness word **zero**, so
/// the panel holding the icon preview and its two steppers takes the
/// [`FL3DBAK`] face (hollow + white is the condition, DESK01b) and draws **no**
/// frame of its own. The dialog ROOT is the only box in `ADINSDSK` with a
/// thickness (`69888L` = `(1<<16)|0x1100`, `:926`).
const IBOX_SPEC: u64 = 0x1100;

// ── Grid placement (`deskins.c:110-221`) ───────────────────────────────────

/// `grid_free` (`deskins.c:110-127`) — is desktop grid cell `(x, y)` free of
/// every icon in `G.g_ahead`? Cells are compared in grid units
/// (`pa->a_xspot/G.g_icw`), so two icons that merely overlap in pixels but
/// round to different cells do not collide, exactly as in the source.
fn grid_free(g: &global::DeskState, x: i16, y: i16) -> bool {
    let icw = g.g_icw.max(1);
    let ich = g.g_ich.max(1);
    !g.g_ahead.iter().any(|pa| x == pa.a_xspot / icw && y == pa.a_yspot / ich)
}

/// `ins_posdisk` (`deskins.c:187-221`) — starting at pixel position
/// `(dx, dy)`, scan right and then down for a grid cell no icon occupies and
/// return that cell's pixel position. Falls back to the input position when the
/// whole desktop is full, as in the source.
fn ins_posdisk(g: &global::DeskState, dx: i16, dy: i16) -> (i16, i16) {
    let icw = g.g_icw.max(1);
    let ich = g.g_ich.max(1);
    let xcnt = (g.g_desk.w / icw).max(1);
    let ycnt = (g.g_desk.h / ich).max(1);
    let xin = dx / icw;
    let yin = dy / ich;

    let mut x = xin;
    let mut y = yin;
    while y < ycnt {
        while x < xcnt {
            if grid_free(g, x, y) {
                return (dx + (x - xin) * icw, dy + (y - yin) * ich);
            }
            x += 1;
        }
        x = 0;
        y += 1;
    }
    (dx, dy)
}

// ── Install devices (`deskins.c:228-333`) ──────────────────────────────────

/// `install_drive` (`deskins.c:228-255`) — append an [`Anode`] for drive index
/// `drive` at the first free grid cell. Returns `false` when no icon object can
/// ever be built for it (EmuTOS's `app_alloc` failure, `:236-238`); our list is
/// a heap `Vec`, so the only equivalent limit is the object arena, and that is
/// reported later by `app_blddesk`.
fn install_drive(g: &mut global::DeskState, drive: u32) -> bool {
    // "find first available spot on desktop (before we alloc a new one)".
    let (x, y) = ins_posdisk(g, 0, 0);
    let letter = b'A' + drive as u8;
    // `pa->a_aicon = (drive > 1) ? IG_HARD : IG_FLOPPY;` (`deskins.c:249`).
    let a_aicon =
        if drive > 1 { super::icons::IG_HARD } else { super::icons::IG_FLOPPY };
    let (a_xspot, a_yspot) = g.snap_icon(x, y);
    g.g_ahead.push(Anode {
        a_type: AType::Disk,
        a_letter: letter,
        a_aicon,
        a_pappl: disk_label(letter),
        a_xspot,
        a_yspot,
        a_obid: NIL,
    });
    true
}

/// `remove_drive` (`deskins.c:262-283`) — drop the `AT_ISDISK` [`Anode`] whose
/// `a_letter` is `drive`'s letter. `false` when there is none.
fn remove_drive(g: &mut global::DeskState, drive: u32) -> bool {
    let letter = b'A' + drive as u8;
    match g.g_ahead.iter().position(|pa| pa.a_type == AType::Disk && pa.a_letter == letter) {
        Some(i) => {
            g.g_ahead.remove(i);
            true
        }
        None => false,
    }
}

/// `ins_devices` (`deskins.c:290-333`) — Options → *Install devices*: remove the
/// desktop icons of drives that no longer exist, then install one for every
/// drive that has none. Existing icons are left alone, position and label
/// included. Returns whether anything changed (the caller rebuilds the desktop
/// only then, `deskmain.c:847-852`).
pub(super) fn ins_devices() -> bool {
    // `current = dos_sdrv(dos_gdrv());` (`:301`) — the trap, read outside the
    // state lock. The `Long` it answers is a bitmap, so it is used unsigned.
    let current = dos_sdrv(dos_gdrv()) as u32;

    global::with(|g| {
        // "scan ANODEs and build bitmask of installed devices" (`:306-309`).
        let installed: u32 = g
            .g_ahead
            .iter()
            .filter(|pa| pa.a_type == AType::Disk)
            .map(|pa| 1u32 << (pa.a_letter.wrapping_sub(b'A') as u32 & 31))
            .fold(0, |acc, b| acc | b);

        let mut change = false;

        // "remove icons for non-existent devices" (`:314-325`).
        let drivebits = installed & !current;
        for drive in 0..BLKDEVNUM {
            if drivebits & (1 << drive) != 0 && remove_drive(g, drive) {
                change = true;
            }
        }

        // "install icons for devices without one" (`:330-341`).
        let drivebits = current & !installed;
        for drive in 0..BLKDEVNUM {
            if drivebits & (1 << drive) != 0 && install_drive(g, drive) {
                change = true;
            }
        }

        change
    })
    .unwrap_or(false)
}

// ── The icon set the stepper offers ────────────────────────────────────────

/// The icons `ID_UP`/`ID_DOWN` cycle through, as `(IG_* number, kind)` pairs.
///
/// EmuTOS steps over `0 .. G.g_numiblks-1`, its whole loaded icon resource
/// (`deskins.c:902-906`). NewTOS's equivalent is [`BUILTIN_ICONS`], which is
/// EmuTOS's built-in set **minus `IG_REMOV`** — there is no removable-disk
/// raster in the bitmaps this desktop inherited, so offering index 5 would step
/// onto a picture that does not exist. Seven entries, not eight.
///
/// The floppy is offered only when it can be used: `IG_FLOPPY` stays out unless
/// the drive map actually reports `A:` or `B:` — the machine this desktop runs
/// on has no floppy controller, and the menu already drops *Format* for the
/// same reason ([`super::menu`]'s module header). The second clause keeps a
/// node that **already** carries the floppy icon (a `NEWDESK.INF` written on a
/// machine that had one) steppable instead of silently rewriting it.
fn offered_icons(current: i16) -> Vec<(i16, IconKind)> {
    let drivemask = dos_sdrv(dos_gdrv()) as u32;
    let has_floppy = drivemask & 0b11 != 0;
    BUILTIN_ICONS
        .iter()
        .copied()
        .filter(|&(ig, _)| ig != super::icons::IG_FLOPPY || has_floppy || current == ig)
        .collect()
}

// ── Install icon: the `ADINSDSK` dialog (`deskins.c:781-978`) ───────────────

/// What [`install_desktop_icon`] did, in `deskins.c`'s sign convention:
/// `>0` the icon changed, `0` the user skipped it, `<0` the user cancelled
/// every further install.
type Change = i16;

/// `install_desktop_icon` (`deskins.c:781-978`) — edit the desktop icon
/// `existing` (an index into `G.g_ahead`), or create a new one when `None`.
///
/// The dialog offers what `ADINSDSK` offers: the drive identifier, the label,
/// the item type (Drive / Trash can / Printer) and the icon picture, stepped
/// with the two arrow gadgets. A **Drive** icon must end up with a non-blank
/// identifier; otherwise the dialog re-displays itself behind a `STDRIVID`
/// alert instead of accepting the node (`:917-918`, `:946-951`).
///
/// **Two named differences from the resource.**
///
/// 1. `ADINSDSK` declares `ID_ID` **without** `EDITABLE`
///    (`desk_rsc.c:936-940`: `FL3DBAK | 0x4000`, the latter being the
///    desktop-private `RIGHT_ALIGNED` alignment hint, `deskmain.c:106`), yet
///    `install_desktop_icon` passes `start_fld = ID_ID` to `form_do` and reads
///    the field back with `inf_sget`. That works because `fm_inifld` hands
///    `start_fld` through untouched (`aes/gemfmlib.c:153-160`) and `ob_edit`
///    never looks at the flag — so the caret *starts* in the identifier field
///    and the user can type there, but `fm_button` refuses to put it back
///    (`EDITABLE` gates the click, `gemfmlib.c` `fm_button`) and Tab walks only
///    the genuinely editable fields. The quirk is reproduced verbatim, flag and
///    start field both.
/// 2. EmuTOS nudges `ID_DOWN` by `ADJ3DSTD` before the dialog and back
///    afterwards (`deskins.c:826-831`, `:966-971`) because its resource was
///    laid out for flat buttons and a 3D indicator grows 3 px per side. This
///    tree is built fresh on every call with the grown rectangles already
///    accounted for, so there is nothing to nudge.
fn install_desktop_icon(existing: Option<usize>) -> Change {
    // ── the node the dialog edits ───────────────────────────────────────
    //
    // For a new icon, EmuTOS allocates the ANODE *before* the dialog and frees
    // it again on Skip/Cancel (`deskins.c:843-859`, `:937-943`); the spot is
    // taken before the allocation so the new node does not see itself
    // (`:789`). Both are reproduced; the index stays valid because nothing else
    // touches `G.g_ahead` while the dialog is modal.
    let icon_exists = existing.is_some();
    let Some(idx) = global::with(|g| match existing {
        Some(i) if i < g.g_ahead.len() => Some(i),
        Some(_) => None,
        None => {
            let (x, y) = ins_posdisk(g, 0, 0);
            let (a_xspot, a_yspot) = g.snap_icon(x, y);
            g.g_ahead.push(Anode {
                a_type: AType::Disk,
                a_letter: 0,
                a_aicon: IG_HARD,
                a_pappl: String::new(),
                a_xspot,
                a_yspot,
                a_obid: NIL,
            });
            Some(g.g_ahead.len() - 1)
        }
    })
    .flatten() else {
        return -1; // no desktop state — "don't try any more"
    };

    let (a_type, a_letter, a_aicon, a_pappl) =
        global::with(|g| (g.g_ahead[idx].a_type, g.g_ahead[idx].a_letter, g.g_ahead[idx].a_aicon, g.g_ahead[idx].a_pappl.clone()))
            .unwrap_or((AType::Disk, 0, IG_HARD, String::new()));

    // The icon list, and where the node's current icon sits in it
    // (`curr_icon` clamped into range, `deskins.c:895-900`).
    let list = offered_icons(a_aicon);
    let mut curr_icon = list.iter().position(|&(ig, _)| ig == a_aicon).unwrap_or(0);

    // ── the editable buffers ────────────────────────────────────────────
    // `inf_sset(tree, ID_ID, id)` / `inf_sset(tree, ID_LABEL, curr_label)`
    // (`deskins.c:876-889`): one identifier character, twelve label characters.
    let mut idbuf = [0u8; 2];
    if a_letter != 0 && a_letter != b' ' {
        idbuf[0] = a_letter;
    }
    let mut labelbuf = [0u8; LABEL_MAX + 1];
    let n = a_pappl.len().min(LABEL_MAX);
    labelbuf[..n].copy_from_slice(&a_pappl.as_bytes()[..n]);
    let curr_label = a_pappl.clone();

    static ID_TMPL: &[u8] = b"Identifier: _:\0";
    static ID_VALID: &[u8] = b"A\0";
    static LABEL_TMPL: &[u8] = b"Label: ____________\0";
    static LABEL_VALID: &[u8] = b"X\0";
    static EMPTY: [u8; 1] = [0];

    let mut id_ted = TedInfo {
        te_ptext: idbuf.as_mut_ptr() as u64,
        te_ptmplt: ID_TMPL.as_ptr() as u64,
        te_pvalid: ID_VALID.as_ptr() as u64,
        te_font: 3,
        te_junk1: 0,
        te_just: 0,
        te_color: TED_COLOR,
        te_junk2: 0,
        te_thickness: 1,
        te_txtlen: 2,
        te_tmplen: 15,
    };
    let mut label_ted = TedInfo {
        te_ptext: labelbuf.as_mut_ptr() as u64,
        te_ptmplt: LABEL_TMPL.as_ptr() as u64,
        te_pvalid: LABEL_VALID.as_ptr() as u64,
        te_font: 3,
        te_junk1: 0,
        te_just: 0,
        te_color: TED_COLOR,
        te_junk2: 0,
        te_thickness: 1,
        te_txtlen: LABEL_MAX as i16 + 1,
        te_tmplen: 20,
    };

    // The preview `ICONBLK`. `insert_icon` copies `G.g_iblist[nicon]` and blanks
    // its label (`deskins.c:772-776`, `ib.ib_ptext = ""`); ours is built to the
    // same cell width the desktop uses, so the bitmap is centred identically.
    let (g_wicon, g_hicon) = global::with(|g| (g.g_wicon, g.g_hicon)).unwrap_or((76, 40));
    let mut ib: IconBlk = build_iconblk(list[curr_icon].1, EMPTY.as_ptr() as u64, 0, g_wicon);

    // ── geometry ────────────────────────────────────────────────────────
    //
    // `ADINSDSK`'s rows, in its own order (`desk_rsc.c:924-1020`): title,
    // identifier, label, "Item type:" + the three radio buttons, the icon box
    // with "Icon:" beside it, then OK / Skip / Cancel. Pixel sizes follow the
    // rules `ADSETPRE` forced on the Preferences dialog (DESK01b): a `FL3DIND`
    // or `FL3DACT` button is grown by `ADJ3DSTD` = 2 px per side and takes its
    // border outside that, so neighbours need 3 px of air each.
    const MARGIN: i16 = 24;
    const FLD_H: i16 = 24;
    const BTN_H: i16 = 22;
    const GAP: i16 = 8;
    /// Height of the icon-preview box: the cell plus room for two 20-px
    /// steppers, each grown by 3 px, stacked with a gap.
    const IBOX_H: i16 = 60;
    const UPDN_W: i16 = 28;
    const UPDN_H: i16 = 20;

    let lbl_w = 20 * CELL_W; // "Label: ____________" is the longest template
    let type_w = 3 * (11 * CELL_W) + 2 * GAP;
    let ibox_w = GAP + g_wicon + GAP + UPDN_W + GAP;
    let dw = MARGIN + lbl_w.max(type_w).max(6 * CELL_W + ibox_w) + MARGIN;

    let y_title = MARGIN;
    let y_id = y_title + CELL_H + 12;
    let y_label = y_id + FLD_H + GAP;
    let y_type = y_label + FLD_H + 16;
    let y_radio = y_type + CELL_H + 6;
    let y_ibox = y_radio + BTN_H + 16;
    let y_btn = y_ibox + IBOX_H + 20;
    let dh = y_btn + 28 + MARGIN;
    let gx = MARGIN + 6 * CELL_W; // left edge of the icon box ("Icon:" + blank)

    static TITLE: &[u8] = b"INSTALL DESKTOP ICON\0";
    static TYPE_LBL: &[u8] = b"Item type:\0";
    static ICON_LBL: &[u8] = b"Icon:\0";
    static DRIVE: &[u8] = b" Drive \0";
    static TRASH: &[u8] = b" Trash can \0";
    static PRINT: &[u8] = b" Printer \0";
    // Characters 1 and 2 of the system font are the up and down arrows —
    // `ADINSDSK` labels its steppers `"\001"` / `"\002"` (`desk_rsc.c:990,996`).
    static UP: &[u8] = b"\x01\0";
    static DOWN: &[u8] = b"\x02\0";
    static OK: &[u8] = b" OK \0";
    static SKIP: &[u8] = b" Skip \0";
    static CANCEL: &[u8] = b" Cancel \0";
    /// `STDRIVID` (`desk_rsc.c:2542`) — "a disk icon needs a drive letter".
    static STDRIVID: &[u8] = b"[1][No drive identifier.][ Retry ]\0";

    let radio_w = 11 * CELL_W;
    // Each `RBUTTON` of `ADINSDSK` is `SELECTABLE | RBUTTON | FL3DIND`
    // (`desk_rsc.c:958` ff.).
    let rb = |spec: &'static [u8], x: i16, next: i16| {
        mk(G_BUTTON, SELECTABLE | RBUTTON | FL3DIND, spec.as_ptr() as u64, x, 0, radio_w, BTN_H, next, -1, -1)
    };

    #[rustfmt::skip]
    let mut tree = [
        // 0 ROOT
        mk_root(BOX_SPEC, dw, dh, ID_CNCL),
        // 1 title
        mk(G_STRING, 0, TITLE.as_ptr() as u64, (dw - 20 * CELL_W) / 2, y_title, 20 * CELL_W, CELL_H, ID_ID, -1, -1),
        // 2 ID_ID — NOT EDITABLE, exactly as `desk_rsc.c:936-940` declares it.
        mk(G_FBOXTEXT, FL3DBAK, &mut id_ted as *mut _ as u64, MARGIN, y_id, 15 * CELL_W + 8, FLD_H, ID_LABEL, -1, -1),
        // 3 ID_LABEL
        mk(G_FBOXTEXT, EDITABLE | FL3DBAK, &mut label_ted as *mut _ as u64, MARGIN, y_label, lbl_w + 8, FLD_H, 4, -1, -1),
        // 4 "Item type:"                 5 its G_IBOX      6/7/8 the radios
        mk(G_STRING, FL3DBAK, TYPE_LBL.as_ptr() as u64, MARGIN, y_type, 10 * CELL_W, CELL_H, 5, -1, -1),
        mk(G_IBOX, 0, 0, MARGIN, y_radio, type_w, BTN_H, ID_IBOX, ID_DRIVE, ID_DRIVE + 2),
        rb(DRIVE, 0, ID_DRIVE + 1),
        rb(TRASH, radio_w + GAP, ID_DRIVE + 2),
        rb(PRINT, 2 * (radio_w + GAP), 5),
        // 9 ID_IBOX                      10 ID_ICON        11/12 the steppers
        mk(G_BOX, FL3DBAK, IBOX_SPEC, gx, y_ibox, ibox_w, IBOX_H, 13, ID_ICON, ID_DOWN),
        mk(G_ICON, 0, &mut ib as *mut _ as u64, GAP, (IBOX_H - g_hicon) / 2, g_wicon, g_hicon, ID_UP, -1, -1),
        mk(G_BUTTON, SELECTABLE | TOUCHEXIT | FL3DACT, UP.as_ptr() as u64, GAP + g_wicon + GAP, 5, UPDN_W, UPDN_H, ID_DOWN, -1, -1),
        mk(G_BUTTON, SELECTABLE | TOUCHEXIT | FL3DACT, DOWN.as_ptr() as u64, GAP + g_wicon + GAP, IBOX_H - UPDN_H - 5, UPDN_W, UPDN_H, ID_IBOX, -1, -1),
        // 13 "Icon:"
        mk(G_STRING, FL3DBAK, ICON_LBL.as_ptr() as u64, MARGIN, y_ibox + (IBOX_H - CELL_H) / 2, 5 * CELL_W, CELL_H, ID_OK, -1, -1),
        // 14/15/16 OK / Skip / Cancel (`FL3DACT`, `desk_rsc.c:1004` ff.)
        mk(G_BUTTON, SELECTABLE | DEFAULT | EXIT | FL3DACT, OK.as_ptr() as u64, MARGIN, y_btn, 90, 28, ID_SKIP, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DACT, SKIP.as_ptr() as u64, (dw - 90) / 2, y_btn, 90, 28, ID_CNCL, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | LASTOB | FL3DACT, CANCEL.as_ptr() as u64, dw - MARGIN - 90, y_btn, 90, 28, 0, -1, -1),
    ];

    // Pre-select the item type (`deskins.c:869-885`); `deselect_all` is implicit
    // because `mk` leaves every `ob_state` at 0.
    let type_obj = match a_type {
        AType::Trash => ID_DRIVE + 1,
        AType::Printer => ID_DRIVE + 2,
        _ => ID_DRIVE,
    };
    tree[type_obj as usize].ob_state |= OB_SELECTED;

    // ── run it (`deskins.c:903-963`) ────────────────────────────────────
    let tree_ptr = tree.as_mut_ptr() as u64;
    let mut change: Change = 0;
    let mut new_letter = a_letter;
    let mut new_type = a_type;
    let mut new_label = curr_label.clone();

    start_dialog(tree_ptr);
    loop {
        let mut retry = false;
        // `start_fld` is `ID_ID` — see this function's doc-comment for why a
        // non-EDITABLE object is a legitimate start field.
        let exitobj = super::form_do(tree_ptr, ID_ID) & 0x7fff;
        let mut new_icon = curr_icon;

        match exitobj {
            ID_UP => new_icon = curr_icon.saturating_sub(1),
            ID_DOWN => new_icon = (curr_icon + 1).min(list.len() - 1),
            ID_OK => {
                // `pa->a_letter = '\0';` then the type switch (`:909-926`).
                new_letter = 0;
                match inf_gindex(&tree, ID_DRIVE as usize, 3) {
                    0 => {
                        new_type = AType::Disk;
                        new_letter = idbuf[0];
                        if new_letter == 0 || new_letter == b' ' {
                            retry = true; // "disks must have a letter"
                        }
                    }
                    1 => new_type = AType::Trash,
                    2 => new_type = AType::Printer,
                    _ => {}
                }
                let typed = cstr(&labelbuf);
                if typed != curr_label {
                    new_label = typed;
                }
                change = 1;
            }
            ID_CNCL => change = -1,
            _ => {} // ID_SKIP, and any object that cannot exit
        }
        // `tree[exitobj].ob_state &= ~SELECTED;` (`:943`).
        if let Some(o) = tree.get_mut(exitobj as usize) {
            o.ob_state &= !OB_SELECTED;
        }

        if retry {
            // `fun_alert(1, STDRIVID); draw_dial(tree); continue;` (`:946-951`).
            // `STDRIVID` is `"[1][No drive identifier.][ Retry ]"`
            // (`desk_rsc.c:2542`) — the button reads *Retry*, not *OK*, so the
            // string goes to `form_alert` verbatim instead of through
            // `dir::fun_alert`'s one-OK-button wrapper.
            change = 0;
            super::form_alert(1, STDRIVID.as_ptr());
            super::dir::draw_dial(tree_ptr);
            continue;
        }

        if exitobj != ID_UP && exitobj != ID_DOWN {
            break;
        }

        if new_icon != curr_icon {
            curr_icon = new_icon;
            // `insert_icon(tree, ID_ICON, curr_icon)` (`:958-961`).
            // The AES reads this block back through `tree[ID_ICON].ob_spec`,
            // which the compiler cannot see — hence the volatile write.
            // SAFETY: `ib` is a live local of this frame.
            unsafe {
                core::ptr::write_volatile(
                    &mut ib as *mut IconBlk,
                    build_iconblk(list[curr_icon].1, EMPTY.as_ptr() as u64, 0, g_wicon),
                )
            };
        }
        super::dir::draw_fld(tree_ptr, ID_IBOX);
    }
    end_dialog(tree_ptr);

    // ── commit (`deskins.c:909-943`) ────────────────────────────────────
    global::with(|g| {
        if change > 0 {
            let pa = &mut g.g_ahead[idx];
            pa.a_type = new_type;
            pa.a_letter = new_letter;
            pa.a_pappl = new_label;
            pa.a_aicon = ig_of(list[curr_icon].1);
        } else if !icon_exists {
            // "if (!icon_exists) app_free(pa);" (`:941-943`) — a cancelled
            // "create new" leaves no dangling node.
            g.g_ahead.remove(idx);
        }
    });

    change
}

// ── Install icon: the `ADINSWIN` dialog (`deskins.c:988-1222`) ──────────────

/// `is_executable` (`deskins.c:988-1004`) — does this file name name a
/// program?
///
/// EmuTOS's table is `exec_ext[] = { "TOS", "TTP", "PRG", "APP", "GTP" }`
/// (`:984`). Those five are exactly NewTOS's [`super::ftype::FileType::M68k`]
/// family; ADR-018 adds the native AArch64 family `.NAP`/`.NCP`/`.NTP`
/// ([`super::ftype::FileType::Native`]), which is what a program on *this*
/// machine is called. Both count, so the question is simply "not a document".
pub(super) fn is_executable(name: &[u8]) -> bool {
    !matches!(super::ftype::classify(name), super::ftype::FileType::Doc)
}

/// The `FNODE` fields `install_window_icon` reads off its argument
/// (`deskins.c:1061`, `pf->f_name` / `pf->f_pa` / the file-vs-folder type),
/// plus the owning window's `p_spec` — NewTOS caches no `f_pa` in the `FNODE`
/// (see `win_icalc` in [`super::win`]), so the node is re-resolved
/// from name + spec, which is the identical `app_afind_by_name` query
/// `win_icalc` ran when the item was built.
struct WinFile {
    /// `AT_ISFILE` or `AT_ISFOLD`, from `pf->f_attr & FA_SUBDIR`.
    atype: AType,
    /// `pf->f_name`.
    name: String,
    /// The window's `w_pnode.p_spec`, for the full-path `a_pappl` compare.
    spec: String,
}

/// `install_window_icon` (`deskins.c:1061-1222`) — the `ADINSWIN` dialog: bind
/// an icon to a **file or folder name**, so every item of that name in every
/// window shows it (the reader is `win_icalc`/`win_bldview`).
///
/// `pf` is the already-visible item whose icon is being edited; `None` is the
/// *Install icon* → *Window* case, where the name is typed instead and the
/// file/folder radio becomes choosable (`:1093-1113`).
///
/// Returns `deskins.c`'s sign convention: `>0` the icon changed, `0` the user
/// skipped, `<0` the user cancelled every further install.
///
/// **One named difference from the resource.** EmuTOS nudges `IW_DOWN` by
/// `ADJ3DSTD` before the dialog and back afterwards (`:1117-1123`, `:1210-
/// 1216`) because `ADINSWIN` was laid out for flat buttons and a 3D indicator
/// grows 2 px per side. This tree is built fresh on every call with the grown
/// rectangles already accounted for, exactly as [`install_desktop_icon`] is, so
/// there is nothing to nudge.
fn install_window_icon(pf: Option<&WinFile>) -> Change {
    // `pa = pf ? pf->f_pa : NULL;` (`:1085`), resolved rather than cached.
    let mut pa = pf.and_then(|f| super::inf::app_afind_by_name(f.atype, &f.spec, &f.name));

    // ── fill in the dialog (`:1090-1113`) ───────────────────────────────
    //
    // With an `FNODE` the name is fixed and not editable and the file/folder
    // radio is fixed and DISABLED; without one the name is typed and the radio
    // starts on *File*.
    let mut namebuf = [0u8; LEN_ZFNAME];
    let (name_editable, edit_start, is_folder) = match pf {
        Some(f) => {
            namebuf = super::dir::fmt_str(&f.name);
            (false, 0i16, f.atype == AType::Folder)
        }
        None => (true, IW_NAME, false),
    };

    // `curr_icon = pa ? pa->a_dicon : 0;` clamped into range (`:1115-1120`).
    let curr_dicon = pa.as_ref().map(|a| a.dicon).unwrap_or(0);
    let list = offered_icons(curr_dicon);
    let mut curr_icon = list.iter().position(|&(ig, _)| ig == curr_dicon).unwrap_or(0);

    static NAME_TMPL: &[u8] = b"Name: ________.___\0";
    /// `te_pvalid` `"F"` — upper-cased file-name characters
    /// (`desk_rs_tedinfo[33]`).
    static NAME_VALID: &[u8] = b"F\0";
    static EMPTY: [u8; 1] = [0];
    static TITLE: &[u8] = b"INSTALL WINDOW ICON\0";
    static TYPE_LBL: &[u8] = b"Item type:\0";
    static ICON_LBL: &[u8] = b"Icon:\0";
    static FOLDER: &[u8] = b" Folder \0";
    static FILE: &[u8] = b" File \0";
    // Characters 1 and 2 of the system font are the up and down arrows —
    // `ADINSWIN` labels its steppers `"\001"` / `"\002"` (`desk_rsc.c:2094,2100`).
    static UP: &[u8] = b"\x01\0";
    static DOWN: &[u8] = b"\x02\0";
    static INSTALL: &[u8] = b" Install \0";
    static REMOVE: &[u8] = b" Remove \0";
    static SKIP: &[u8] = b" Skip \0";
    static CANCEL: &[u8] = b" Cancel \0";
    /// `STNOMTCH` (`desk_rsc.c:2535`) — *Remove* was asked for a name that does
    /// not match the `ANODE` the dialog found. Passed to `form_alert` verbatim
    /// (buttons included), like [`install_desktop_icon`]'s `STDRIVID`.
    static STNOMTCH: &[u8] = b"[1][No matching file type.][  OK  ]\0";

    let mut name_ted = TedInfo {
        te_ptext: namebuf.as_mut_ptr() as u64,
        te_ptmplt: NAME_TMPL.as_ptr() as u64,
        te_pvalid: NAME_VALID.as_ptr() as u64,
        te_font: 3,
        te_junk1: 0,
        te_just: 0,
        te_color: TED_COLOR,
        te_junk2: 0,
        te_thickness: 1,
        te_txtlen: LEN_ZFNAME as i16 - 1,
        te_tmplen: 19,
    };

    // The preview `ICONBLK`, built exactly as `insert_icon` builds it
    // (`deskins.c:762-769`: a copy of `G.g_iblist[nicon]` with a blank label).
    let (g_wicon, g_hicon) = global::with(|g| (g.g_wicon, g.g_hicon)).unwrap_or((76, 40));
    let mut ib: IconBlk = build_iconblk(list[curr_icon].1, EMPTY.as_ptr() as u64, 0, g_wicon);

    // ── geometry ────────────────────────────────────────────────────────
    //
    // `ADINSWIN`'s rows, in its own order (`desk_rsc.c:2038-2135`): title, the
    // name field, "Item type:" + the two radio buttons, the icon box with
    // "Icon:" beside it, then Install / Remove on one row and Skip / Cancel on
    // the next. Pixel sizes follow the DESK01b rules the sibling dialogs use.
    const MARGIN: i16 = 24;
    const FLD_H: i16 = 24;
    const BTN_H: i16 = 22;
    const GAP: i16 = 8;
    /// Height of the icon-preview box: the cell plus room for two 20-px
    /// steppers, each grown by 3 px, stacked with a gap.
    const IBOX_H: i16 = 60;
    const UPDN_W: i16 = 28;
    const UPDN_H: i16 = 20;
    const BTN_W: i16 = 100;

    let fld_w = 19 * CELL_W + 8; // "Name: ________.___" plus the field border
    let radio_w = 13 * CELL_W;
    let type_w = 2 * radio_w + GAP;
    let ibox_w = GAP + g_wicon + GAP + UPDN_W + GAP;
    let btn_row_w = 2 * BTN_W + 3 * GAP;
    let dw = MARGIN + fld_w.max(type_w).max(6 * CELL_W + ibox_w).max(btn_row_w) + MARGIN;

    let y_title = MARGIN;
    let y_name = y_title + CELL_H + 12;
    let y_type = y_name + FLD_H + 16;
    let y_radio = y_type + CELL_H + 6;
    let y_ibox = y_radio + BTN_H + 16;
    let y_btn1 = y_ibox + IBOX_H + 20;
    let y_btn2 = y_btn1 + 28 + GAP;
    let dh = y_btn2 + 28 + MARGIN;
    let gx = MARGIN + 6 * CELL_W; // left edge of the icon box ("Icon:" + blank)
    let btn_x2 = MARGIN + BTN_W + GAP;

    // Each radio of `ADINSWIN` is `SELECTABLE | RBUTTON | FL3DIND`
    // (`desk_rsc.c:2068`, `:2074`).
    let rb = |spec: &'static [u8], x: i16, next: i16| {
        mk(G_BUTTON, SELECTABLE | RBUTTON | FL3DIND, spec.as_ptr() as u64, x, 0, radio_w, BTN_H, next, -1, -1)
    };
    let name_flags = if name_editable { EDITABLE | FL3DBAK } else { FL3DBAK };

    #[rustfmt::skip]
    let mut tree = [
        // 0 ROOT
        mk_root(BOX_SPEC, dw, dh, IW_CNCL),
        // 1 title
        mk(G_STRING, 0, TITLE.as_ptr() as u64, (dw - 19 * CELL_W) / 2, y_title, 19 * CELL_W, CELL_H, IW_NAME, -1, -1),
        // 2 IW_NAME
        mk(G_FBOXTEXT, name_flags, &mut name_ted as *mut _ as u64, MARGIN, y_name, fld_w, FLD_H, 3, -1, -1),
        // 3 "Item type:"                 4 IW_RBOX          5/6 the radios
        mk(G_STRING, FL3DBAK, TYPE_LBL.as_ptr() as u64, MARGIN, y_type, 10 * CELL_W, CELL_H, IW_RBOX, -1, -1),
        mk(G_IBOX, 0, 0, MARGIN, y_radio, type_w, BTN_H, IW_IBOX, IW_FOLD, IW_FILE),
        rb(FOLDER, 0, IW_FILE),
        rb(FILE, radio_w + GAP, IW_RBOX),
        // 7 IW_IBOX                      8 IW_ICON          9/10 the steppers
        mk(G_BOX, FL3DBAK, IBOX_SPEC, gx, y_ibox, ibox_w, IBOX_H, 11, IW_ICON, IW_DOWN),
        mk(G_ICON, 0, &mut ib as *mut _ as u64, GAP, (IBOX_H - g_hicon) / 2, g_wicon, g_hicon, IW_UP, -1, -1),
        mk(G_BUTTON, SELECTABLE | TOUCHEXIT | FL3DACT, UP.as_ptr() as u64, GAP + g_wicon + GAP, 5, UPDN_W, UPDN_H, IW_DOWN, -1, -1),
        mk(G_BUTTON, SELECTABLE | TOUCHEXIT | FL3DACT, DOWN.as_ptr() as u64, GAP + g_wicon + GAP, IBOX_H - UPDN_H - 5, UPDN_W, UPDN_H, IW_IBOX, -1, -1),
        // 11 "Icon:"
        mk(G_STRING, FL3DBAK, ICON_LBL.as_ptr() as u64, MARGIN, y_ibox + (IBOX_H - CELL_H) / 2, 5 * CELL_W, CELL_H, IW_INST, -1, -1),
        // 12/13 Install / Remove, 14/15 Skip / Cancel (`FL3DACT`, `:2112` ff.)
        mk(G_BUTTON, SELECTABLE | DEFAULT | EXIT | FL3DACT, INSTALL.as_ptr() as u64, MARGIN, y_btn1, BTN_W, 28, IW_REMV, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DACT, REMOVE.as_ptr() as u64, btn_x2, y_btn1, BTN_W, 28, IW_SKIP, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DACT, SKIP.as_ptr() as u64, MARGIN, y_btn2, BTN_W, 28, IW_CNCL, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | LASTOB | FL3DACT, CANCEL.as_ptr() as u64, btn_x2, y_btn2, BTN_W, 28, 0, -1, -1),
    ];

    // Pre-select (and, with an `FNODE`, disable) the item type (`:1096-1112`).
    tree[if is_folder { IW_FOLD } else { IW_FILE } as usize].ob_state |= OB_SELECTED;
    if pf.is_some() {
        tree[IW_FILE as usize].ob_state |= global::DISABLED;
        tree[IW_FOLD as usize].ob_state |= global::DISABLED;
    }

    // ── run it (`deskins.c:1125-1208`) ──────────────────────────────────
    let tree_ptr = tree.as_mut_ptr() as u64;
    let mut change: Change = 0;

    start_dialog(tree_ptr);
    loop {
        let exitobj = super::form_do(tree_ptr, edit_start) & 0x7fff;
        let mut new_icon = curr_icon;

        match exitobj {
            IW_UP => new_icon = curr_icon.saturating_sub(1),
            IW_DOWN => new_icon = (curr_icon + 1).min(list.len() - 1),
            IW_INST | IW_REMV => {
                // `inf_sget(tree, IW_NAME, temp); unfmt_str(temp, new_name);`
                let new_name = super::dir::unfmt_str(&namebuf);
                // Every `break` below is EmuTOS's `switch` break, i.e. "treat
                // as skip" with `change` left at 0 — hence the labelled block.
                'commit: {
                    if new_name.is_empty() {
                        break 'commit;
                    }
                    // "type = (tree[IW_FILE].ob_state & SELECTED) ? AT_ISFILE
                    //  : AT_ISFOLD;" (`:1151`)
                    let atype = if tree[IW_FILE as usize].ob_state & OB_SELECTED != 0 {
                        AType::File
                    } else {
                        AType::Folder
                    };
                    if pf.is_none() {
                        // `app_afind_by_name(type, AF_ISDESK, "", new_name, &dummy)`
                        pa = super::inf::app_afind_by_name(atype, "", &new_name);
                    }
                    let Some(found) = pa.as_ref() else {
                        break 'commit; // "Null anode ptr … skipping" (`:1157-1161`)
                    };
                    // "set flag if the names are exactly the same" (`:1164`)
                    let identical = found.pdata == new_name;
                    if exitobj == IW_INST {
                        let idx = if identical {
                            found.idx
                        } else {
                            let idx = super::inf::allocate_window_anode(atype);
                            super::inf::set_anode_data(idx, &new_name);
                            idx
                        };
                        super::inf::set_icon(idx, ig_of(list[curr_icon].1));
                    } else {
                        if !identical {
                            // "no matching file type" (`:1183-1186`): the alert
                            // replaces the dialog, which then closes.
                            super::form_alert(1, STNOMTCH.as_ptr());
                            break 'commit;
                        }
                        super::inf::app_free(found.idx);
                    }
                    change = 1;
                }
            }
            IW_CNCL => change = -1, // FALLTHROUGH to IW_SKIP, which does nothing
            _ => {}                 // IW_SKIP, and any object that cannot exit
        }

        // `install_window_icon` breaks **before** clearing SELECTED, unlike
        // `install_desktop_icon` (`:1196-1201` vs `:943-945`).
        if exitobj != IW_UP && exitobj != IW_DOWN {
            break;
        }
        if let Some(o) = tree.get_mut(exitobj as usize) {
            o.ob_state &= !OB_SELECTED;
        }

        if new_icon != curr_icon {
            curr_icon = new_icon;
            // `insert_icon(tree, IW_ICON, curr_icon)` (`:1203-1204`). The AES
            // reads this block back through `tree[IW_ICON].ob_spec`, which the
            // compiler cannot see — hence the volatile write.
            // SAFETY: `ib` is a live local of this frame.
            unsafe {
                core::ptr::write_volatile(
                    &mut ib as *mut IconBlk,
                    build_iconblk(list[curr_icon].1, EMPTY.as_ptr() as u64, 0, g_wicon),
                )
            };
        }
        super::dir::draw_fld(tree_ptr, IW_IBOX);
    }
    end_dialog(tree_ptr);

    change
}

/// `ins_icon` (`deskins.c:1234-1311`) — Options → *Install icon*. `sobj` is the
/// first selected object of the current root (`0` for none). With nothing
/// selected `STICNTYP` asks which kind of icon to make and a brand new one is
/// created; with desktop icons selected each is edited in turn, one dialog
/// each; otherwise the selection is taken to be inside the current window and
/// each selected file or folder gets the `ADINSWIN` dialog.
///
/// Returns EmuTOS's signal: `>0` = rebuild the desktop, `0` = nothing changed,
/// `<0` = rebuild the windows. The window branch's sign is **inverted**
/// (`rebuild--` on success, `:1298`), which is what tells `deskmain`'s
/// `IICNITEM` arm to `win_bdall`/`win_shwall` instead of `app_blddesk`.
pub(super) fn ins_icon(sobj: i16) -> i16 {
    /// `STICNTYP` (`desk_rsc.c:2534`) — desktop icon, window icon, or neither.
    static STICNTYP: &[u8] = b"[1][Please select icon type.][Desktop|Window|Cancel]\0";

    if sobj == 0 {
        // `:1246-1265`, the `CONF_WITH_WINDOW_ICONS` arm: `1` builds a desktop
        // icon and answers "rebuild the desktop", `2` a window icon and answers
        // "rebuild the windows"; a dialog the user skipped or cancelled
        // (`<= 0`) falls through to "nothing changed", as does *Cancel*.
        return match super::form_alert(1, STICNTYP.as_ptr()) {
            1 => i16::from(install_desktop_icon(None) > 0),
            2 => -i16::from(install_window_icon(None) > 0),
            _ => 0,
        };
    }

    let mut rebuild = 0i16;

    // "handle one or more desktop icons" (`:1269-1288`) — entered only when the
    // first selected object resolves to a desktop `ANODE`.
    if global::with(|g| g.app_afind_by_id(sobj)).flatten().is_some() {
        let mut obj = sobj;
        while obj != 0 {
            let pa = global::with(|g| g.app_afind_by_id(obj)).flatten();
            if let Some(i) = pa {
                let rc = install_desktop_icon(Some(i));
                if rc > 0 {
                    rebuild += 1;
                }
                if rc < 0 {
                    break;
                }
            }
            // `win_isel(G.g_screen, DROOT, sobj)` — the literal `DROOT`, not
            // `G.g_croot` (`deskins.c:1275`; see `deskins.md`'s precision note).
            obj = global::with(|g| g.win_isel(DROOT, obj)).unwrap_or(0);
        }
        return rebuild;
    }

    // "handle one or more window icons" (`:1290-1306`). EmuTOS walks the live
    // `p_flist` across the dialogs; the selected entries are snapshotted here
    // instead, because the dialog issues AES traps and cannot run while the
    // desktop lock is held. Nothing the dialog does touches the file list, so
    // the two walks see the same entries.
    let files: Vec<WinFile> = global::with(|g| {
        let Some(slot) = g.win_slot(g.g_cwin) else {
            return Vec::new(); // `pw = win_find(G.g_cwin); if (pw)`
        };
        let wn = &g.windows.slots[slot];
        wn.w_pnode
            .p_flist
            .iter()
            .filter(|f| f.f_selected)
            .map(|f| WinFile {
                atype: if f.is_dir() { AType::Folder } else { AType::File },
                // Widened byte-by-byte, as `win_icalc` and `dir::unfmt_str`
                // do — the three have to produce the same key for a name
                // outside ASCII.
                name: f.name_bytes().iter().map(|&b| b as char).collect(),
                spec: wn.w_pnode.p_spec.clone(),
            })
            .collect()
    })
    .unwrap_or_default();

    for wf in &files {
        let rc = install_window_icon(Some(wf));
        if rc > 0 {
            rebuild -= 1;
        }
        if rc < 0 {
            break;
        }
    }
    rebuild
}

/// `rmv_icon` (`deskins.c:1317-1335`) — Options → *Remove desktop icon*: drop
/// the [`Anode`] of every selected desktop icon. Returns how many were removed;
/// the caller rebuilds the desktop when that is non-zero
/// (`deskmain.c:875-881`).
///
/// EmuTOS's `if (pa->a_flags & AF_ISDESK)` guard (`:1327`) has no counterpart:
/// `G.g_ahead` holds desktop icons only, so a node found by `app_afind_by_id`
/// **is** an `AF_ISDESK` node. A selected object that resolves to no node at
/// all is skipped, as there.
pub(super) fn rmv_icon(sobj: i16) -> i16 {
    let mut removed = 0i16;
    let mut obj = sobj;
    while obj != 0 {
        // Read the successor before the removal: `app_free` only unlinks the
        // node, it does not touch the object tree, so the walk is unaffected —
        // but taking the next index first keeps that independent of the list.
        let next = global::with(|g| g.win_isel(DROOT, obj)).unwrap_or(0);
        global::with(|g| {
            if let Some(i) = g.app_afind_by_id(obj) {
                g.g_ahead.remove(i);
                removed += 1;
            }
        });
        obj = next;
    }
    removed
}

// ── Small local helpers ─────────────────────────────────────────────────────

/// `inf_gindex` (`util/optimize.c:134-147`) — the index, counted from the
/// start, of the first `SELECTED` object in `tree[baseobj .. baseobj+numobj]`,
/// or `-1`. A local copy: [`super::inf`]'s is private to the Preferences
/// dialog, and EmuTOS's lives in `util/`, shared by both call sites.
fn inf_gindex(tree: &[Object], baseobj: usize, numobj: usize) -> i16 {
    for (n, obj) in tree.iter().skip(baseobj).take(numobj).enumerate() {
        if obj.ob_state & OB_SELECTED != 0 {
            return n as i16;
        }
    }
    -1
}

/// The NUL-terminated contents of an edit buffer as an owned string.
fn cstr(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// Build one dialog `OBJECT` — the same constructor the sibling dialogs use.
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

// ── Headless self-test (not wired into any boot path) ───────────────────────

/// `ins_devices` / `rmv_icon` model self-test: drive the icon list through the
/// same calls the two menu entries make and check what comes out. Printed one
/// line per case, like the other `selftest_*` entry points; it needs no
/// keyboard, so it covers everything about DESK03/DESK04 except the dialog's
/// text entry.
///
/// **Mutates `G.g_ahead`** (not the SD image) — it restores the list it found
/// before returning, so a desktop that is already up is left as it was.
pub fn selftest() {
    use crate::kprintln;

    kprintln!("[desk-ins] Install-devices / Remove-icon self-test");
    let mut pass = 0u32;
    let mut fail = 0u32;
    let mut check = |name: &str, cond: bool| {
        if cond {
            pass += 1;
            kprintln!("[desk-ins]  PASS {}", name);
        } else {
            fail += 1;
            kprintln!("[desk-ins]  FAIL {}", name);
        }
    };

    let Some(saved) = global::with(|g| g.g_ahead.clone()) else {
        kprintln!("[desk-ins] no desktop state — skipped");
        return;
    };
    let current = dos_sdrv(dos_gdrv()) as u32;
    let ndrives = current.count_ones() as usize;

    // (1) On a desktop that already has every drive, Install devices is a no-op.
    check("ins_devices on a complete desktop reports no change", !ins_devices());

    // (2) Remove every disk icon, then let Install devices put them back.
    let removed = global::with(|g| {
        let before = g.g_ahead.len();
        g.g_ahead.retain(|pa| pa.a_type != AType::Disk);
        before - g.g_ahead.len()
    })
    .unwrap_or(0);
    check("every drive had an icon to remove", removed == ndrives);
    check("ins_devices reports a change after the icons were removed", ins_devices());
    let (disks, letters_ok) = global::with(|g| {
        let disks: Vec<u8> = g.g_ahead.iter().filter(|pa| pa.a_type == AType::Disk).map(|pa| pa.a_letter).collect();
        let ok = disks.iter().all(|&l| current & (1 << (l - b'A') as u32) != 0);
        (disks.len(), ok)
    })
    .unwrap_or((0, false));
    check("one icon per drive in the map", disks == ndrives);
    check("every installed icon names a drive that exists", letters_ok);
    check(
        "a hard disk gets IG_HARD, a floppy IG_FLOPPY",
        global::with(|g| {
            g.g_ahead.iter().filter(|pa| pa.a_type == AType::Disk).all(|pa| {
                let want = if pa.a_letter > b'B' { super::icons::IG_HARD } else { super::icons::IG_FLOPPY };
                pa.a_aicon == want
            })
        })
        .unwrap_or(false),
    );
    check(
        "no two icons share a grid cell",
        global::with(|g| {
            let icw = g.g_icw.max(1);
            let ich = g.g_ich.max(1);
            let mut cells: Vec<(i16, i16)> =
                g.g_ahead.iter().map(|pa| (pa.a_xspot / icw, pa.a_yspot / ich)).collect();
            cells.sort_unstable();
            let n = cells.len();
            cells.dedup();
            cells.len() == n
        })
        .unwrap_or(false),
    );

    // (3) A drive that vanishes from the map loses its icon. Install a fake `Z:`
    //     that the map cannot contain, then reconcile.
    global::with(|g| {
        g.g_ahead.push(Anode {
            a_type: AType::Disk,
            a_letter: b'Z',
            a_aicon: super::icons::IG_HARD,
            a_pappl: disk_label(b'Z'),
            a_xspot: 0,
            a_yspot: 0,
            a_obid: NIL,
        })
    });
    check("ins_devices reports a change for a vanished drive", ins_devices());
    check(
        "the icon of the vanished drive is gone",
        global::with(|g| !g.g_ahead.iter().any(|pa| pa.a_letter == b'Z')).unwrap_or(false),
    );

    // (4) The Trash and Printer fixtures are never touched by Install devices.
    check(
        "Trash and Printer survive Install devices",
        global::with(|g| {
            g.g_ahead.iter().any(|pa| pa.a_type == AType::Trash)
                && g.g_ahead.iter().any(|pa| pa.a_type == AType::Printer)
        })
        .unwrap_or(false),
    );

    // (5) `rmv_icon` removes exactly the selected icons. Build the object tree,
    //     select the first icon, and run the menu entry's body.
    global::with(|g| g.app_blddesk());
    let first = global::with(|g| g.g_screen[DROOT as usize].ob_head).unwrap_or(NIL);
    if first != NIL {
        let before = global::with(|g| g.g_ahead.len()).unwrap_or(0);
        global::with(|g| g.g_screen[first as usize].ob_state |= OB_SELECTED);
        let n = rmv_icon(global::with(|g| g.win_isel(DROOT, 0)).unwrap_or(0));
        check("rmv_icon removed one icon", n == 1);
        check("the list is one shorter", global::with(|g| g.g_ahead.len()).unwrap_or(0) + 1 == before);
    } else {
        check("the desktop had an icon to select", false);
    }

    // (6) The icon set the stepper offers.
    let list = offered_icons(IG_HARD);
    check(
        "the stepper offers no icon NewTOS has no raster for",
        list.iter().all(|&(ig, _)| icon_kind(ig).is_some()),
    );
    check(
        "the floppy icon is offered exactly when a floppy drive exists",
        list.iter().any(|&(ig, _)| ig == super::icons::IG_FLOPPY) == (current & 0b11 != 0),
    );

    // Put the desktop back the way it was.
    global::with(|g| {
        g.g_ahead = saved;
        g.app_blddesk();
    });

    kprintln!("[desk-ins] install/remove self-test: {} passed, {} failed", pass, fail);
}
