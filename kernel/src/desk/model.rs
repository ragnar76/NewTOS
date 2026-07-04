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
//! The directory scan itself goes through GEMDOS `Fsfirst`/`Fsnext`
//! (`tos_api::bdos::fs::find`) exactly like EmuTOS `pn_active()` drives
//! `dos_sfirst`/`dos_snext`.

use alloc::string::String;
use alloc::vec::Vec;

use tos_api::aes::object::IconBlk;
use tos_api::bdos::fs::find;
use tos_api::trap::TrapArgs;
use tos_types::error::E_OK;

use super::global::NIL;

/// FAT attribute bit marking a sub-directory (`bdosdefs.h` `FA_SUBDIR`).
pub const FA_SUBDIR: u8 = 0x10;

/// The attribute mask the desktop passes to `Fsfirst` (`deskbind.h`
/// `DISPATTR`): request sub-directories too (plain files always match).
pub const DISPATTR: u8 = FA_SUBDIR;

// GEMDOS function ids used by the directory scan.
const FSETDTA: u16 = 0x1A;
const FSFIRST: u16 = 0x4E;
const FSNEXT: u16 = 0x4F;
const TRAP_GEMDOS: u16 = 1;

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
    /// The directory path + wildcard (e.g. `C:\*.*`).
    pub p_spec: String,
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
    /// does not scan (call [`PNode::pn_active`] for that).
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
    /// EmuTOS folding those errors to success.
    pub fn pn_active(&mut self, include_folders: bool) {
        let (list, total) = scan_dir(&self.p_spec, self.p_attr, include_folders);
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

/// Build a native GEMDOS [`TrapArgs`] with up to two argument registers.
fn gemdos_args(func: u16, a0: u64, a1: u64) -> TrapArgs {
    TrapArgs {
        regs: [a0, a1, 0, 0, 0, 0, 0, 0],
        func: func as u64,
        svc_imm: TRAP_GEMDOS,
    }
}

/// Scan `spec` (e.g. `C:\*.*`) through GEMDOS `Fsfirst`/`Fsnext`, returning
/// the matched [`FNode`]s and the summed file size. This is the faithful
/// `pn_active()` loop: point the current DTA at a local buffer, walk the
/// matches, and copy each DTA field out individually.
fn scan_dir(spec: &str, attr: u8, include_folders: bool) -> (Vec<FNode>, u64) {
    // A 44-byte DTA buffer (`docs/api/structures/dta.md`). Kept alive for the
    // whole scan; the current-DTA pointer is a single-foreground-caller global
    // in `tos_api::bdos::fs::find`, matching EmuTOS's `p_xdta` model.
    // Mutable: the GEMDOS trap writes the found entry back through this
    // pointer (mutable provenance), even though this binding never reads it
    // via `&mut`.
    let mut dta = [0u8; 44];
    find::fsetdta(&gemdos_args(FSETDTA, dta.as_mut_ptr() as u64, 0));

    // `Fsfirst` reads a NUL-terminated path; build one from the spec.
    let mut cpath = [0u8; 130];
    let sb = spec.as_bytes();
    let n = sb.len().min(cpath.len() - 1);
    cpath[..n].copy_from_slice(&sb[..n]);

    let mut list: Vec<FNode> = Vec::new();
    let mut total: u64 = 0;
    let mut seq: u16 = 0;

    let mut rc = find::fsfirst(&gemdos_args(FSFIRST, cpath.as_ptr() as u64, attr as u64));
    while rc == E_OK {
        if let Some(f) = fnode_from_dta(&dta, seq, include_folders) {
            if f.f_attr & FA_SUBDIR == 0 {
                total = total.saturating_add(f.f_size as u64);
            }
            list.push(f);
            seq = seq.wrapping_add(1);
        }
        rc = find::fsnext(&gemdos_args(FSNEXT, 0, 0));
    }
    (list, total)
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
