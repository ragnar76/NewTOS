//! `desk::dir` — the desktop's file-operation engine (`d_doop`/`dir_op`), the
//! drag-and-drop dispatch tree (`fun_drag`), and the menu/drag action glue
//! (`fun_op`/`fun_del`/`fun_selectall`/`fun_mkdir`/`fun_rename`), per
//! `docs/api/desktop/deskdir.md` + `deskfun.md`.
//!
//! Reimplements EmuTOS `deskdir.c` (the recursive copy/move/delete/rename
//! subtree walker) and the `deskfun.c` action layer in idiomatic `#![no_std]`
//! Rust on top of the GEMDOS trap ([`super::dosbind`]).
//!
//! ## What is contract, what is free (`docs/COMPATIBILITY.md`)
//!
//! The **observable filesystem effect** — which GEMDOS calls happen, in what
//! order, with what net result — is the contract (`deskdir.md`). The path
//! representation, the recursion strategy and the *interactive* error/overwrite
//! UI are explicitly free to replace. Accordingly:
//!
//! * EmuTOS mutates a single fixed on-stack path buffer per level
//!   (`add_path`/`sub_path`/`add_fname`); NewTOS uses per-level owned [`String`]s
//!   — the recursion-depth and path-length limits stay observably the same
//!   ([`MAX_LEVEL`]).
//! * EmuTOS's per-item **skip/retry/abort on an I/O error**
//!   (`d_dofdel`/`d_dofcopy`'s alert loops, `deskdir.c:237-268,396-414`) is
//!   replaced by the NewTOS default: best-effort skip, and one summary
//!   [`fun_alert`] at the end.
//!
//! The prompts that *are* EmuTOS's are all here. Two of them the user can
//! switch off in *Set preferences* (DESK13):
//!
//! * the **copy/move/delete confirmation** with its file and folder counts
//!   ([`dir_op`]'s `ADCPYDEL` box, `deskdir.c:939,951-965`), gated on
//!   `g_ccopypref`/`g_cdelepref`; and
//! * the **name-conflict dialog** before a copy overwrites an existing file
//!   ([`output_fname`]'s `ADCPALER` box, `deskdir.c:299-397`), gated on
//!   `g_covwrpref`.
//!
//! Two more cannot be switched off (DESK24/DESK25):
//!
//! * the **same dialog for a folder** whose name is already taken at the
//!   destination ([`output_path`]/[`get_new_name`], `deskdir.c:700-779`) —
//!   *not* gated on `g_covwrpref`, because a folder that is merged into or
//!   renamed over is a decision, not an overwrite; and
//! * the **whole-drive delete** question ([`dir_op`]'s `STDELDIS`,
//!   `deskdir.c:971-975`), which hangs on the source being a desktop **disk**
//!   icon (`icontype == AT_ISDISK`).
//!
//! Because the confirmation lives in [`dir_op`], as it does in EmuTOS, the
//! engine is no longer dialog-free: [`selftest`] drives the box too and needs
//! the AES up (it always needed [`fun_alert`] one level higher, in [`fun_op`]).

use alloc::string::String;
use alloc::vec::Vec;

use tos_api::aes::object::{Object, TedInfo};
use tos_api::vdi::font::{CELL_H, CELL_W};
use tos_types::error::E_OK;

use super::dosbind::{
    dos_close, dos_create, dos_delete, dos_mkdir, dos_open, dos_read, dos_rename, dos_rmdir,
    dos_sdta, dos_setdt, dos_sfirst, dos_snext, dos_write,
};
use super::global::{self, DeskState, DROOT, NIL};
use super::inf::{
    end_dialog, inf_gindex, mk, mk_root, start_dialog, BOX_SPEC, FL3DACT, FL3DBAK, TED_COLOR,
};
use super::model::{AType, FNode, FA_SUBDIR};
use super::win::{DESKWH, OB_SELECTED};

// ── `OBJECT` graphic types / flags (`obdefs.h`), as in the sibling dialogs ──

const G_BUTTON: u16 = 26;
const G_STRING: u16 = 28;
const G_FBOXTEXT: u16 = 30;

const SELECTABLE: u16 = 0x0001;
const DEFAULT: u16 = 0x0002;
const EXIT: u16 = 0x0004;
const EDITABLE: u16 = 0x0008;
const LASTOB: u16 = 0x0020;

/// `TE_RIGHT` (`obdefs.h`) — the `te_just` of both count fields
/// (`desk_rs_tedinfo[20,21]`, `desk_rsc.c:135-143`).
const TE_RIGHT: i16 = 1;

/// `LEN_ZFNAME` (`include/sysconf.h:52`) — the longest file name including its
/// separator; the width of every `ml_f*` buffer in `deskdir.c`.
const LEN_ZFNAME: usize = 13;

/// Attribute mask for the recursive scan (EmuTOS `ALLFILES` =
/// `FA_SUBDIR|FA_SYSTEM|FA_HIDDEN`): a subtree operation must see hidden and
/// system files too, unlike the display scan's `DISPATTR`.
const ALLFILES: u8 = 0x16;

/// Recursion-depth guard (EmuTOS `MAX_LEVEL`, `deskconf.h:44`). A folder tree
/// deeper than this aborts that subtree (the EmuTOS `STFO8DEE` alert path).
pub const MAX_LEVEL: i16 = 12;

/// Copy buffer size (heap). EmuTOS sizes this to available ST-RAM up to
/// `MAX_CLUS_SIZE` (32 KiB); NewTOS uses a fixed 32 KiB heap buffer — the size
/// only affects copy throughput, not the result.
const COPY_BUF: usize = 32 * 1024;

/// `MODE_CTRL` (`aesdefs.h`) — the Ctrl modifier toggles Copy↔Move on a drag.
const MODE_CTRL: u16 = 0x04;

/// A file/folder operation (EmuTOS `OP_*` + the internal `OP_RENAME`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    /// `OP_COUNT` — tally files/dirs/size without mutating anything.
    Count,
    /// `OP_DELETE` — delete files and (recursively) folders.
    Delete,
    /// `OP_COPY` — copy files/folders to a destination directory.
    Copy,
    /// `OP_MOVE` — copy then delete the source (cross-drive) or `Frename`
    /// (same drive, via the [`dir_op`] move→rename optimisation).
    Move,
    /// `OP_RENAME` — same-drive move via `Frename` (internal, not user-visible).
    Rename,
}

/// Running totals for a directory operation (EmuTOS `DIRCOUNT`).
#[derive(Clone, Copy, Default, Debug)]
pub struct DirCount {
    /// Number of files.
    pub files: u32,
    /// Number of folders.
    pub dirs: u32,
    /// Sum of file sizes in bytes.
    pub size: u64,
}

/// One selected source entry the top-level [`dir_op`] iterates (a resolved
/// snapshot of an [`FNode`], so the engine does not hold the desktop lock while
/// it does I/O).
#[derive(Clone)]
pub struct SelEntry {
    /// `NAME.EXT\0` FAT display name.
    pub name: [u8; 14],
    /// Whether this entry is a sub-directory.
    pub is_dir: bool,
    /// File length (0 for a folder).
    pub size: u32,
    /// Packed FAT time.
    pub time: u16,
    /// Packed FAT date.
    pub date: u16,
}

impl SelEntry {
    /// From a model [`FNode`].
    pub fn from_fnode(f: &FNode) -> Self {
        Self {
            name: f.f_name,
            is_dir: f.is_dir(),
            size: f.f_size,
            time: f.f_time,
            date: f.f_date,
        }
    }

    /// The display name as a `&str` (up to the NUL).
    fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&c| c == 0).unwrap_or(self.name.len());
        core::str::from_utf8(&self.name[..end]).unwrap_or("")
    }
}

// ── path helpers (owned strings, no fixed-buffer mutation) ──────────────────

/// One raw directory entry from the recursive scan.
struct RawEntry {
    name: [u8; 14],
    attr: u8,
    size: u32,
    time: u16,
    date: u16,
}

impl RawEntry {
    fn is_dir(&self) -> bool {
        self.attr & FA_SUBDIR != 0
    }
    fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&c| c == 0).unwrap_or(self.name.len());
        core::str::from_utf8(&self.name[..end]).unwrap_or("")
    }
}

/// Append `name` to a directory path that already ends with `\` (e.g.
/// `join_leaf("C:\\SUB\\", "F.TXT") == "C:\\SUB\\F.TXT"`).
fn join_leaf(dir: &str, name: &str) -> String {
    let mut s = String::from(dir);
    s.push_str(name);
    s
}

/// A leaf path → its directory form (`"C:\\SUB" -> "C:\\SUB\\"`).
fn as_dir(leaf: &str) -> String {
    let mut s = String::from(leaf);
    s.push('\\');
    s
}

/// A directory path ending with `\` → its leaf form (`"C:\\SUB\\" -> "C:\\SUB"`).
fn strip_dir(dir: &str) -> &str {
    dir.strip_suffix('\\').unwrap_or(dir)
}

/// Scan directory `dir` (must end with `\`) via GEMDOS `Fsfirst`/`Fsnext` with
/// the [`ALLFILES`] mask, collecting **every** entry (files, folders, hidden,
/// system) except the `.`/`..` pseudo-entries. Fully drains the search before
/// returning, so a recursive caller can safely start a fresh scan afterwards
/// (the current DTA is a single global — EmuTOS uses a per-level private DTA
/// for the same reason; collecting first is the clean equivalent).
fn scan_folder(dir: &str) -> Vec<RawEntry> {
    let mut spec = String::from(dir);
    spec.push_str("*.*");

    // The DTA the search writes into. It is a kernel-stack buffer, and the
    // trap keeps it in the process-less caller's DTA cell (`dosbind`'s module
    // documentation, "What the trap changes"); `dos_sdta` must therefore be
    // re-issued for every scan, exactly as EmuTOS re-points `G.g_wdta`
    // (`deskdir.c:195-196`).
    let mut dta = [0u8; 44];
    dos_sdta(dta.as_mut_ptr());

    let mut out = Vec::new();
    let mut rc = dos_sfirst(&spec, ALLFILES);
    while rc == E_OK {
        let mut name = [0u8; 14];
        name.copy_from_slice(&dta[30..44]);
        let is_dot = name[0] == b'.' && (name[1] == 0 || (name[1] == b'.' && name[2] == 0));
        if !is_dot {
            out.push(RawEntry {
                name,
                attr: dta[21],
                time: u16::from_le_bytes([dta[22], dta[23]]),
                date: u16::from_le_bytes([dta[24], dta[25]]),
                size: u32::from_le_bytes([dta[26], dta[27], dta[28], dta[29]]),
            });
        }
        rc = dos_snext();
    }
    out
}

/// Whether `src` is `dst` itself or a parent of it (EmuTOS `source_is_parent`):
/// copying/moving a folder into itself or a descendant. Both are fully-qualified
/// leaf paths (no wildcard).
fn source_is_parent(src: &str, dst: &str) -> bool {
    if !dst.starts_with(src) {
        return false;
    }
    // Equal (self) or the next byte is a separator (a descendant).
    dst.len() == src.len() || dst.as_bytes()[src.len()] == b'\\'
}

// ── The operation's two dialogs (`ADCPYDEL` / `ADCPALER`) ──────────────────
//
// EmuTOS keeps `ml_havebox` and the running counts in file-scope statics
// (`deskdir.c:58-61`); NewTOS threads one [`OpCtx`] through the recursion
// instead, so the engine carries no hidden state.

/// EmuTOS's `more` tri-state, the value every `d_do*` routine returns and
/// every caller tests with `if (!more) break;` (`deskdir.c:232-236,296-298`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum More {
    /// `1` — the item was processed.
    Ok,
    /// `-1` — the item was deliberately **not** processed (the user pressed
    /// *Skip*, or a copy onto itself was declined); the operation continues and
    /// this is not an error.
    Skip,
    /// `0` — stop the whole operation (the user pressed *Stop*).
    Stop,
    /// A NewTOS distinction inside EmuTOS's `-1`: the item failed for an I/O
    /// reason. EmuTOS asks skip/retry/abort at this point
    /// (`deskdir.c:396-414`); NewTOS continues and reports one summary alert at
    /// the end (this module's header), so control flow matches `-1` while the
    /// failure still reaches the user.
    Failed,
}

/// The live state one file operation carries through the recursion.
struct OpCtx {
    /// The `ADCPYDEL` tree while it is on screen, or `0` — EmuTOS's
    /// `tree`/`NULL` (`deskdir.c:911-918`), which is what its every `if (tree)`
    /// tests. `0` for `OP_COUNT`, and for a direct [`d_dofcopy`] call.
    tree: u64,
    /// `ml_havebox` (`deskdir.c:60`) — a dialog box is currently displayed, so
    /// the next one redraws in place instead of reserving its own area.
    havebox: bool,
    /// `G.g_covwrpref`, read once at [`dir_op`] entry: the box is modal, so it
    /// cannot change while the operation runs.
    covwr: bool,
    /// Files still to go, shown in `CDFILES` and counted down as the walk
    /// proceeds (`deskdir.c:676,1053`).
    files: u32,
    /// Folders still to go, shown in `CDFOLDS` (`deskdir.c:592`).
    dirs: u32,
    /// Set once the user pressed *Stop*; every loop checks it, which is what
    /// EmuTOS's `more == 0` propagation does.
    stop: bool,
}

impl OpCtx {
    /// The dialog-free context a direct engine call uses ([`selftest`]).
    fn quiet() -> Self {
        OpCtx { tree: 0, havebox: false, covwr: false, files: 0, dirs: 0, stop: false }
    }

    /// One file processed: count it down in `CDFILES` and repaint that field
    /// (`deskdir.c:674-678`, `:1051-1056`). A no-op without a box.
    fn file_done(&mut self) {
        if self.tree == 0 {
            return;
        }
        self.files = self.files.saturating_sub(1);
        inf_numset(self.tree, CDFILES, self.files);
        draw_fld(self.tree, CDFILES);
    }

    /// One folder finished: the same for `CDFOLDS` (`deskdir.c:590-594`).
    fn dir_done(&mut self) {
        if self.tree == 0 {
            return;
        }
        self.dirs = self.dirs.saturating_sub(1);
        inf_numset(self.tree, CDFOLDS, self.dirs);
        draw_fld(self.tree, CDFOLDS);
    }
}

// `ADCPYDEL` object indices (`desk_rsc.h:136-141`). Object 1 is `CDTITLE`, the
// `COPY`/`MOVE`/`DELETE FILE(S)` heading; it is written when the tree is built
// rather than patched afterwards as EmuTOS does (`deskdir.c:907,940,944`), so
// it needs no name here.
/// `CDFOLDS` — the remaining-folder count field.
const CDFOLDS: i16 = 2;
/// `CDFILES` — the remaining-file count field.
const CDFILES: i16 = 3;
/// `CDOK` — the confirmation button; `CDCNCL` follows it.
const CDOK: i16 = 4;

// `ADCPALER` object indices (`desk_rsc.h:143-149`).
/// `CACURRNA` — the (read-only) source name.
const CACURRNA: i16 = 2;
/// `CACOPYNA` — the editable destination name.
const CACOPYNA: i16 = 3;
/// `CAOK` — the first of the three exit buttons.
const CAOK: i16 = 4;
/// `CASKIP` — skip this one file (`deskdir.c:348`).
const CASKIP: i16 = 5;
/// `CASTOP` — stop the whole operation (`deskdir.c:346`).
const CASTOP: i16 = 6;

/// `inf_numset` (`deskinf.c:406-420`) — `sprintf(ted->te_ptext, "%*lu",
/// ted->te_txtlen - 1, value)`: the number right-justified, space-padded, in
/// the field's own width, so the template's underscores are all consumed.
///
/// # Safety
/// `tree` must point at a live object array and `obj` must be a `G_FBOXTEXT`
/// (or other TEDINFO) object of it whose `te_ptext` buffer is at least
/// `te_txtlen` bytes. Both hold for the trees built in this module.
fn inf_numset(tree: u64, obj: i16, value: u32) {
    // SAFETY: see the contract above — every call site passes one of this
    // module's own stack trees and one of its own object indices.
    let (ptext, len) = unsafe {
        let o = &*(tree as *const Object).add(obj as usize);
        let ted = &*(o.ob_spec as *const TedInfo);
        (ted.te_ptext as *mut u8, (ted.te_txtlen - 1).max(0) as usize)
    };
    // SAFETY: as above — `te_ptext` points at a `te_txtlen`-byte buffer.
    let buf = unsafe { core::slice::from_raw_parts_mut(ptext, len + 1) };

    let mut digits = [0u8; 10];
    let mut n = 0usize;
    let mut v = value;
    loop {
        digits[n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 || n == digits.len() {
            break;
        }
    }
    buf.fill(b' ');
    for (k, &d) in digits[..n].iter().enumerate() {
        if len > k {
            buf[len - 1 - k] = d;
        }
    }
    buf[len] = 0;
}

/// `fmt_str` (`util/optimize.c:43-70`) — `SAMPLE.PRG` → the fixed 8+3 field
/// form `SAMPLE  PRG`, NUL-terminated, which is what the `________.___`
/// template of `CACURRNA`/`CACOPYNA` merges character by character.
pub(super) fn fmt_str(instr: &str) -> [u8; LEN_ZFNAME] {
    let mut out = [0u8; LEN_ZFNAME];
    let src = instr.as_bytes();
    let mut q = 0usize;
    let mut p = 0usize;
    // "copy up to 8 bytes before the (first) dot (we eat excess bytes)"
    while p < src.len() {
        let c = src[p];
        p += 1;
        if c == b'.' {
            break;
        }
        if q < 8 {
            out[q] = c;
            q += 1;
        }
    }
    // "if any extension present, fill out with spaces, then copy extension"
    if p < src.len() {
        while q < 8 {
            out[q] = b' ';
            q += 1;
        }
        while q < 11 && p < src.len() {
            out[q] = src[p];
            q += 1;
            p += 1;
        }
    }
    out[q] = 0;
    out
}

/// `unfmt_str` (`util/optimize.c:77-96`) — the reverse of [`fmt_str`]:
/// `SAMPLE  PRG` → `SAMPLE.PRG`.
pub(super) fn unfmt_str(instr: &[u8]) -> String {
    let end = instr.iter().position(|&c| c == 0).unwrap_or(instr.len());
    let s = &instr[..end];
    let mut out = String::new();
    let mut p = 0usize;
    while p < s.len() && p < 8 {
        let c = s[p];
        p += 1;
        if c != b' ' {
            out.push(c as char);
        }
    }
    if p < s.len() {
        out.push('.');
        while p < s.len() {
            out.push(s[p] as char);
            p += 1;
        }
    }
    out
}

/// `filename_start` (`util/optimize.c`) — everything after the last `\`.
fn filename_start(path: &str) -> &str {
    match path.rfind('\\') {
        Some(i) => &path[i + 1..],
        None => path,
    }
}

/// The directory half of a leaf path, **including** the trailing `\` — the
/// prefix `del_fname` leaves behind (`deskdir.c:214-217`).
fn dir_part(path: &str) -> &str {
    match path.rfind('\\') {
        Some(i) => &path[..=i],
        None => "",
    }
}

/// `item_exists` (`deskdir.c:186-208`) — does this file (`is_folder == false`)
/// or folder (`is_folder == true`) exist?
///
/// ```c
/// if (is_folder) { p = path + strlen(path); strcpy(p, "\\*.*"); }
/// rc = dos_sfirst(path, ALLFILES);
/// ```
///
/// The `\*.*` suffix is what makes the folder form *folder-only*: a plain
/// `Fsfirst` on `C:\FOO` also matches a **file** named `FOO`, while
/// `C:\FOO\*.*` only resolves when `FOO` is a directory — it finds that
/// directory's `.` entry (`ALLFILES` carries `FA_SUBDIR`, and `Dcreate` writes
/// `.`/`..` into every new sub-directory, `bdos/fs/dirops.rs`).
///
/// EmuTOS saves the caller's DTA around the probe because its walker has a live
/// `Fsfirst`/`Fsnext` search running; NewTOS's [`scan_folder`] collects a whole
/// directory and **drains** the search before returning, so there is never a
/// live search to disturb (that module note is why every scan re-issues
/// `dos_sdta`).
fn item_exists(path: &str, is_folder: bool) -> bool {
    let mut dta = [0u8; 44];
    dos_sdta(dta.as_mut_ptr());
    if is_folder {
        let mut probe = String::from(path);
        probe.push_str("\\*.*");
        dos_sfirst(&probe, ALLFILES) == E_OK
    } else {
        dos_sfirst(path, ALLFILES) == E_OK
    }
}

/// `illegal_op_msg` (`deskdir.c:220-224`) — the `STILLOP` alert
/// (`desk_rsc.c:2497`), and the `FALSE` it returns: the caller stops.
fn illegal_op_msg() -> More {
    fun_alert(1, b"Illegal operation!");
    More::Stop
}

/// `do_namecon` (`deskdir.c:93-130`) — put the `ADCPALER` name-conflict dialog
/// up over whatever is already on screen, run it, take it down again and
/// repaint the copy box underneath. Returns the object index of the button that
/// was pressed ([`CAOK`] / [`CASKIP`] / [`CASTOP`]).
///
/// `dst` is edited in place: it is both the pre-filled suggestion and the name
/// the user may have changed.
fn do_namecon(src: &[u8; LEN_ZFNAME], dst: &mut [u8; LEN_ZFNAME], ctx: &mut OpCtx) -> i16 {
    static TITLE: &[u8] = b"NAME CONFLICT DURING COPY\0";
    static CURR_TMPL: &[u8] = b"Current name: ________.___\0";
    static COPY_TMPL: &[u8] = b"Copy's name: ________.___\0";
    /// `te_pvalid` `"f"` — file-name characters (`desk_rs_tedinfo[22,23]`).
    static VALID: &[u8] = b"f\0";
    static OK: &[u8] = b" OK \0";
    static SKIP: &[u8] = b" Skip \0";
    static STOP: &[u8] = b" Stop \0";

    let ted = |text: *mut u8, tmpl: &[u8], tmplen: i16| TedInfo {
        te_ptext: text as u64,
        te_ptmplt: tmpl.as_ptr() as u64,
        te_pvalid: VALID.as_ptr() as u64,
        te_font: 3,
        te_junk1: 0,
        te_just: 0,
        te_color: TED_COLOR,
        te_junk2: 0,
        te_thickness: 1,
        te_txtlen: LEN_ZFNAME as i16 - 1,
        te_tmplen: tmplen,
    };
    let mut src_copy = *src;
    let mut curr_ted = ted(src_copy.as_mut_ptr(), CURR_TMPL, 27);
    let mut copy_ted = ted(dst.as_mut_ptr(), COPY_TMPL, 26);

    // `ADCPALER` is 38 characters wide with its fields at column 2 and its
    // **three** buttons on the bottom row (`desk_rsc.c:1239-1279`): 10
    // characters each at columns 2, 14 and 26, i.e. 80 px wide with a 16 px
    // gap. The box is therefore sized by the button row, not by the fields —
    // the first cut of this dialog took the two-button box's 90 px width and
    // centred the middle button, which made all three overlap by 23 px.
    const MARGIN: i16 = 24;
    const GAP: i16 = 8;
    const FLD_H: i16 = 24;
    /// `desk_rsc.c:1267` — `CAOK` is 10 characters wide.
    const BTN_W: i16 = 10 * CELL_W;
    /// The 2-character gap `desk_rsc.c:1263-1279`'s columns leave between them.
    const BTN_GAP: i16 = 2 * CELL_W;
    const BTN_ROW_W: i16 = 3 * BTN_W + 2 * BTN_GAP;
    let fld_w = 27 * CELL_W + 8;
    let dw = MARGIN + if fld_w > BTN_ROW_W { fld_w } else { BTN_ROW_W } + MARGIN;
    let y_title = MARGIN;
    let y_curr = y_title + CELL_H + 12;
    let y_copy = y_curr + FLD_H + GAP;
    let y_btn = y_copy + FLD_H + 20;
    let dh = y_btn + 28 + MARGIN;
    let btn_w = BTN_W;
    // `align_title` (`deskmain.c:1731-1750`) centres object 1 in the root.
    let title_w = (TITLE.len() as i16 - 1) * CELL_W;

    #[rustfmt::skip]
    let mut tree = [
        mk_root(BOX_SPEC, dw, dh, CASTOP),
        mk(G_STRING, 0, TITLE.as_ptr() as u64, (dw - title_w) / 2, y_title, title_w, CELL_H, CACURRNA, -1, -1),
        // `CACURRNA` is **not** EDITABLE (`desk_rsc.c:1251-1255`).
        mk(G_FBOXTEXT, FL3DBAK, &mut curr_ted as *mut _ as u64, MARGIN, y_curr, fld_w, FLD_H, CACOPYNA, -1, -1),
        mk(G_FBOXTEXT, EDITABLE | FL3DBAK, &mut copy_ted as *mut _ as u64, MARGIN, y_copy, fld_w, FLD_H, CAOK, -1, -1),
        mk(G_BUTTON, SELECTABLE | DEFAULT | EXIT | FL3DACT, OK.as_ptr() as u64, MARGIN, y_btn, btn_w, 28, CASKIP, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | FL3DACT, SKIP.as_ptr() as u64, MARGIN + btn_w + BTN_GAP, y_btn, btn_w, 28, CASTOP, -1, -1),
        mk(G_BUTTON, SELECTABLE | EXIT | LASTOB | FL3DACT, STOP.as_ptr() as u64, MARGIN + 2 * (btn_w + BTN_GAP), y_btn, btn_w, 28, 0, -1, -1),
    ];
    let tree_ptr = tree.as_mut_ptr() as u64;

    // `if (ml_havebox) draw_dial(tree); else { start_dialog(tree); … }`
    // (`deskdir.c:99-105`): the copy box is already up and has already reserved
    // its area, so this dialog paints straight over it.
    if ctx.havebox {
        draw_dial(tree_ptr);
    } else {
        start_dialog(tree_ptr);
        ctx.havebox = true;
    }
    super::form_do(tree_ptr, 0);

    // "trigger a redraw for the copy alert dialog so that a redraw message is
    // issued for the copy alert area" (`deskdir.c:108-116`), then repaint the
    // copy/delete box that was underneath (`:118`).
    let t = super::form_center(tree_ptr);
    super::form_dial_finish(t);
    if ctx.tree != 0 {
        draw_dial(ctx.tree);
    }

    // `ob = inf_gindex(tree, CAOK, 3) + CAOK; (tree+ob)->ob_state = NORMAL;`
    // (`deskdir.c:121-122`). EmuTOS deselects the button because `ADCPALER` is
    // a resource tree it reuses on the next conflict; this one is built fresh
    // per call and dropped on return, so the reset has nothing to reset — the
    // rebuild already guarantees no button is pre-selected next time.
    inf_gindex(&tree, CAOK as usize, 3) + CAOK
}

/// `output_fname` (`deskdir.c:290-397`) — settle the destination name of one
/// file copy, asking the user when the copy would overwrite something.
///
/// Returns [`More::Ok`] when the copy may proceed (`pdst_file` then holds the
/// possibly-renamed destination), [`More::Skip`] for "leave this one alone" and
/// [`More::Stop`] for "abandon the whole operation".
///
/// The dialog is skipped entirely — as in EmuTOS — when the names differ and
/// either *Confirm overwrites* is off or the destination does not exist yet;
/// `Fcreate`'s truncation then does what it always did.
fn output_fname(psrc_file: &str, pdst_file: &mut String, ctx: &mut OpCtx) -> More {
    let mut ob = 0i16;
    loop {
        // "set flag if user is trying to overwrite a file with itself"
        let samefile = psrc_file == pdst_file.as_str();
        if !samefile {
            if ob == CAOK {
                break;
            }
            if !ctx.covwr {
                break;
            }
            if !item_exists(pdst_file, false) {
                break;
            }
        }

        let ml_fsrc = fmt_str(filename_start(psrc_file));
        let mut ml_fdst = fmt_str(filename_start(pdst_file));
        let old_dst = ml_fdst; // "remember old destination"

        ob = do_namecon(&ml_fsrc, &mut ml_fdst, ctx);
        if ob == CASTOP {
            return More::Stop;
        }
        if ob == CASKIP {
            return More::Skip;
        }

        // "user says ok, so update destination filename"
        let ml_fstr = unfmt_str(&ml_fdst);
        if !ml_fstr.is_empty() {
            let dir = String::from(dir_part(pdst_file));
            *pdst_file = join_leaf(&dir, &ml_fstr);
        }

        // "if destination has changed, pretend there was no OK, so the next
        // iteration of this loop can check everything again"
        if ml_fdst != old_dst {
            ob = 0;
            continue;
        }

        // "if the user was trying to overwrite a file with itself, treat it as
        // a skip"
        if samefile {
            return More::Skip;
        }
    }
    More::Ok
}

/// The four answers of [`get_new_name`] — EmuTOS's `2`/`1`/`0`/`-1`
/// (`deskdir.c:692-698`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NewName {
    /// `2` — OK, and the name has changed.
    Changed,
    /// `1` — OK, and the name has not changed.
    Unchanged,
    /// `0` — *Stop*.
    Stop,
    /// `-1` — *Skip*.
    Skip,
}

/// `get_new_name` (`deskdir.c:700-723`) — ask for a replacement **folder** name
/// and write it back into `dstpth`.
///
/// ```c
/// fmt_str(filename_start(dstpth), ml_fsrc);   /* extract current folder name */
/// strcpy(ml_fdst,ml_fsrc);                    /* pre-fill new folder name */
/// inf_sset(tree, CACURRNA, ml_fsrc);
/// inf_sset(tree, CACOPYNA, ml_fdst);
/// ob = do_namecon();
/// …
/// inf_sget(tree, CACOPYNA, ml_fdst);
/// unfmt_str(ml_fdst, str);
/// del_fname(dstpth); add_fname(dstpth, str);
/// return strcmp(ml_fdst,ml_fsrc) ? 2 : 1;
/// ```
///
/// It is the **same** `ADCPALER` dialog the file conflict uses
/// ([`output_fname`]) — EmuTOS reuses `desk_rs_trees[ADCPALER]` for both, so
/// the heading stays `NAME CONFLICT DURING COPY` and both fields carry the
/// *destination* name: `CACURRNA` the one that is in the way, `CACOPYNA` the
/// same name pre-filled for editing (unlike the file case, where `CACURRNA` is
/// the **source** name, `deskdir.c:304-307`).
fn get_new_name(dstpth: &mut String, ctx: &mut OpCtx) -> NewName {
    let ml_fsrc = fmt_str(filename_start(dstpth));
    let mut ml_fdst = ml_fsrc; // pre-fill new folder name

    let ob = do_namecon(&ml_fsrc, &mut ml_fdst, ctx);
    if ob == CASTOP {
        return NewName::Stop;
    }
    if ob == CASKIP {
        return NewName::Skip;
    }

    // "& update destination path (it may not have changed)"
    let str_ = unfmt_str(&ml_fdst);
    let dir = String::from(dir_part(dstpth));
    *dstpth = join_leaf(&dir, &str_);

    if ml_fdst != ml_fsrc {
        NewName::Changed
    } else {
        NewName::Unchanged
    }
}

/// `output_path` (`deskdir.c:734-779`) — settle the destination **folder** of
/// one folder copy/move, asking for a new name when a folder of that name is
/// already there.
///
/// ```c
/// while(1) {
///     if (op == OP_RENAME) { if (!item_exists(dstpth, TRUE)) break; }
///     else {
///         if (dos_mkdir(dstpth) == 0) break;      /* we created the folder */
///         if (!item_exists(dstpth, TRUE)) return illegal_op_msg();
///     }
///     ret = get_new_name(dstpth);
///     if (ret <= 0) return ret;
///     if ((op != OP_RENAME) && (ret == 1)) break;
/// }
/// ```
///
/// The asymmetry between the two branches is the whole point:
///
/// * **copy/move across drives** — the folder is created here, once, before the
///   walk descends. It already existing is not fatal: *OK* on an unchanged name
///   means "merge into it" and the loop breaks (`ret == 1`). A changed name is
///   re-checked from the top, so the user can be sent round again.
/// * **move via rename** (`Op::Rename`) — `Frename` cannot merge, so the
///   destination must **not** exist. An unchanged name therefore does *not*
///   break the loop: the user is asked again until a free name is entered, or
///   *Stop*/*Skip* ends it.
///
/// A `dos_mkdir` that fails while nothing is in the way (a full root directory,
/// an illegal name) is [`illegal_op_msg`] — the operation stops.
///
/// EmuTOS finishes with `strcat(dstpth, "\\*.*")`; NewTOS's walker takes a
/// directory path ending in `\`, which [`dir_op`] appends with [`as_dir`] at
/// the call site.
fn output_path(op: Op, dstpth: &mut String, ctx: &mut OpCtx) -> More {
    loop {
        if op == Op::Rename {
            // "for move via rename, the destination folder must not exist"
            if !item_exists(dstpth, true) {
                break;
            }
        } else {
            if dos_mkdir(dstpth) == E_OK {
                break; // ok, we created the new folder
            }
            // "either it already exists or there is insufficient space"
            if !item_exists(dstpth, true) {
                return illegal_op_msg();
            }
        }

        // "the destination folder exists: we try to get a new one"
        match get_new_name(dstpth, ctx) {
            NewName::Stop => return More::Stop,
            NewName::Skip => return More::Skip,
            // "if it's not move via rename, and the destination path has not
            // changed, we're done (we'll copy/move into the existing path)"
            NewName::Unchanged if op != Op::Rename => break,
            _ => continue, // loop to check the updated destination path
        }
    }
    More::Ok
}

/// The `ADCPYDEL` confirmation / progress box (`deskdir.c:911-965`) while an
/// operation runs. Holds the two count fields' text buffers and TEDINFOs
/// alongside the object array, so all of it lives in one [`dir_op`] frame.
struct CpyDelBox {
    tree: [Object; 6],
    /// `CDFOLDS`/`CDFILES` text buffers — `te_txtlen` is 6
    /// (`desk_rs_tedinfo[20,21]`: five digits plus the NUL).
    folds_buf: [u8; 6],
    files_buf: [u8; 6],
    folds_ted: TedInfo,
    files_ted: TedInfo,
}

/// `STCOPY` / `STMOVE` / `STDELETE` (`desk_rsc.c:2472-2474`) — the heading
/// `dir_op` writes into `CDTITLE` (`deskdir.c:907,940,944`).
const STCOPY: &[u8] = b"COPY FILE(S)\0";
const STMOVE: &[u8] = b"MOVE FILE(S)\0";
const STDELETE: &[u8] = b"DELETE FILE(S)\0";

impl CpyDelBox {
    /// Build the box for `op` with `count`'s totals already in its two fields.
    fn new(op: Op, count: &DirCount) -> alloc::boxed::Box<CpyDelBox> {
        static FOLDS_TMPL: &[u8] = b"Number of folders:         _____\0";
        static FILES_TMPL: &[u8] = b"Number of files:         _____\0";
        static EMPTY: &[u8] = b"\0";
        static OK: &[u8] = b" OK \0";
        static CANCEL: &[u8] = b" Cancel \0";

        const MARGIN: i16 = 24;
        const GAP: i16 = 8;
        const FLD_H: i16 = 24;
        let fld_w = 33 * CELL_W + 8; // the longer of the two templates
        let dw = MARGIN + fld_w + MARGIN;
        let y_title = MARGIN;
        let y_folds = y_title + CELL_H + 12;
        let y_files = y_folds + FLD_H + GAP;
        let y_btn = y_files + FLD_H + 20;
        let dh = y_btn + 28 + MARGIN;
        let btn_w = 90i16;

        let title: &[u8] = match op {
            Op::Delete => STDELETE,
            Op::Copy => STCOPY,
            // "if (op != OP_COPY) … obj->ob_spec = STMOVE" (`deskdir.c:941-945`)
            _ => STMOVE,
        };
        let title_w = (title.len() as i16 - 1) * CELL_W;

        // `Box`ed before the TEDINFOs are pointed at the buffers: the object
        // array holds the TEDINFO addresses and the TEDINFOs hold the buffer
        // addresses, so nothing may move afterwards.
        let mut b = alloc::boxed::Box::new(CpyDelBox {
            tree: [mk_root(BOX_SPEC, dw, dh, CDOK + 1); 6],
            folds_buf: [0; 6],
            files_buf: [0; 6],
            folds_ted: TedInfo {
                te_ptext: 0,
                te_ptmplt: FOLDS_TMPL.as_ptr() as u64,
                te_pvalid: EMPTY.as_ptr() as u64,
                te_font: 3,
                te_junk1: 0,
                te_just: TE_RIGHT,
                te_color: TED_COLOR,
                te_junk2: 0,
                te_thickness: 1,
                te_txtlen: 6,
                te_tmplen: 33,
            },
            files_ted: TedInfo {
                te_ptext: 0,
                te_ptmplt: FILES_TMPL.as_ptr() as u64,
                te_pvalid: EMPTY.as_ptr() as u64,
                te_font: 3,
                te_junk1: 0,
                te_just: TE_RIGHT,
                te_color: TED_COLOR,
                te_junk2: 0,
                te_thickness: 1,
                te_txtlen: 6,
                te_tmplen: 31,
            },
        });
        b.folds_ted.te_ptext = b.folds_buf.as_ptr() as u64;
        b.files_ted.te_ptext = b.files_buf.as_ptr() as u64;
        let folds_ted = &b.folds_ted as *const TedInfo as u64;
        let files_ted = &b.files_ted as *const TedInfo as u64;

        b.tree = [
            mk_root(BOX_SPEC, dw, dh, CDOK + 1),
            // `align_title` centres the heading (`deskmain.c:1744-1747`).
            mk(G_STRING, 0, title.as_ptr() as u64, (dw - title_w) / 2, y_title, title_w, CELL_H, CDFOLDS, -1, -1),
            mk(G_FBOXTEXT, FL3DBAK, folds_ted, MARGIN, y_folds, fld_w, FLD_H, CDFILES, -1, -1),
            mk(G_FBOXTEXT, FL3DBAK, files_ted, MARGIN, y_files, fld_w, FLD_H, CDOK, -1, -1),
            mk(G_BUTTON, SELECTABLE | DEFAULT | EXIT | FL3DACT, OK.as_ptr() as u64, MARGIN, y_btn, btn_w, 28, CDOK + 1, -1, -1),
            mk(G_BUTTON, SELECTABLE | EXIT | LASTOB | FL3DACT, CANCEL.as_ptr() as u64, dw - MARGIN - btn_w, y_btn, btn_w, 28, 0, -1, -1),
        ];

        let ptr = b.tree.as_ptr() as u64;
        inf_numset(ptr, CDFILES, count.files);
        inf_numset(ptr, CDFOLDS, count.dirs);
        b
    }

    /// The object array's address, as every `tree`-taking helper wants it.
    fn ptr(&self) -> u64 {
        self.tree.as_ptr() as u64
    }
}

// ── per-item engine primitives ──────────────────────────────────────────────

/// `d_dofdel` — delete one file (EmuTOS `deskdir.c:237`). `true` on success.
fn d_dofdel(path: &str) -> bool {
    dos_delete(path) == E_OK
}

/// `d_dofoldel` — remove one now-empty folder (EmuTOS `deskdir.c:269`).
fn d_dofoldel(path: &str) -> bool {
    dos_rmdir(path) == E_OK
}

/// `d_dofileren`/`d_dofoldren` — rename one file/folder (EmuTOS
/// `deskdir.c:785`/`:831`). Same-drive move via `Frename`.
fn d_dofileren(old: &str, new: &str) -> bool {
    dos_rename(old, new) == E_OK
}

/// `d_dofcopy` — copy one file (EmuTOS `deskdir.c:389-519`): open the source,
/// settle the destination name with [`output_fname`] (which is where the
/// overwrite question lives), create the destination, stream through `buf`,
/// then stamp it with the source's date/time.
///
/// `dst` is `&mut` because `output_fname` may rename it: the user can type a
/// different name into the name-conflict dialog, exactly as EmuTOS edits
/// `pdst_file` in place (`deskdir.c:354-360`).
fn d_dofcopy(
    src: &str,
    dst: &mut String,
    time: u16,
    date: u16,
    attr: u16,
    buf: &mut [u8],
    ctx: &mut OpCtx,
) -> More {
    let srcfh = dos_open(src, 0);
    if srcfh < 0 {
        return More::Failed;
    }
    let srcfh = srcfh as i16;

    // `rc = output_fname(psrc_file, pdst_file);` (`deskdir.c:417-424`) — after
    // the source is open and before the destination is created, so a Stop or
    // Skip leaves nothing behind.
    match output_fname(src, dst, ctx) {
        More::Ok => {}
        other => {
            dos_close(srcfh);
            return other;
        }
    }

    // Mask off the sub-directory bit — a file copy never carries FA_SUBDIR.
    let dstfh = dos_create(dst, attr & !(FA_SUBDIR as u16));
    if dstfh < 0 {
        dos_close(srcfh);
        return More::Failed;
    }
    let dstfh = dstfh as i16;

    let mut ok = true;
    loop {
        let n = dos_read(srcfh, buf.len() as i32, buf.as_mut_ptr());
        if n == 0 {
            dos_setdt(dstfh, time, date); // stamp target from source (deskdir.c:428)
            break;
        }
        if n < 0 {
            ok = false;
            break;
        }
        let w = dos_write(dstfh, n, buf.as_ptr());
        if w != n {
            ok = false; // short write = disk full (deskdir.c:467)
            break;
        }
    }

    dos_close(srcfh);
    dos_close(dstfh);
    if !ok {
        dos_delete(dst); // remove the partial destination (deskdir.c:497)
        return More::Failed;
    }
    More::Ok
}

// ── the recursive subtree walker ────────────────────────────────────────────

/// `d_doop` — process the entire contents of one folder recursively (EmuTOS
/// `deskdir.c:528`). `src_dir`/`dst_dir` end with `\`. Returns `true` if the
/// whole subtree completed without error (and without hitting [`MAX_LEVEL`]).
///
/// For `Copy`/`Move` a matching destination folder is created (merged if it
/// already exists); files are copied. For `Delete`/`Move` the folder is removed
/// once emptied (post-order, like EmuTOS). `Count` tallies files pre-order and
/// folders post-order (one `dirs++` per folder visited).
fn d_doop(
    level: i16,
    op: Op,
    src_dir: &str,
    dst_dir: &str,
    count: &mut DirCount,
    buf: &mut [u8],
    ctx: &mut OpCtx,
) -> bool {
    if level > MAX_LEVEL {
        return false; // STFO8DEE guard (deskdir.c:537)
    }

    let entries = scan_folder(src_dir);
    let mut ok = true;

    for e in &entries {
        if ctx.stop {
            break; // `for (… ; pf && more ; …)` — the user pressed Stop
        }
        let name = e.name_str();
        if e.is_dir() {
            let src_sub = join_leaf(src_dir, name);
            let src_sub_dir = as_dir(&src_sub);
            let dst_sub_dir = if matches!(op, Op::Copy | Op::Move) {
                let dst_sub = join_leaf(dst_dir, name);
                let _ = dos_mkdir(&dst_sub); // create or merge into existing
                as_dir(&dst_sub)
            } else {
                String::new()
            };
            if !d_doop(level + 1, op, &src_sub_dir, &dst_sub_dir, count, buf, ctx) {
                ok = false;
            }
            continue; // the folder branch does not count a *file* down
        }
        let src_file = join_leaf(src_dir, name);
        match op {
            Op::Count => {
                count.files += 1;
                count.size += e.size as u64;
            }
            Op::Delete => {
                if !d_dofdel(&src_file) {
                    ok = false;
                }
            }
            Op::Copy | Op::Move => {
                let mut dst_file = join_leaf(dst_dir, name);
                match d_dofcopy(&src_file, &mut dst_file, e.time, e.date, e.attr as u16, buf, ctx) {
                    // "if moving, delete original only if copy was ok"
                    // (`deskdir.c:667-669`).
                    More::Ok => {
                        if op == Op::Move && !d_dofdel(&src_file) {
                            ok = false;
                        }
                    }
                    More::Skip => {}
                    More::Failed => ok = false,
                    More::Stop => ctx.stop = true,
                }
            }
            Op::Rename => {} // rename never recurses (handled at dir_op level)
        }
        // `inf_numset(tree, CDFILES, --(count->files)); draw_fld(…)`
        // (`deskdir.c:674-678`) — one file down, box updated.
        ctx.file_done();
    }

    // End-of-folder handling (EmuTOS `deskdir.c:559-585`).
    match op {
        Op::Count => count.dirs += 1,
        // `&=` (not `&&`) so the folder is always removed, then the result
        // ANDed into `ok` — never short-circuit past the rmdir.
        Op::Delete | Op::Move => ok &= d_dofoldel(strip_dir(src_dir)),
        _ => {}
    }
    // `inf_numset(tree, CDFOLDS, --(count->dirs))` at end-of-folder
    // (`deskdir.c:590-594`), for every `op` that has a box.
    ctx.dir_done();
    ok
}

// ── the top-level dispatch over the selected items ──────────────────────────

/// `dir_op` — perform `op` on the [`SelEntry`] set selected in `src_dir`
/// (EmuTOS `deskdir.c:871-1084`). `src_dir`/`dst_dir` end with `\`. `dst_dir`
/// is `None` for `Delete`/`Count`. Returns `true` if every item completed.
///
/// `icontype` is EmuTOS's second parameter: the [`AType`] of the **desktop
/// icon** the selection came from, or `None` for EmuTOS's `-1` — "the source is
/// a window, not a desktop icon" (`deskfun.c:985,1113,1421`). Its only use is
/// the whole-drive question below, exactly as in EmuTOS.
///
/// The move→rename optimisation (`deskdir.c:885`): a `Move` whose source and
/// destination are on the **same drive letter** is silently a `Rename`
/// (`Frename`), never a copy+delete.
///
/// ## The confirmation box
///
/// Every operation but `Count` puts the `ADCPYDEL` box up (`deskdir.c:911-918`
/// decides that by `op != OP_COUNT`, which is exactly what its later
/// `if (tree)` tests mean), pre-filled with the file and folder counts of the
/// preceding `Count` pass. Whether it then **waits** for OK is the user's
/// preference (`:906,939-945`):
///
/// | operation      | `confirm` is                        |
/// |----------------|-------------------------------------|
/// | `Delete`       | `g_cdelepref`                       |
/// | `Copy`         | `g_ccopypref`                       |
/// | `Move`/`Rename`| `g_ccopypref \| g_cdelepref`        |
///
/// With the preference off the box still appears — as a progress display,
/// counting the two numbers down as the walk proceeds — and the operation
/// starts immediately. Cancel ends it before a single file is touched.
///
/// ## The whole-drive question
///
/// ```c
/// if (more && (op == OP_DELETE) && (icontype == AT_ISDISK))
///     more = (fun_alert_merge(2, STDELDIS, psrc_path[0]) == 1) ? TRUE: FALSE;
/// ```
/// (`deskdir.c:967-976`) — a **second**, unconditional question after the
/// `ADCPYDEL` box, asked only when the delete was started from a desktop disk
/// icon, i.e. when the whole drive is about to be emptied. It is not gated on
/// `g_cdelepref`: switching *Confirm deletes* off removes the counting box, not
/// this one.
pub fn dir_op(
    op: Op,
    icontype: Option<AType>,
    src_dir: &str,
    entries: &[SelEntry],
    dst_dir: Option<&str>,
    count: &mut DirCount,
    buf: &mut [u8],
) -> bool {
    let mut op = op;
    if op == Op::Move {
        if let Some(dst) = dst_dir {
            if src_dir.as_bytes().first() == dst.as_bytes().first() {
                op = Op::Rename;
            }
        }
    }
    if op == Op::Count {
        *count = DirCount::default();
    }

    // `confirm` and the box (`deskdir.c:899-965`).
    let (confdel, confcpy, covwr) =
        global::with(|g| (g.g_cdelepref, g.g_ccopypref, g.g_covwrpref)).unwrap_or((true, true, true));
    let confirm = match op {
        Op::Count => false,
        Op::Delete => confdel,
        Op::Copy => confcpy,
        // "if (op != OP_COPY) confirm |= G.g_cdelepref;" (`:941-943`)
        Op::Move | Op::Rename => confcpy || confdel,
    };

    let mut ctx = OpCtx { covwr, files: count.files, dirs: count.dirs, ..OpCtx::quiet() };
    let mut dialog = (op != Op::Count).then(|| CpyDelBox::new(op, count));
    if let Some(b) = dialog.as_mut() {
        ctx.tree = b.ptr();
        start_dialog(ctx.tree);
        ctx.havebox = true;
        if confirm {
            super::form_do(ctx.tree, 0);
            // `more = (tree[CDOK].ob_state & SELECTED) ? TRUE : FALSE;` (`:963`)
            ctx.stop = b.tree[CDOK as usize].ob_state & OB_SELECTED == 0;
        }
    }

    // "if user says OK, but we're deleting a whole disk, we always get an
    // additional special prompt" (`deskdir.c:967-976`). `psrc_path[0]` is the
    // drive letter, merged into `STDELDIS`.
    if !ctx.stop && op == Op::Delete && icontype == Some(AType::Disk) {
        let letter = src_dir.as_bytes().first().copied().unwrap_or(b'?');
        if !delete_disk_alert(letter) {
            ctx.stop = true;
        }
    }

    let mut ok = true;
    for e in entries {
        if ctx.stop {
            break;
        }
        let name = e.name_str();
        if e.is_dir {
            let src_sub = join_leaf(src_dir, name); // C:\SUB\CHILD
            match op {
                Op::Count => {
                    if !d_doop(0, Op::Count, &as_dir(&src_sub), "", count, buf, &mut ctx) {
                        ok = false;
                    }
                }
                Op::Delete => {
                    if !d_doop(0, Op::Delete, &as_dir(&src_sub), "", count, buf, &mut ctx) {
                        ok = false;
                    }
                }
                Op::Copy | Op::Move | Op::Rename => {
                    let dst = dst_dir.unwrap_or("");
                    let mut dst_sub = join_leaf(dst, name);
                    if source_is_parent(&src_sub, &dst_sub) {
                        ok = false; // STILLDIR — skip (deskdir.c:999)
                        continue;
                    }
                    // `more = output_path(op,srcpth,dstpth);` (`deskdir.c:1002`)
                    // — creates the destination folder, or asks for another
                    // name when one of that name is already there. It runs
                    // **before** the copy-to-self test, as in EmuTOS: dragging
                    // a folder onto its own parent is a conflict like any
                    // other and is asked about first (`:1005-1012`).
                    match output_path(op, &mut dst_sub, &mut ctx) {
                        More::Ok => {}
                        More::Skip => continue, // `more == -1`: skip this folder
                        More::Stop => {
                            ctx.stop = true; // `if (!more) break;`
                            continue;
                        }
                        More::Failed => {
                            ok = false;
                            continue;
                        }
                    }
                    if src_sub == dst_sub {
                        continue; // copy/move to self — no-op
                    }
                    let more = if op == Op::Rename {
                        // `d_dofoldren(srcpth,dstpth)` (`deskdir.c:1015`), which
                        // strips the trailing wildcards and renames (`:829-841`).
                        d_dofileren(&src_sub, &dst_sub)
                    } else {
                        d_doop(0, op, &as_dir(&src_sub), &as_dir(&dst_sub), count, buf, &mut ctx)
                    };
                    if !more {
                        ok = false;
                    }
                }
            }
            // A folder never counts a *file* down: EmuTOS's folder branch
            // `continue`s before the `CDFILES` update (`deskdir.c:1019`).
            continue;
        }
        let src_file = join_leaf(src_dir, name);
        match op {
            Op::Count => {
                count.files += 1;
                count.size += e.size as u64;
            }
            Op::Delete => {
                if !d_dofdel(&src_file) {
                    ok = false;
                }
            }
            Op::Copy | Op::Move | Op::Rename => {
                let dst = dst_dir.unwrap_or("");
                let mut dst_file = join_leaf(dst, name);
                if op == Op::Rename {
                    if !d_dofileren(&src_file, &dst_file) {
                        ok = false;
                    }
                } else {
                    match d_dofcopy(&src_file, &mut dst_file, e.time, e.date, e.attr(), buf, &mut ctx) {
                        More::Ok => {
                            if op == Op::Move && !d_dofdel(&src_file) {
                                ok = false;
                            }
                        }
                        More::Skip => {}
                        More::Failed => ok = false,
                        More::Stop => ctx.stop = true,
                    }
                }
            }
        }
        ctx.file_done();
    }

    // `tree[CDOK].ob_state = NORMAL; tree[CDCNCL].ob_state = NORMAL;
    //  end_dialog(tree);` (`deskdir.c:1075-1080`).
    if let Some(b) = dialog.as_mut() {
        b.tree[CDOK as usize].ob_state = 0;
        b.tree[CDOK as usize + 1].ob_state = 0;
        end_dialog(b.ptr());
    }
    ok
}

impl SelEntry {
    /// The FAT attribute a file copy should carry (never `FA_SUBDIR`).
    fn attr(&self) -> u16 {
        if self.is_dir {
            FA_SUBDIR as u16
        } else {
            0
        }
    }
}

// ── fun_op / fun_del / fun_selectall (deskfun.c action layer) ───────────────

/// `fun_op` — the single choke point for Copy/Move/Delete from any caller
/// (EmuTOS `deskfun.c:782`). Runs an `OP_COUNT` pre-pass; a zero-item selection
/// is a silent no-op (`false`). Otherwise runs the real operation and returns
/// `true` (the operation was *attempted* — matching EmuTOS, per-item failures
/// are not reflected in the return value).
///
/// `icontype` is EmuTOS's `icontype_src`, handed straight to both [`dir_op`]
/// passes (`deskfun.c:793,798`): the [`AType`] of the desktop icon the
/// selection came from, or `None` for a window source (EmuTOS's `-1`).
pub fn fun_op(
    op: Op,
    icontype: Option<AType>,
    src_dir: &str,
    entries: &[SelEntry],
    dst_dir: Option<&str>,
) -> bool {
    let mut count = DirCount::default();
    let mut empty: [u8; 0] = [];
    if !dir_op(Op::Count, icontype, src_dir, entries, None, &mut count, &mut empty) {
        fun_alert(1, b"Operation not allowed.");
        return false;
    }
    if count.files == 0 && count.dirs == 0 {
        return false; // nothing selected/matched — silent (deskfun.c:796)
    }

    let mut buf: Vec<u8> = Vec::new();
    if matches!(op, Op::Copy | Op::Move) {
        buf.resize(COPY_BUF, 0);
    }
    let all_ok = dir_op(op, icontype, src_dir, entries, dst_dir, &mut count, &mut buf);
    if !all_ok {
        fun_alert(1, b"Some items could not be processed.");
    }
    true
}

/// `fun_del` — the Delete menu item / Delete-key handler (EmuTOS
/// `deskfun.c:1376`), window case: delete the current window's selection after
/// a confirmation alert. The desktop-icon case (whole-drive delete + the
/// Remove→`MN_SELECTED` redirect) is [`fun_del_desktop`].
///
/// Returns `true` if the delete ran (so the caller repaints/rebuilds).
pub fn fun_del() -> bool {
    let cwin = global::with(|g| g.g_cwin).unwrap_or(DESKWH);
    if cwin == DESKWH {
        return fun_del_desktop();
    }
    let Some(src_dir) = window_dir(cwin) else {
        return false;
    };
    let entries = window_selected(cwin);
    if entries.is_empty() {
        return false;
    }
    // No question here: EmuTOS's window-case `fun_del` goes straight into
    // `fun_op` (`deskfun.c:1419-1423`) and the confirmation is `dir_op`'s
    // `ADCPYDEL` box, gated on *Confirm deletes* (`deskdir.c:906`). DESK13
    // moved the question there; the stand-in alert that used to sit here is
    // gone, so a delete is confirmed exactly once.
    // `fun_op(OP_DELETE, -1, &pw->w_pnode, NULL)` (`deskfun.c:1421`): the
    // source is a window, so no `icontype` and no `STDELDIS`.
    let done = fun_op(Op::Delete, None, &src_dir, &entries, None);
    if done {
        rebuild_window(cwin);
    }
    done
}

/// `fun_del`, desktop case (EmuTOS `deskfun.c:1391-1420`): each selected disk
/// icon dropped on Delete would wipe that whole drive — so it goes through
/// [`wants_to_delete_files`], whose "Remove" answer redirects (via a synthetic
/// `MN_SELECTED`) to the Options→Remove-desktop-icon handler instead of
/// touching the filesystem.
fn fun_del_desktop() -> bool {
    match wants_to_delete_files() {
        DeleteChoice::Cancel => false,
        DeleteChoice::Remove => {
            // Redirect to Options → Remove desktop icon (deskfun.c:1443). The
            // icon-removal handler is deskins (Task #14); the redirect itself is
            // implemented here so the wiring is faithful the moment it lands.
            super::main::fun_msg_mn_selected(
                super::menu::Title::Options,
                super::menu::Item::RemoveIcon,
            );
            false
        }
        DeleteChoice::Delete => {
            // Whole-disk / desktop-icon delete: gather each selected disk icon's
            // drive root and delete its full contents. `delete_ffd(path,
            // pa->a_type)` (`deskfun.c:1380,1365`) hands the icon's own type
            // down, so `dir_op` raises `STDELDIS` once per drive on top of the
            // `wants_to_delete_files` answer above — EmuTOS asks both.
            let targets = desktop_selected_disks();
            let mut any = false;
            for root in targets {
                let entries = root_entries(&root);
                if !entries.is_empty()
                    && fun_op(Op::Delete, Some(AType::Disk), &root, &entries, None)
                {
                    any = true;
                }
            }
            if any {
                super::redraw::redraw_desktop();
            }
            any
        }
    }
}

/// `wants_to_delete_files` — the three-choice delete alert (EmuTOS
/// `deskfun.c:1433`, `STRMVDEL`): Cancel / Remove (icon only) / Delete (files).
/// `form_alert` returns the 1-based button, left→right.
fn wants_to_delete_files() -> DeleteChoice {
    let text = b"[2][Remove the desktop icon,|or delete its files?][ Cancel | Remove | Delete ]\0";
    match super::form_alert(1, text.as_ptr()) {
        2 => DeleteChoice::Remove,
        3 => DeleteChoice::Delete,
        _ => DeleteChoice::Cancel,
    }
}

/// The answer to [`wants_to_delete_files`].
enum DeleteChoice {
    /// Do nothing.
    Cancel,
    /// Remove the desktop icon only (redirect to Options→Remove).
    Remove,
    /// Delete the underlying files.
    Delete,
}

/// Create directory `path` (`Dcreate`); `true` on success. The single-level
/// creation used by File → New folder ([`super::main`]).
pub fn mkdir(path: &str) -> bool {
    dos_mkdir(path) == E_OK
}

/// Rename/move `old` → `new` on the same drive (`Frename`); `true` on success.
/// The File → Rename action ([`super::main`]).
pub fn rename(old: &str, new: &str) -> bool {
    dos_rename(old, new) == E_OK
}

/// `fun_selectall` — select every item in the current window (EmuTOS
/// `deskfun.c:607`). Returns `true` if the current root is a real window (the
/// desktop has no "select all").
///
/// EmuTOS marks every `FNODE`/object `SELECTED` and sends a plain `WM_REDRAW`
/// — it does **not** re-scan the directory (a rescan would reset `f_selected`).
/// So this repaints the existing item objects (already flagged by
/// [`DeskState::select_all`]) rather than calling [`rebuild_window`].
pub fn fun_selectall() -> bool {
    let cwin = global::with(|g| g.g_cwin).unwrap_or(DESKWH);
    if cwin == DESKWH {
        return false;
    }
    global::with(|g| g.select_all(cwin));
    // `win_sinfo(pw, TRUE)` before the redraw (`deskfun.c:627-631`): the info
    // line now counts the whole selection.
    super::main::win_sinfo(cwin, true);
    super::redraw::redraw_desktop();
    true
}

/// `draw_dial` (EmuTOS `deskdir.c:79-85`) — redraw an already-started dialog in
/// place: re-centre it (`form_center`, which reports the same rectangle
/// `start_dialog` used) and `objc_draw` the whole tree into that rectangle. This
/// is the in-loop repaint a multi-step dialog does between two `form_do` turns;
/// it does **not** re-reserve or release the area, so the dialog stays up.
pub(super) fn draw_dial(tree: u64) {
    let t = super::form_center(tree);
    super::objc_draw_dialog(tree, t);
}

/// `draw_fld` (EmuTOS `deskdir.c:131-139`) — redraw **one field** of a dialog
/// that is already up: take the object's own rectangle, move it to absolute
/// screen coordinates with `objc_offset`, and `objc_draw` that subtree into it.
///
/// The cheap counterpart of [`draw_dial`]: the Install-icon dialog steps its
/// preview with it (`deskins.c:962`, `draw_fld(tree, ID_IBOX)`) so a click on an
/// arrow repaints the icon box and nothing else.
pub(super) fn draw_fld(tree: u64, obj: i16) {
    let (x, y) = super::objc_offset(tree, obj);
    // SAFETY: the caller owns `tree` for the duration of the dialog and `obj` is
    // one of its objects (a compile-time constant at every call site).
    let o = unsafe { &*(tree as *const tos_api::aes::object::Object).add(obj as usize) };
    let t = global::Grect { x, y, w: o.ob_width, h: o.ob_height };
    super::objc_draw_tree(tree, obj, t);
}

/// `fun_alert` — a one-line alert with a single OK button (EmuTOS
/// `deskfun.c:75`). `default` is the default button (1). Returns the clicked
/// button.
pub fn fun_alert(default: i16, msg: &[u8]) -> i16 {
    let mut text = Vec::with_capacity(msg.len() + 16);
    text.extend_from_slice(b"[1][");
    text.extend_from_slice(msg);
    text.extend_from_slice(b"][ OK ]\0");
    super::form_alert(default, text.as_ptr())
}

/// `fun_alert_merge(2, STDELDIS, psrc_path[0])` (`deskdir.c:974`) — the
/// whole-drive delete question, with the drive letter merged into the
/// `%c` of `STDELDIS` (`desk_rsc.c:2499-2500`):
///
/// ```text
/// [2][Do you want to delete all|the files on drive %c: ?][Yes| No ]
/// ```
///
/// `true` for *Yes*, i.e. EmuTOS's `== 1`. The default button is 2 (*No*).
/// `fun_alert_merge`'s `sprintf(G.g_work, …)` is the whole of the "merge";
/// with a single `%c` it is one byte substituted, which is what this builds.
fn delete_disk_alert(letter: u8) -> bool {
    let mut text = Vec::with_capacity(72);
    text.extend_from_slice(b"[2][Do you want to delete all|the files on drive ");
    text.push(letter);
    text.extend_from_slice(b": ?][Yes| No ]\0");
    super::form_alert(2, text.as_ptr()) == 1
}

/// A yes/no confirmation alert; `true` if the user chose the affirmative
/// (second) button.
fn fun_confirm(msg: &[u8]) -> bool {
    let mut text = Vec::with_capacity(msg.len() + 24);
    text.extend_from_slice(b"[2][");
    text.extend_from_slice(msg);
    text.extend_from_slice(b"][ Cancel | OK ]\0");
    super::form_alert(2, text.as_ptr()) == 2
}

// ── the drag-and-drop dispatch tree (deskfun.c fun_drag) ────────────────────

/// `fun_drag` — dispatch a completed drag gesture (EmuTOS `deskfun.c:1302`).
/// `wh` = source window handle ([`DESKWH`] for the desktop), `dest_wh` =
/// destination window handle (`0`/[`DESKWH`] if dropped on the desktop),
/// `dobj` = destination object index, `keystate` = modifiers at drop.
///
/// Returns `exit_desktop`. EmuTOS sets it when a dropped file launched a
/// program so that `deskmain` leaves and the AES shell loads the child;
/// NewTOS launches **nested** (AES gap G1, `super::main::launch_native`), so
/// the window-to-window branch — the one that can launch (DESK19) — hands back
/// `false` after the child has already run and the desktop is back.
pub fn fun_drag(wh: i16, dest_wh: i16, dobj: i16, keystate: u16) -> bool {
    if wh != DESKWH {
        // Dragging FROM a window.
        if dest_wh != DESKWH && dest_wh != NIL {
            return fun_win2win(wh, dest_wh, dobj, keystate);
        } else if dobj == DROOT {
            // Drop onto empty desktop: making a desktop shortcut is Task #14.
            fun_alert(1, b"Can't drop items here.");
        } else {
            fun_win2desk(wh, dobj, keystate);
        }
    } else {
        // Dragging FROM the desktop.
        if dest_wh != DESKWH && dest_wh != NIL {
            fun_desk2win(dest_wh, dobj, keystate);
        } else {
            fun_desk2desk(dobj, keystate);
        }
    }
    false
}

/// The Copy/Move operation a drop with `keystate` requests.
fn drag_op(keystate: u16) -> Op {
    if keystate & MODE_CTRL != 0 {
        Op::Move
    } else {
        Op::Copy
    }
}

/// `build_selected_path` (EmuTOS `deskfun.c:903-928`) — "builds the path
/// corresponding to the **first** selected file in the specified `PNODE`. the
/// path will be the filename only, or the full pathname, depending on the
/// desktop configuration settings."
///
/// ```c
/// for (fn = pn->p_flist; fn; fn = fn->f_next)
///     if (fnode_is_selected(fn)) break;
/// if (!fn) return FALSE;
/// if (G.g_fullpath) { strcpy(pathname, pn->p_spec); add_fname(pathname, fn->f_name); }
/// else              { strcpy(pathname, fn->f_name); }
/// ```
///
/// `entries` is the source window's selection in `p_flist` order
/// ([`window_selected`]), so its first element **is** EmuTOS's `fn`; `src_dir`
/// is `p_spec` with its mask already cut off ([`window_dir`]), which is what
/// `strcpy` + `add_fname` leave behind. [`None`] is EmuTOS's `FALSE` — "no file
/// is selected (probable program bug)".
///
/// Note what this does *not* do: only the first selection reaches the command
/// line, however many files were dragged. That is EmuTOS's own loop, and the
/// reason is the single `char destpath[MAXPATHLEN]` its caller passes.
fn build_selected_path(src_dir: &str, entries: &[SelEntry], fullpath: bool) -> Option<String> {
    let fn_name = entries.first()?.name_str();
    if fullpath {
        let mut s = String::from(src_dir);
        s.push_str(fn_name);
        Some(s)
    } else {
        Some(String::from(fn_name))
    }
}

/// `i_find(wh, item, &pf, NULL)`'s window arm (EmuTOS `desksupp.c:1753-1770`)
/// for the destination of a drop: the item's file name and the `ANODE` that
/// governs it, resolved by type and name through
/// [`super::inf::app_afind_by_name`] exactly as `win_icalc` resolves the
/// icon. [`None`] where EmuTOS's `pf`/`pa` would be `NULL` — a non-item
/// object, or a name no `ANODE` claims.
fn i_find_win(dst_wh: i16, dobj: i16) -> Option<(String, super::inf::Afind)> {
    if dobj < 0 {
        return None;
    }
    let (name, atype, spec) = global::with(|g| {
        let slot = g.win_slot(dst_wh)?;
        let fnode = g.g_screeninfo.get(dobj as usize)?.fnode;
        if fnode < 0 {
            return None;
        }
        let f = g.windows.slots[slot].w_pnode.p_flist.get(fnode as usize)?;
        // `(pf->f_attr&FA_SUBDIR) ? AT_ISFOLD : AT_ISFILE` (`desksupp.c:1766`).
        let atype = if f.is_dir() { AType::Folder } else { AType::File };
        // The same byte-by-byte widening `win_icalc` uses, so a high-bit name
        // produces the same key here as it does there.
        let name: String = f.name_bytes().iter().map(|&b| b as char).collect();
        Some((name, atype, g.windows.slots[slot].w_pnode.p_spec.clone()))
    })
    .flatten()?;
    let pa = super::inf::app_afind_by_name(atype, &spec, &name)?;
    Some((name, pa))
}

/// `fun_win2win` — copy/move between two windows, **or** launch the program
/// the selection was dropped on (EmuTOS `deskfun.c:942-1001`). Returns
/// `exit_desktop`.
fn fun_win2win(src_wh: i16, dst_wh: i16, dst_obj: i16, keystate: u16) -> bool {
    let Some(src_dir) = window_dir(src_wh) else {
        return false;
    };
    let entries = window_selected(src_wh);
    if entries.is_empty() {
        return false;
    }

    // `pda = i_find(dst_wh, dst_ob, &pdf, NULL);` (`deskfun.c:958`) and, when
    // it resolves, `if (pda->a_aicon >= 0)` — "dropping file on to an
    // application" (`:962`). The test is the icon number, not the file's
    // extension: whatever `ANODE` gives this name the **application** picture
    // is what makes the drop a launch. It is the same number `win_icalc` hands
    // the view, so the branch taken always matches the icon on screen.
    if let Some((dst_name, pda)) = i_find_win(dst_wh, dst_obj) {
        if pda.aicon >= 0 {
            let fullpath = global::with(|g| g.g_fullpath).unwrap_or(true);
            if let Some(destpath) = build_selected_path(&src_dir, &entries, fullpath) {
                // `exit_desktop = do_aopen(pda, TRUE, dst_ob,
                //  pdw->w_pnode.p_spec, pdf->f_name, destpath); return;`
                // (`:967-968`) — and nothing is copied or moved.
                let dst_dir = window_dir(dst_wh).unwrap_or_default();
                return super::main::do_aopen_drop(
                    pda.is_installed(),
                    &pda.pappl,
                    pda.appdir,
                    &dst_dir,
                    &dst_name,
                    &destpath,
                );
            }
        }
    }

    // Destination: into the dropped-on folder if it is one, else the window dir.
    let dst_dir = dest_folder_dir(dst_wh, dst_obj).unwrap_or_else(|| window_dir(dst_wh).unwrap_or_default());
    let op = drag_op(keystate);
    // `fun_op(op, -1, &psw->w_pnode, destpath)` (`deskfun.c:985`).
    if fun_op(op, None, &src_dir, &entries, Some(&dst_dir)) {
        if src_wh != dst_wh {
            super::main::desk_clear(src_wh);
        }
        if op == Op::Move {
            rebuild_window(src_wh);
        }
        rebuild_window(dst_wh);
        super::redraw::redraw_desktop();
    }
    false
}

/// `fun_win2desk` — drop a window selection onto a desktop icon (EmuTOS
/// `deskfun.c:1117`): resolve the destination desktop icon and route through
/// [`fun_file2desk`], rebuilding the source window on success.
fn fun_win2desk(src_wh: i16, dobj: i16, keystate: u16) {
    let Some(src_dir) = window_dir(src_wh) else {
        return;
    };
    let entries = window_selected(src_wh);
    if entries.is_empty() {
        return;
    }
    // `fun_file2desk(&wn_src->w_pnode, -1, an_dest, obj, keystate)`
    // (`deskfun.c:1130`) — a window source carries no icon type.
    if fun_file2desk(dobj, None, &src_dir, &entries, keystate) {
        rebuild_window(src_wh);
        super::redraw::redraw_desktop();
    }
}

/// `fun_file2desk` — drop a file set onto a desktop icon (EmuTOS
/// `deskfun.c:1004`): a Disk icon copies/moves to that drive's root, the Trash
/// deletes (after confirmation). File/Folder-shortcut and Printer targets are
/// Task #14. Returns `true` if a filesystem operation ran.
///
/// `icontype_src` is the type of the **source** icon — `None` for a window
/// selection — and is passed to `fun_op` unchanged (`deskfun.c:1081`), which is
/// how a whole drive dragged to the trash reaches `dir_op`'s `STDELDIS`.
fn fun_file2desk(
    dobj: i16,
    icontype_src: Option<AType>,
    src_dir: &str,
    entries: &[SelEntry],
    keystate: u16,
) -> bool {
    let (atype, letter) = global::with(|g| {
        let info = &g.g_screeninfo[dobj as usize];
        (info.atype, (info.block.ib_char & 0xFF) as u8)
    })
    .unwrap_or((AType::None, 0));

    match atype {
        AType::Disk => {
            if !letter.is_ascii_uppercase() {
                return false;
            }
            let mut dst = String::new();
            dst.push(letter as char);
            dst.push_str(":\\");
            fun_op(drag_op(keystate), icontype_src, src_dir, entries, Some(&dst))
        }
        AType::Trash => {
            // With a *window* selection (`icontype_src == None`) EmuTOS skips
            // `wants_to_delete_files` — that guard is for a dragged desktop
            // icon (`deskfun.c:1049-1051`) — and the only question is the
            // `ADCPYDEL` box `dir_op` puts up, gated on *Confirm deletes*
            // (`deskdir.c:906`).
            fun_op(Op::Delete, icontype_src, src_dir, entries, None)
        }
        // File/Folder shortcuts and the printer are Task #14 (deskins/print).
        _ => {
            fun_alert(1, b"Operation not allowed.");
            false
        }
    }
}

/// `fun_desk2win` — drag from the desktop to a window (EmuTOS `deskfun.c:1198`):
/// copy each selected disk icon's contents into the window. Trash/printer icons
/// and non-existent drives are skipped.
fn fun_desk2win(dest_wh: i16, dobj: i16, _keystate: u16) {
    let dst_dir = dest_folder_dir(dest_wh, dobj).unwrap_or_else(|| window_dir(dest_wh).unwrap_or_default());
    let disks = desktop_selected_disks();
    let mut any = false;
    for root in disks {
        let entries = root_entries(&root);
        // `fun_file2win` passes `-1`, not the source icon's type
        // (`deskfun.c:1113`) — it is a copy, so `STDELDIS` cannot apply anyway.
        if !entries.is_empty() && fun_op(Op::Copy, None, &root, &entries, Some(&dst_dir)) {
            any = true;
        }
    }
    if any {
        rebuild_window(dest_wh);
        super::redraw::redraw_desktop();
    }
}

/// `fun_desk2desk` — drag between two desktop icons (EmuTOS `deskfun.c:1260`):
/// dropping one disk's contents onto another disk (Copy) or onto the trash
/// (Delete). Self-drop is rejected.
fn fun_desk2desk(dobj: i16, keystate: u16) {
    let (dtype, dletter) = global::with(|g| {
        let info = &g.g_screeninfo[dobj as usize];
        (info.atype, (info.block.ib_char & 0xFF) as u8)
    })
    .unwrap_or((AType::None, 0));

    let sources = desktop_selected_disks_with_obj();
    let mut any = false;
    for (sobj, src_root) in sources {
        if sobj == dobj {
            continue; // self-drop
        }
        let entries = root_entries(&src_root);
        if entries.is_empty() {
            continue;
        }
        let ran = match dtype {
            AType::Disk if dletter.is_ascii_uppercase() => {
                let mut dst = String::new();
                dst.push(dletter as char);
                dst.push_str(":\\");
                fun_op(drag_op(keystate), Some(AType::Disk), &src_root, &entries, Some(&dst))
            }
            AType::Trash => {
                // **Not** gated on *Confirm deletes*: the source is a desktop
                // icon, i.e. a whole drive, and EmuTOS puts its
                // `wants_to_delete_files` guard in front of that case
                // unconditionally (`deskfun.c:1049-1051`) — the preference only
                // reaches the per-item question `d_doop` asks afterwards. This
                // alert stands in for that guard, so it stays unconditional.
                //
                // The icon type *is* handed on (`fun_file2any`:
                // `icontype = an_src->a_type`, `deskfun.c:1179-1180`), so
                // `dir_op` raises `STDELDIS` for the drive behind it, as EmuTOS
                // does after its own guard.
                if fun_confirm(b"Delete the selected item(s)?") {
                    fun_op(Op::Delete, Some(AType::Disk), &src_root, &entries, None)
                } else {
                    false
                }
            }
            _ => {
                fun_alert(1, b"Operation not allowed.");
                false
            }
        };
        any |= ran;
    }
    if any {
        super::redraw::redraw_desktop();
    }
}

// ── window rebuild bookkeeping (deskfun.c fun_rebld) ────────────────────────

/// `rebuild_window` — re-scan a window's directory, rebuild its item view and
/// repaint it (EmuTOS `deskfun.c:135-142` `rebuild_window`, reached from
/// `fun_rebld` `:173` and `fun_rebld_marked` `:150`). Called after a
/// filesystem-mutating operation touched a window's directory.
///
/// ```c
/// pn_active(&pwin->w_pnode, TRUE);
/// desk_verify(pwin->w_id, TRUE);
/// win_sinfo(pwin, FALSE);
/// wind_get_grect(pwin->w_id, WF_WXYWH, &gr);
/// fun_msg(WM_REDRAW, pwin->w_id, gr.g_x, gr.g_y, gr.g_w, gr.g_h);
/// ```
///
/// The `win_sinfo(pwin, **FALSE**)` is the point (DESK26): the re-scan throws
/// the old `FNODE`s away, so the selection the info line was counting is gone
/// — and `FALSE` is what puts the directory's own totals back in place of a
/// stale `" … in N selected items."`. It is the sixth `win_sinfo` call site,
/// and the second of the two that pass `FALSE` (the other being `do_diropen`,
/// `desksupp.c:450`).
pub fn rebuild_window(handle: i16) {
    if handle == DESKWH {
        return;
    }
    let work = super::wind_get_work(handle);
    let scanned = global::with(|g| {
        let sort = g.sort;
        let slot = g.win_slot(handle)?;
        let spec = g.windows.slots[slot].w_pnode.p_spec.clone();
        {
            let pn = &mut g.windows.slots[slot].w_pnode;
            pn.pn_open(&spec);
            pn.pn_active(true);
            pn.pn_sort(sort);
        }
        Some(slot)
    })
    .flatten();
    if let Some(slot) = scanned {
        if let Some(sliders) = global::with(|g| g.win_bldview(slot, work)) {
            super::main::set_sliders_pub(handle, sliders);
        }
    }
    // `win_sinfo(pwin, FALSE)` (`deskfun.c:141`), after the re-scan and before
    // the redraw.
    super::main::win_sinfo(handle, false);
    super::redraw::redraw_desktop();
}

/// `fun_rebld` — rebuild every open window whose directory equals `path`
/// (EmuTOS `deskfun.c:173`). Used after a drop that changed a folder's
/// contents in a window other than the source/destination.
pub fn fun_rebld(path: &str) {
    let handles = global::with(|g| {
        let mut v: Vec<i16> = Vec::new();
        for w in &g.windows.slots {
            if w.w_id != DESKWH && window_dir_of(&w.w_pnode.p_spec).as_deref() == Some(path) {
                v.push(w.w_id);
            }
        }
        v
    })
    .unwrap_or_default();
    for h in handles {
        rebuild_window(h);
    }
}

// ── desktop-state helpers ───────────────────────────────────────────────────

/// The directory (ending with `\`) of window `handle`'s `p_spec`.
fn window_dir(handle: i16) -> Option<String> {
    global::with(|g| {
        let slot = g.win_slot(handle)?;
        window_dir_of(&g.windows.slots[slot].w_pnode.p_spec)
    })
    .flatten()
}

/// The directory of a `p_spec` (`"C:\\SUB\\*.*" -> "C:\\SUB\\"`).
fn window_dir_of(spec: &str) -> Option<String> {
    let i = spec.rfind('\\')?;
    Some(String::from(&spec[..=i]))
}

/// The selected [`SelEntry`]s of window `handle`.
fn window_selected(handle: i16) -> Vec<SelEntry> {
    global::with(|g| {
        let Some(slot) = g.win_slot(handle) else {
            return Vec::new();
        };
        g.windows.slots[slot]
            .w_pnode
            .p_flist
            .iter()
            .filter(|f| f.f_selected)
            .map(SelEntry::from_fnode)
            .collect()
    })
    .unwrap_or_default()
}

/// If `dobj` in window `dst_wh` is a folder item, its directory (`dir + name +
/// \`); otherwise `None` (drop into the window's own directory).
fn dest_folder_dir(dst_wh: i16, dobj: i16) -> Option<String> {
    if dobj < 0 {
        return None;
    }
    global::with(|g| {
        let slot = g.win_slot(dst_wh)?;
        if g.g_screeninfo.get(dobj as usize)?.atype != AType::Folder {
            return None;
        }
        let fnode = g.g_screeninfo[dobj as usize].fnode;
        let f = g.windows.slots[slot].w_pnode.p_flist.get(fnode as usize)?;
        let name = core::str::from_utf8(f.name_bytes()).ok()?;
        let dir = window_dir_of(&g.windows.slots[slot].w_pnode.p_spec)?;
        let mut d = join_leaf(&dir, name);
        d.push('\\');
        Some(d)
    })
    .flatten()
}

/// The drive-root spec (`X:\`) of every selected desktop **disk** icon.
fn desktop_selected_disks() -> Vec<String> {
    desktop_selected_disks_with_obj().into_iter().map(|(_, s)| s).collect()
}

/// Like [`desktop_selected_disks`] but also returns each icon's object index.
fn desktop_selected_disks_with_obj() -> Vec<(i16, String)> {
    global::with(|g| {
        let mut v: Vec<(i16, String)> = Vec::new();
        let mut child = g.g_screen[DROOT as usize].ob_head;
        while child != NIL && child != DROOT {
            let sel = g.g_screen[child as usize].ob_state & OB_SELECTED != 0;
            let info = &g.g_screeninfo[child as usize];
            if sel && info.atype == AType::Disk {
                let letter = (info.block.ib_char & 0xFF) as u8;
                if letter.is_ascii_uppercase() {
                    let mut root = String::new();
                    root.push(letter as char);
                    root.push_str(":\\");
                    v.push((child, root));
                }
            }
            child = g.g_screen[child as usize].ob_next;
        }
        v
    })
    .unwrap_or_default()
}

/// Scan a drive-root directory (`X:\`) into a full selected-entry list (every
/// top-level entry — dragging a disk icon means "all its contents").
fn root_entries(root: &str) -> Vec<SelEntry> {
    scan_folder(root)
        .into_iter()
        .map(|e| SelEntry {
            name: e.name,
            is_dir: e.is_dir(),
            size: e.size,
            time: e.time,
            date: e.date,
        })
        .collect()
}

impl DeskState {
    /// `fun_selectall` core — set every window-item child of `handle`'s root
    /// selected (both the `OBJECT` bit and the backing `f_selected`).
    pub fn select_all(&mut self, handle: i16) {
        let Some(slot) = self.win_slot(handle) else {
            return;
        };
        let root = self.windows.slots[slot].w_root;
        let mut child = self.g_screen[root as usize].ob_head;
        while child != NIL && child != root {
            self.g_screen[child as usize].ob_state |= OB_SELECTED;
            child = self.g_screen[child as usize].ob_next;
        }
        for f in &mut self.windows.slots[slot].w_pnode.p_flist {
            f.f_selected = true;
        }
    }
}

// ── Task-#9 backend verification (deskdir.md "Verification criteria") ────────

/// Direct, programmatic verification of the file-operation backend against a
/// live drive (the `xtask run --board rpi4 --sd /tmp/sd.img` C: volume). Not
/// wired into the normal boot — invoked from `lib.rs` only when proving the
/// backend, exactly like the other `selftest_*` modules. Prints one line per
/// check to the boot console.
///
/// Exercises: `Dcreate` (new folder), `Fcreate`+`Fwrite` (make a file), a
/// **copy** (`d_dofcopy`), a **rename** (`d_dofileren`, same-drive move), and a
/// recursive **delete** (`d_doop` empties + removes a folder), each confirmed by
/// a fresh directory rescan.
pub fn selftest() {
    use crate::kprintln;

    kprintln!("[desk-dir] backend self-test on C:\\ (mutates the SD image)");

    let base = "C:\\DESKT9\\"; // work directory, created + removed by the test
    let mut pass = 0u32;
    let mut fail = 0u32;
    let mut check = |name: &str, cond: bool| {
        if cond {
            pass += 1;
            kprintln!("[desk-dir]  PASS {}", name);
        } else {
            fail += 1;
            kprintln!("[desk-dir]  FAIL {}", name);
        }
    };

    // Clean any prior run.
    let _ = dos_delete("C:\\DESKT9\\A.TXT");
    let _ = dos_delete("C:\\DESKT9\\B.TXT");
    let _ = dos_delete("C:\\DESKT9\\SUB\\A.TXT");
    let _ = dos_rmdir("C:\\DESKT9\\SUB");
    let _ = dos_rmdir("C:\\DESKT9");

    // 1. Dcreate — new folder appears in a root rescan.
    check("Dcreate C:\\DESKT9", dos_mkdir("C:\\DESKT9") == E_OK);
    check("  rescan sees DESKT9", scan_folder("C:\\").iter().any(|e| e.name_str() == "DESKT9" && e.is_dir()));

    // 2. Fcreate + Fwrite — a file appears with the written length.
    let body = b"NewTOS desk file-op backend test.\r\n";
    let fh = dos_create("C:\\DESKT9\\A.TXT", 0);
    let mut wrote = false;
    if fh >= 0 {
        wrote = dos_write(fh as i16, body.len() as i32, body.as_ptr()) == body.len() as i32;
        dos_close(fh as i16);
    }
    check("Fcreate+Fwrite A.TXT", wrote);
    check(
        "  rescan sees A.TXT sized",
        scan_folder(base).iter().any(|e| e.name_str() == "A.TXT" && !e.is_dir() && e.size == body.len() as u32),
    );

    // 3. Copy (d_dofcopy) — B.TXT is a byte copy of A.TXT.
    let mut buf: Vec<u8> = alloc::vec![0u8; COPY_BUF];
    let mut ctx = OpCtx::quiet();
    let mut dst = base_join("B.TXT");
    let copied = d_dofcopy(base_join("A.TXT").as_str(), &mut dst, 0x2000, 0x5461, 0, &mut buf, &mut ctx);
    check("Copy A.TXT -> B.TXT", copied == More::Ok);
    check(
        "  rescan sees B.TXT same size",
        scan_folder(base).iter().any(|e| e.name_str() == "B.TXT" && e.size == body.len() as u32),
    );

    // 4. Rename (same-drive move) — A.TXT -> R.TXT.
    let renamed = d_dofileren(base_join("A.TXT").as_str(), base_join("R.TXT").as_str());
    check("Rename A.TXT -> R.TXT", renamed);
    check(
        "  rescan: R.TXT present, A.TXT gone",
        scan_folder(base).iter().any(|e| e.name_str() == "R.TXT")
            && !scan_folder(base).iter().any(|e| e.name_str() == "A.TXT"),
    );

    // 5. Recursive delete via dir_op(Delete) on a whole sub-folder.
    let _ = dos_mkdir("C:\\DESKT9\\SUB");
    let fh2 = dos_create("C:\\DESKT9\\SUB\\N.TXT", 0);
    if fh2 >= 0 {
        dos_write(fh2 as i16, body.len() as i32, body.as_ptr());
        dos_close(fh2 as i16);
    }
    let mut count = DirCount::default();
    let sub_entry = [SelEntry { name: fat_name(b"SUB"), is_dir: true, size: 0, time: 0, date: 0 }];
    let del_ok = dir_op(Op::Delete, None, base, &sub_entry, None, &mut count, &mut buf);
    check("dir_op(Delete) SUB subtree", del_ok);
    check("  rescan: SUB gone", !scan_folder(base).iter().any(|e| e.name_str() == "SUB"));

    // 6. Count pre-pass reports the two remaining files (B.TXT, R.TXT).
    let mut cnt = DirCount::default();
    let all = [
        SelEntry { name: fat_name(b"B.TXT"), is_dir: false, size: body.len() as u32, time: 0, date: 0 },
        SelEntry { name: fat_name(b"R.TXT"), is_dir: false, size: body.len() as u32, time: 0, date: 0 },
    ];
    dir_op(Op::Count, None, base, &all, None, &mut cnt, &mut buf);
    check("Count reports 2 files", cnt.files == 2);

    // Tidy up (leave the image close to its original state).
    let _ = dos_delete("C:\\DESKT9\\B.TXT");
    let _ = dos_delete("C:\\DESKT9\\R.TXT");
    let _ = dos_rmdir("C:\\DESKT9");
    check("cleanup: DESKT9 removed", !scan_folder("C:\\").iter().any(|e| e.name_str() == "DESKT9"));

    kprintln!("[desk-dir] backend self-test: {} passed, {} failed", pass, fail);
}

/// `C:\DESKT9\<leaf>` — a helper for [`selftest`].
fn base_join(leaf: &str) -> String {
    join_leaf("C:\\DESKT9\\", leaf)
}

/// A `NAME.EXT\0`-padded 14-byte FAT display name from a byte slice.
fn fat_name(s: &[u8]) -> [u8; 14] {
    let mut n = [0u8; 14];
    let k = s.len().min(13);
    n[..k].copy_from_slice(&s[..k]);
    n
}
