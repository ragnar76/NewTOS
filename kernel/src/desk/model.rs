//! `desk::model` — the desktop's file-list data model and per-object
//! auxiliary data.
//!
//! Reimplements EmuTOS `deskfpd.c` (`FNODE`/`PNODE`, `pn_open`/`pn_active`/
//! `pn_sort`) and the `SCREENINFO`/`ICONINFO` half of `deskbind.h`, per
//! `docs/api/desktop/_data-model.md`. None of these types cross an ABI
//! boundary (`_data-model.md`: "Contract: none"), so they are designed as
//! clean Rust rather than a byte-for-byte port:
//!
//! * `PNode::p_flist` is a growable [`Vec`] — there is **no** fixed 64-entry
//!   directory cap (fixes defect D9).
//! * The DTA fields are copied out **individually** (`fnode_from_dta`); the
//!   EmuTOS 23-byte `FNODE`/`DTA` `memcpy` overlap trick (`deskfpd.c:271`) is
//!   deliberately dropped, per `_rebuild-plan.md` §3.
//!
//! The directory scan itself goes through the GEMDOS trap
//! ([`super::dosbind`]) exactly like EmuTOS `pn_active()` drives
//! `dos_sfirst`/`dos_snext`.

use alloc::string::String;
use alloc::vec::Vec;

use tos_api::aes::object::IconBlk;
use tos_types::error::E_OK;

use super::dosbind::{dos_sdta, dos_sfirst, dos_snext};
use super::global::NIL;

/// FAT attribute bit marking a sub-directory (`bdosdefs.h` `FA_SUBDIR`).
pub const FA_SUBDIR: u8 = 0x10;

/// The attribute mask the desktop passes to `Fsfirst` (`deskbind.h`
/// `DISPATTR`): request sub-directories too (plain files always match).
pub const DISPATTR: u8 = FA_SUBDIR;

/// The directory-view mode (`deskfpd.h` `V_ICON`/`V_TEXT`). Global to the
/// whole desktop, not per-window (`_data-model.md` `g_iview`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum View {
    /// Icon grid (`V_ICON`, the boot default `START_VIEW`).
    #[default]
    Icon,
    /// Text rows (`V_TEXT`).
    Text,
}

/// The directory sort key (`deskfpd.h` `S_NAME`/`S_TYPE`/`S_SIZE`/`S_DATE`/
/// `S_NSRT`). Global to the whole desktop (`_data-model.md` `g_isort`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Sort {
    /// By name (`S_NAME`, the boot default `START_SORT`).
    #[default]
    Name,
    /// By type / extension (`S_TYPE`).
    Type,
    /// By size (`S_SIZE`).
    Size,
    /// By date+time (`S_DATE`).
    Date,
    /// Unsorted — keep the directory (scan) order (`S_NSRT`).
    NoSort,
}

// ── The `_IDT` cookie, and the one rule that resolves it ────────────────────
//
// `_IDT` carries the machine's date/time *display* preference: a separator
// character in the low byte, a field-order pair of bits, and a 12/24-hour bit
// (`include/biosdefs.h:161-173`). It is the value `DATEFORM_IDT`/`TIMEFORM_IDT`
// — the `Default` button of each format group in *Set preferences* — defer to.
//
// EmuTOS fetches it with `Supexec(get_idt_cookie)` (`desksupp.c:219`) because
// on m68k the cookie jar pointer lives at `$5A0`, in supervisor-only low
// memory. NewTOS has no such need and no `Supexec` (`docs/api/xbios/
// 26-supexec.md`, "decision deferred"; `docs/api/bios/_cookie-jar.md` §Status
// 2026-09-14, "There is no `Supexec` in the kernel"): the desktop reads the
// cookie **by value** through the GEMDOS trap, `Ssystem(S_GETCOOKIE, '_IDT',
// 0)` (`super::dosbind::dos_getcookie`), and keeps it in `G.g_idt`
// ([`super::global::DeskState::g_idt`]) exactly as EmuTOS does.
//
// The resolution rule below is the **only** copy of "what does IDT mean here":
// both readers — `desk::inf::fmt_date` and `desk::inf::fmt_time` — go through
// it. DESK02 had the rule written twice with contradicting answers (the dialog
// pre-selected one format, the text view printed another); it is written once
// now, and the Preferences dialog no longer needs it at all, because it offers
// EmuTOS's `Default` button and stores the raw value.

/// `IDT_TMASK` (`biosdefs.h:163`) — the clock bit of the `_IDT` cookie.
pub const IDT_TMASK: u32 = 0x1000;
/// `IDT_24H` (`biosdefs.h:162`) — [`IDT_TMASK`] set means a 24-hour clock.
pub const IDT_24H: u32 = 0x1000;
/// `IDT_BIT_DM` (`biosdefs.h:166`) — day before month.
pub const IDT_BIT_DM: u32 = 0x100;
/// `IDT_BIT_YM` (`biosdefs.h:167`) — year before month.
pub const IDT_BIT_YM: u32 = 0x200;
/// `IDT_DMASK` (`biosdefs.h:172`) — the two field-order bits.
pub const IDT_DMASK: u32 = IDT_BIT_YM | IDT_BIT_DM;
/// `IDT_SMASK` (`biosdefs.h:173`) — the separator character, low byte.
pub const IDT_SMASK: u32 = 0xFF;

/// `DATEFORM_SEP` (`deskbind.h:64`) — the separator EmuTOS uses whenever the
/// format does **not** come from `_IDT`. Both call sites of `fmt_date` pass it
/// (`deskinf.c:129`), which is why the text view and the Show-Info box print
/// the same character.
pub const DATEFORM_SEP: u8 = b'/';

/// One of the four field orders an `_IDT` date format can name
/// (`biosdefs.h:168-171`), i.e. the result of `fmt_date`'s second `switch`
/// (`deskinf.c:154-176`). Not a preference — [`DateFmt`] is the preference;
/// this is what it resolves to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DateOrder {
    /// `IDT_MMDDYY` — month, day, year.
    Mdy,
    /// `IDT_DDMMYY` — day, month, year (EmuTOS's `default:` arm).
    Dmy,
    /// `IDT_YYMMDD` — year, month, day.
    Ymd,
    /// `IDT_YYDDMM` — year, day, month ("does anyone ever use this?",
    /// `deskinf.c:166`).
    Ydm,
}

impl DateOrder {
    /// `switch(format)` (`deskinf.c:154-176`) over the two [`IDT_DMASK`] bits.
    fn from_idt(dmask: u32) -> DateOrder {
        match dmask {
            0 => DateOrder::Mdy,               // IDT_MMDDYY
            IDT_BIT_YM => DateOrder::Ymd,      // IDT_YYMMDD
            IDT_DMASK => DateOrder::Ydm,       // IDT_YYDDMM
            _ => DateOrder::Dmy,               // default: DDMMYY
        }
    }
}

/// The clock format the text view and the Show-Info box print times in
/// (EmuTOS `TIMEFORM_*`, `deskbind.h:68-70`, held in `G.g_ctimeform`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TimeFmt {
    /// `TIMEFORM_24H` (0) — `hh:mm`, 24-hour.
    #[default]
    H24,
    /// `TIMEFORM_12H` (1) — `hh:mm` with an `am`/`pm` suffix.
    H12,
    /// `TIMEFORM_IDT` (2) — take the clock from the `_IDT` cookie; the
    /// `Default` button of the *Time format* group. EmuTOS's own no-`.INF`
    /// default (`INF_E2_DEFAULT` carries `INF_E2_IDTTIME`, `deskapp.c:103`).
    Idt,
}

impl TimeFmt {
    /// The clock that is actually **rendered**, with [`TimeFmt::Idt`] resolved
    /// against the `_IDT` word `idt` — `fmt_time`'s `switch`
    /// (`deskinf.c:82-101`): `TIMEFORM_IDT` is 24-hour if `(idt & IDT_TMASK) ==
    /// IDT_24H`, and falls through to the 12-hour arm otherwise.
    pub fn rendered(self, idt: u32) -> TimeFmt {
        match self {
            TimeFmt::Idt => {
                if idt & IDT_TMASK == IDT_24H {
                    TimeFmt::H24
                } else {
                    TimeFmt::H12
                }
            }
            other => other,
        }
    }
}

/// The date order the text view and the Show-Info box print dates in (EmuTOS
/// `DATEFORM_*`, `deskbind.h:65-67`, held in `G.g_cdateform`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DateFmt {
    /// `DATEFORM_DMY` (0) — day before month.
    Dmy,
    /// `DATEFORM_MDY` (1) — month before day.
    #[default]
    Mdy,
    /// `DATEFORM_IDT` (2) — take the order *and the separator* from the `_IDT`
    /// cookie; the `Default` button of the *Date format* group, and EmuTOS's
    /// own no-`.INF` default (`INF_E2_DEFAULT`, `deskapp.c:103`).
    Idt,
}

impl DateFmt {
    /// The `(order, separator)` pair that is actually **rendered** —
    /// `fmt_date`'s first `switch` (`deskinf.c:138-152`) plus the `separator =
    /// DATEFORM_SEP` it starts from (`:129`).
    ///
    /// `DATEFORM_IDT` takes both from the cookie, and only overrides the
    /// separator when the cookie's low byte is non-zero (`:141-143`); every
    /// other value keeps [`DATEFORM_SEP`].
    pub fn rendered(self, idt: u32) -> (DateOrder, u8) {
        match self {
            DateFmt::Idt => {
                let sep = (idt & IDT_SMASK) as u8;
                (
                    DateOrder::from_idt(idt & IDT_DMASK),
                    if sep != 0 { sep } else { DATEFORM_SEP },
                )
            }
            DateFmt::Dmy => (DateOrder::Dmy, DATEFORM_SEP),
            DateFmt::Mdy => (DateOrder::Mdy, DATEFORM_SEP),
        }
    }
}

/// One matched directory entry (EmuTOS `FNODE`, `deskfpd.h:51-66`), carrying
/// the fields the desktop needs for display and sorting.
#[derive(Clone)]
pub struct FNode {
    /// `TRUE` if the user has selected this entry.
    pub f_selected: bool,
    /// FAT attribute byte (bit [`FA_SUBDIR`] = directory).
    pub f_attr: u8,
    /// Packed FAT time.
    pub f_time: u16,
    /// Packed FAT date.
    pub f_date: u16,
    /// File length in bytes (0 for a directory).
    pub f_size: u32,
    /// `NAME.EXT\0` display name (trim-padded, NUL-terminated).
    pub f_name: [u8; 14],
    /// Sequence within the directory (pre-sort scan order), used by
    /// [`Sort::NoSort`].
    pub f_seq: u16,
    /// Index into `DeskState::g_screen[]` while this file is on screen, or
    /// [`NIL`] when it is not currently displayed.
    pub f_obid: i16,
}

impl FNode {
    /// Whether this entry is a directory.
    pub fn is_dir(&self) -> bool {
        self.f_attr & FA_SUBDIR != 0
    }

    /// The display name as a byte slice up to (not including) the NUL.
    pub fn name_bytes(&self) -> &[u8] {
        let end = self.f_name.iter().position(|&c| c == 0).unwrap_or(self.f_name.len());
        &self.f_name[..end]
    }
}

/// One open directory's scan result (EmuTOS `PNODE`, `deskfpd.h:69-78`),
/// embedded in its owning [`super::win::WNode`].
#[derive(Default)]
pub struct PNode {
    /// Attribute mask passed to `Fsfirst` (always [`DISPATTR`]).
    pub p_attr: u8,
    /// The directory path + wildcard (e.g. `C:\*.*`). The scan always runs with
    /// the `*.*` tail so **all** entries are returned; the display filter is
    /// [`PNode::p_mask`], not this spec (see [`PNode::pn_active`]).
    pub p_spec: String,
    /// The File → Set-mask display filter (EmuTOS's `filename_start(p_spec)`,
    /// the per-window file mask). Empty or `*.*` means "show everything";
    /// otherwise a TOS 8.3 wildcard (e.g. `*.TXT`) applied to **files only** —
    /// folders always appear, matching EmuTOS `pn_active` (deskfpd.c:227,
    /// `del_fname(search)` + `wildcmp(match)` on files). Persists across a
    /// rescan/navigation (it is a window property, not reset by [`PNode::pn_open`]).
    pub p_mask: String,
    /// The matched entries, in current (sorted) order — dynamic, uncapped.
    pub p_flist: Vec<FNode>,
    /// Number of entries (`p_flist.len()`).
    pub p_count: usize,
    /// Sum of `f_size` over all file entries.
    pub p_size: u64,
}

impl PNode {
    /// `pn_open` — prepare this node for a directory path (EmuTOS
    /// `deskfpd.c:73-96`). Stores the search spec and the [`DISPATTR`] mask;
    /// does not scan (call [`PNode::pn_active`] for that). The display
    /// [`PNode::p_mask`] is **left untouched** — it is a window property that
    /// survives a rescan (so File → Set-mask's own rescan keeps the mask) and a
    /// same-window navigation (EmuTOS keeps the mask via `add_one_level`).
    pub fn pn_open(&mut self, spec: &str) {
        self.p_attr = DISPATTR;
        self.p_spec.clear();
        self.p_spec.push_str(spec);
        self.p_flist.clear();
        self.p_count = 0;
        self.p_size = 0;
    }

    /// `pn_active` — scan the directory into [`PNode::p_flist`] via GEMDOS
    /// `Fsfirst`/`Fsnext` (EmuTOS `deskfpd.c:227-298`). `include_folders`
    /// keeps sub-directories in the listing. `.` and `..` are skipped. An
    /// empty directory (or `EFILNF`/`ENMFIL`) yields an empty list, matching
    /// EmuTOS folding those errors to success. The [`PNode::p_mask`] display
    /// filter is applied to files only (folders always kept).
    pub fn pn_active(&mut self, include_folders: bool) {
        let (list, total) = scan_dir(&self.p_spec, self.p_attr, include_folders, &self.p_mask);
        self.p_count = list.len();
        self.p_size = total;
        self.p_flist = list;
    }

    /// `pn_sort` — reorder [`PNode::p_flist`] by `sort` (EmuTOS
    /// `deskfpd.c:pn_sort`). Directories sort ahead of files; [`Sort::NoSort`]
    /// restores the directory scan order (`f_seq`).
    pub fn pn_sort(&mut self, sort: Sort) {
        sort_flist(&mut self.p_flist, sort);
    }
}

/// The kind of icon an object represents, for the drag-and-drop dispatch tree
/// (EmuTOS `ANODE.a_type`, `deskapp.h` `AT_ISFILE`/`AT_ISFOLD`/`AT_ISDISK`/
/// `AT_ISTRSH`/`AT_ISPRNT`). A window item is a `File` or `Folder`; a desktop
/// icon is a `Disk`/`Trash`/`Printer`. Read by [`super::dir::fun_drag`] to
/// resolve what a drop onto this object means.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AType {
    /// Not a droppable target / unused object slot.
    #[default]
    None,
    /// A plain data file (`AT_ISFILE`).
    File,
    /// A directory / folder (`AT_ISFOLD`).
    Folder,
    /// A disk-drive icon (`AT_ISDISK`).
    Disk,
    /// The trash can (`AT_ISTRSH`).
    Trash,
    /// The printer (`AT_ISPRNT`).
    Printer,
}

/// The label buffer length carried per displayed object. Sized to EmuTOS's
/// `LEN_FNODE` (48, `deskwin.h:36`) so the **same** per-object buffer holds
/// either an icon label (`NAME.EXT\0`, ≤13 chars) or a full `V_TEXT`
/// formatted line (name/size/date/time columns).
pub const LEN_LABEL: usize = 48;

/// Parallel-to-`g_screen[]` per-object auxiliary data (EmuTOS `SCREENINFO`/
/// `ICONINFO`, `deskbind.h:89-102`). Carries the data that does not fit in
/// the fixed-size `OBJECT`.
///
/// Both [`ScreenInfo::block`] and [`ScreenInfo::label`] live at a **stable
/// address** for the desktop's lifetime: the `g_screeninfo` [`Vec`] is
/// allocated once by `obj_init` and never reallocated, so the raw pointers
/// baked into the `OBJECT` tree (`ob_spec` → `block`, `block.ib_ptext` →
/// `label`) stay valid while `objc_draw` reads them.
#[derive(Clone)]
pub struct ScreenInfo {
    /// Back-reference to the [`FNode`] this object represents: the entry's
    /// index in its window's `p_flist`, or [`NIL`] for a desktop icon
    /// (`ANODE`-backed, no `FNode`) or an unused slot.
    pub fnode: i16,
    /// The per-instance `ICONBLK` copy this object's `ob_spec` points at
    /// (each displayed icon owns its own copy so it can carry its own
    /// `ib_ptext`/`ib_char`).
    pub block: IconBlk,
    /// The per-instance NUL-terminated label the icon's `ib_ptext` points at
    /// (a copy, so it outlives the source `FNode` and stays put in memory).
    pub label: [u8; LEN_LABEL],
    /// Which `DeskState::g_iblist[]` entry [`ScreenInfo::block`] was copied
    /// from (`ICONINFO::index`).
    pub index: u16,
    /// What this object represents for drag-and-drop dispatch (`ANODE.a_type`).
    pub atype: AType,
}

/// A fully-zeroed `ICONBLK`, used to seed empty [`ScreenInfo`] slots.
pub const ZERO_ICONBLK: IconBlk = IconBlk {
    ib_pmask: 0,
    ib_pdata: 0,
    ib_ptext: 0,
    ib_char: 0,
    ib_xchar: 0,
    ib_ychar: 0,
    ib_xicon: 0,
    ib_yicon: 0,
    ib_wicon: 0,
    ib_hicon: 0,
    ib_xtext: 0,
    ib_ytext: 0,
    ib_wtext: 0,
    ib_htext: 0,
};

impl Default for ScreenInfo {
    fn default() -> Self {
        Self { fnode: NIL, block: ZERO_ICONBLK, label: [0; LEN_LABEL], index: 0, atype: AType::None }
    }
}

/// Scan `spec` (e.g. `C:\*.*`) through GEMDOS `Fsfirst`/`Fsnext`, returning
/// the matched [`FNode`]s and the summed file size. This is the faithful
/// `pn_active()` loop: point the current DTA at a local buffer, walk the
/// matches, and copy each DTA field out individually.
///
/// `mask` is the File → Set-mask display filter (EmuTOS `pn_active`'s
/// `match`): applied via [`wildcmp`] to **files only** so a `*.TXT` mask hides
/// non-`.TXT` files but never a folder — EmuTOS scans with `*.*`
/// (`del_fname(search)`) and `wildcmp`s the mask against files. An empty mask
/// or `*.*` disables filtering.
fn scan_dir(spec: &str, attr: u8, include_folders: bool, mask: &str) -> (Vec<FNode>, u64) {
    // A `*.*` (or empty) mask matches everything, so skip the per-file test.
    let apply_mask = !mask.is_empty() && mask != "*.*";
    let mask_b = mask.as_bytes();
    // A 44-byte DTA buffer (`docs/api/structures/dta.md`). Kept alive for the
    // whole scan; the current-DTA pointer is a single-foreground-caller cell
    // in `tos_api::bdos::fs::find`, matching EmuTOS's `p_xdta` model.
    // Mutable: the GEMDOS trap writes the found entry back through this
    // pointer (mutable provenance), even though this binding never reads it
    // via `&mut`.
    let mut dta = [0u8; 44];
    dos_sdta(dta.as_mut_ptr());

    let mut list: Vec<FNode> = Vec::new();
    let mut total: u64 = 0;
    let mut seq: u16 = 0;

    let mut rc = dos_sfirst(spec, attr);
    while rc == E_OK {
        if let Some(f) = fnode_from_dta(&dta, seq, include_folders) {
            let is_file = f.f_attr & FA_SUBDIR == 0;
            // File-mask filter: skip files that don't match; folders always pass.
            if !(apply_mask && is_file && !wildcmp(mask_b, f.name_bytes())) {
                if is_file {
                    total = total.saturating_add(f.f_size as u64);
                }
                list.push(f);
                seq = seq.wrapping_add(1);
            }
        }
        rc = dos_snext();
    }
    (list, total)
}

/// `wildcmp` (EmuTOS `util/optimize.c:244`) — does 8.3 `filename` match the TOS
/// wildcard `pattern`? Both are in `NAME.EXT` display form, uppercase; `*`
/// matches the rest of a field, `?` matches one character. The name is matched
/// on pass 1 and the extension on pass 2 (a `.`-less pattern/name has an empty
/// extension field). A byte-faithful reimplementation of the algorithm the
/// desktop's Set-mask and Search share with GEMDOS's own matcher; past the end
/// of either string the read is `0` (C NUL semantics), so the final compare is
/// "both exhausted".
pub(super) fn wildcmp(pattern: &[u8], filename: &[u8]) -> bool {
    let at = |s: &[u8], i: usize| -> u8 { s.get(i).copied().unwrap_or(0) };
    let mut p = 0usize; // pattern cursor
    let mut f = 0usize; // filename cursor
    // Pass 0 = name field, pass 1 = extension field.
    for _ in 0..2 {
        while at(filename, f) != 0 && at(filename, f) != b'.' {
            let pc = at(pattern, p);
            if pc == b'*' {
                f += 1; // '*' consumes the rest of the field; pattern stays put
                continue;
            }
            if pc == b'?' || pc == at(filename, f) {
                p += 1;
                f += 1;
                continue;
            }
            return false;
        }
        // Soak up any remaining wildcard characters in this field.
        while at(pattern, p) == b'*' || at(pattern, p) == b'?' {
            p += 1;
        }
        // Step past the field separator in each string, if present.
        if at(pattern, p) == b'.' {
            p += 1;
        }
        if at(filename, f) == b'.' {
            f += 1;
        }
    }
    at(pattern, p) == at(filename, f)
}

/// Decode one populated DTA into an [`FNode`], copying each field
/// individually (no `memcpy` overlap trick). Returns `None` for `.`/`..` or
/// for a folder when `include_folders` is false. Multi-byte values are read
/// little-endian, matching how `tos_api::bdos::fs::find::makbuf` writes them.
fn fnode_from_dta(dta: &[u8; 44], seq: u16, include_folders: bool) -> Option<FNode> {
    let attr = dta[21];

    let mut name = [0u8; 14];
    name.copy_from_slice(&dta[30..44]);

    // Skip the "." and ".." pseudo-entries.
    let is_dot = name[0] == b'.' && (name[1] == 0 || (name[1] == b'.' && name[2] == 0));
    if is_dot {
        return None;
    }

    let is_dir = attr & FA_SUBDIR != 0;
    if is_dir && !include_folders {
        return None;
    }

    let time = u16::from_le_bytes([dta[22], dta[23]]);
    let date = u16::from_le_bytes([dta[24], dta[25]]);
    let size = u32::from_le_bytes([dta[26], dta[27], dta[28], dta[29]]);

    Some(FNode {
        f_selected: false,
        f_attr: attr,
        f_time: time,
        f_date: date,
        f_size: size,
        f_name: name,
        f_seq: seq,
        f_obid: NIL,
    })
}

/// The "type" key of a `NAME.EXT\0` name for the [`Sort::Type`] order: the
/// substring from the **first** `.` **inclusive of the dot** (`".EXT"`), or the
/// **empty** string when there is no dot at all.
///
/// This is the corrected `S_TYPE` semantics per `deskdir.md` §357-363
/// (Audit-#10 vs `util/optimopt.S:19-38`): EmuTOS `pn_fcomp` compares
/// `scasb(name, '.')`, and `scasb(str, c)` returns a pointer to the **first**
/// occurrence of `c` — or to the terminating `\0` (an **empty** string) when
/// `c` is absent. So the key is `".EXT"` (leading dot included) for a dotted
/// name and `""` for an extensionless one. Extensionless files (`README`,
/// `MAKEFILE`) therefore sort **before** every dotted file (an empty key
/// compares less than any non-empty one), immediately after the folders that
/// `sort_flist`'s folder-first pre-check already split out — **not** by their
/// whole name and **not** by the substring after the last dot.
fn type_key(name: &[u8; 14]) -> &[u8] {
    let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    match name[..end].iter().position(|&c| c == b'.') {
        Some(dot) => &name[dot..end], // from the first '.', dot included
        None => &name[..0],           // no extension → empty key
    }
}

/// Sort `list` in place by `sort`, reproducing EmuTOS `pn_comp`/`pn_fcomp`
/// (`deskfpd.c:99-147`):
///
/// * **Folder-first** pre-check for every key except [`Sort::NoSort`]
///   (`pn_comp`, `deskfpd.c:138-147`).
/// * **Name is the final tiebreak for every key** (`pn_fcomp`'s
///   `if (chk) return chk; return strcmp(...)` structure, `deskfpd.c:124-127`).
/// * [`Sort::Size`] and [`Sort::Date`] are **descending** (largest / newest
///   first — `pf2 - pf1`).
/// * [`Sort::Type`] compares the extension from the **first** dot inclusive
///   ([`type_key`]); extensionless files (empty key) sort ahead of dotted ones.
/// * [`Sort::NoSort`] restores the directory scan order via the unique
///   `f_seq`, with **no** folder-first pre-check (files and folders
///   interleave in raw `Fsfirst`/`Fsnext` order).
fn sort_flist(list: &mut [FNode], sort: Sort) {
    use core::cmp::Ordering as O;

    if matches!(sort, Sort::NoSort) {
        list.sort_by_key(|f| f.f_seq);
        return;
    }

    list.sort_by(|a, b| {
        // Folder-first pre-check (applied on top of whatever key follows).
        let (ad, bd) = (a.is_dir(), b.is_dir());
        if ad != bd {
            return if ad { O::Less } else { O::Greater };
        }
        // Per-key comparison; name is always the final tiebreak.
        let key = match sort {
            Sort::Type => type_key(&a.f_name).cmp(type_key(&b.f_name)),
            // Descending — largest first.
            Sort::Size => b.f_size.cmp(&a.f_size),
            // Descending — newest first (date, then time).
            Sort::Date => (b.f_date, b.f_time).cmp(&(a.f_date, a.f_time)),
            // Sort::Name: the whole key is the name tiebreak below.
            _ => O::Equal,
        };
        key.then_with(|| a.f_name.cmp(&b.f_name))
    });
}
