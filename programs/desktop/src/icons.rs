//! `desktop::icons` — NewTOS's built-in desktop icon set (`G.g_iblist`-equivalent)
//! and the per-instance `ICONBLK` builder.
//!
//! Reimplements EmuTOS's built-in `icon.rsc` bitmap set (`deskapp.c`'s
//! `app_rdicon`/`setup_iconblks` consumers) as native NewTOS data. The 32×32
//! 1-bpp mask/data rasters are ported from the legacy `crate::desktop_icons`
//! (which was itself extracted from `references/emutos/desk/icon.rsc`); the
//! `ICONBLK` geometry is **NewTOS's own** per `_rebuild-plan.md` §5: NewTOS
//! owns its `ib_xtext` centring offset and derives every cell metric from the
//! grid-metric contract, not from a baked resource constant.
//!
//! **The D1 fix lives here and in [`super::app`]:** the label box is a
//! **fixed** [`MAX_ICONTEXT_WIDTH`]-character width ([`ib_wtext`](IconBlk::ib_wtext)),
//! **never** shrunk to the actual name length. The icon cell is that fixed
//! label width **plus `2 * IB_XTEXT`** — so every cell is exactly `g_wicon`
//! wide and a full 12-char 8.3 name fills its box without ever spilling into
//! the neighbouring column.

use tos_types::aes::IconBlk;
use super::global::{gl_hschar, gl_wschar};

/// Maximum icon-label length in characters (EmuTOS `MAX_ICONTEXT_WIDTH`,
/// `deskapp.c:74`). A 12-char 8.3 name (`NAME.EXT`) is the worst case.
pub const MAX_ICONTEXT_WIDTH: i16 = 12;

/// The icon bitmap's fixed width/height in pixels (32×32, from `icon.rsc`).
pub const IB_WICON: i16 = 32;
/// The icon bitmap's fixed height in pixels.
pub const IB_HICON: i16 = 32;

/// The label box's horizontal centring offset inside the icon cell
/// (**NewTOS design choice**, `_rebuild-plan.md` §5): a small fixed inset so
/// the cell width is `ib_wtext + 2 * IB_XTEXT`. EmuTOS bakes this into the
/// icon resource's `ib_xtext`; NewTOS picks it here and feeds it through the
/// grid-metric **formula** everywhere (`app_start`'s `g_wicon`).
pub const IB_XTEXT: i16 = 2;

/// The label box's vertical slack: EmuTOS makes the box **two pixels taller**
/// than the small font's cell (`ib_htext = gl_hschar + 2`, `deskapp.c:520`)
/// and the same `+2` shows up in the cell height (`g_hicon = ib_hicon +
/// gl_hschar + 2`, `deskapp.c:811`). It is *not* a gap above the label — the
/// box starts flush at the bitmap's bottom edge (`ib_ytext = ih`,
/// `deskapp.c:518`).
pub const ICON_LABEL_SLACK: i16 = 2;

/// The fixed label-box width in pixels: `MAX_ICONTEXT_WIDTH * gl_wschar`
/// (EmuTOS `setup_iconblks`, `deskapp.c:519`) — the **small** AES font, which
/// is what `gr_gicon` draws an icon label with (`gemgraf.c:806`). **Never**
/// shrink-to-fit — this is the root D1 fix. 12 * 6 = 72 with the 6×6 small
/// font, exactly the `ib_wtext` in EmuTOS's own `desk/icon.rsc` and in
/// TeraDesk's `icons.rsc`.
pub fn ib_wtext() -> i16 {
    MAX_ICONTEXT_WIDTH * gl_wschar()
}

/// The fixed label-box height in pixels: `gl_hschar + 2` (EmuTOS
/// `setup_iconblks`, `deskapp.c:520`) = 8 with the 6×6 small font, again
/// exactly the `ib_htext` both stock icon resources carry.
pub fn ib_htext() -> i16 {
    gl_hschar() + ICON_LABEL_SLACK
}

/// Which built-in icon a displayed object uses. The variant set is NewTOS's;
/// the **numbering** that reaches `NEWDESK.INF` is EmuTOS's `IG_*`
/// ([`ig_of`]/[`icon_kind`]), because the `a_aicon`/`a_dicon` byte of an
/// `ANODE` line is file-format, not internal.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    /// Hard-disk drive (`C:`..`Z:`) — also `g_iblist[0]`, whose `ib_hicon`
    /// sizes every cell (EmuTOS `deskapp.c:810`).
    Hard,
    /// Floppy-disk drive (`A:`/`B:`).
    Floppy,
    /// Directory / folder.
    Folder,
    /// Trash can.
    Trash,
    /// Printer.
    Printer,
    /// Removable disk (`icon.rsc` ICONBLK[5]) — the hard-disk silhouette with
    /// an eject slot instead of the front-panel detail, and the same drive
    /// letter overlay. Shares `rs_iconmask0` with [`IconKind::Hard`], exactly
    /// as the resource does (`icons.c`'s `icon_rs_iconblk[5]`).
    Removable,
    /// Generic document / data file (the `win_bldview` default for a file
    /// with no matching installed application).
    Doc,
    /// Generic application / executable program — the `win_bldview` icon for a
    /// native NewTOS program (`.NAP`/`.NCP`/`.NTP`, see [`super::ftype`]). The
    /// EmuTOS "generic app" icon (`icon.rsc` ICONBLK[6]).
    App,
}

// ── EmuTOS's built-in icon numbering (`deskapp.h:54-62`) ────────────────────
//
// These eight numbers are **file format**, not an internal detail: they are the
// `a_aicon`/`a_dicon` byte of every `ANODE` line in `NEWDESK.INF`/`EMUDESK.INF`
// (`app_parse`, `deskapp.c:359-365`; `app_save`, `:1258`), and the built-in
// default INF `EmuTOS` synthesises writes them literally
// (`desk_inf_data2`, `deskapp.c:181-189`: `07` = document, `02` = folder,
// `06` = application). A file written by EmuTOS and read by NewTOS — or the
// other way round — has to agree on them, so NewTOS uses EmuTOS's values.

/// `IG_HARD` (`deskapp.h:54`) — hard drive.
pub const IG_HARD: i16 = 0;
/// `IG_FLOPPY` (`deskapp.h:55`) — floppy drive.
pub const IG_FLOPPY: i16 = 1;
/// `IG_FOLDER` (`deskapp.h:56`) — folder.
pub const IG_FOLDER: i16 = 2;
/// `IG_TRASH` (`deskapp.h:57`) — trash can.
pub const IG_TRASH: i16 = 3;
/// `IG_PRINT` (`deskapp.h:58`) — printer.
pub const IG_PRINT: i16 = 4;
/// `IG_REMOV` (`deskapp.h:59`) — removable disk.
pub const IG_REMOV: i16 = 5;
/// `IG_APPL` (`deskapp.h:60`) — generic application icon.
pub const IG_APPL: i16 = 6;
/// `IG_DOCU` (`deskapp.h:61`) — generic document icon.
pub const IG_DOCU: i16 = 7;

/// `NUM_GEM_IBLKS` (`deskapp.h:62`) — the number of built-in icons, which is
/// what `app_parse` clamps `a_aicon`/`a_dicon` against (`deskapp.c:359-365`,
/// where it reads it out of the live `G.g_numiblks`; this desktop loads no
/// user icon resource, so the two are the same number).
pub const NUM_GEM_IBLKS: i16 = IG_DOCU + 1;

/// Every built-in icon, in EmuTOS's `IG_*` order — our `G.g_iblist[]`
/// (`deskapp.c` `app_rdicon`). All eight of `icon.rsc`'s enabled `ICONBLK`s,
/// so the Install-icon stepper's `G.g_numiblks` equivalent is
/// `BUILTIN_ICONS.len()` = [`NUM_GEM_IBLKS`].
pub const BUILTIN_ICONS: [(i16, IconKind); NUM_GEM_IBLKS as usize] = [
    (IG_HARD, IconKind::Hard),
    (IG_FLOPPY, IconKind::Floppy),
    (IG_FOLDER, IconKind::Folder),
    (IG_TRASH, IconKind::Trash),
    (IG_PRINT, IconKind::Printer),
    (IG_REMOV, IconKind::Removable),
    (IG_APPL, IconKind::App),
    (IG_DOCU, IconKind::Doc),
];

/// The [`IconKind`] for an EmuTOS `IG_*` number, or [`None`] for a number that
/// names no picture — `-1` (`NIL`, the `FF` an `ANODE` line writes for "no
/// icon") and anything `>= NUM_GEM_IBLKS`, which `app_parse` clamps away on
/// the way in (`deskapp.c:359-365`).
pub fn icon_kind(ig: i16) -> Option<IconKind> {
    BUILTIN_ICONS.iter().find(|&&(n, _)| n == ig).map(|&(_, k)| k)
}

/// The EmuTOS `IG_*` number of an [`IconKind`] — what [`super::inf`] writes
/// into the `ANODE` line's `a_aicon` field.
pub fn ig_of(kind: IconKind) -> i16 {
    BUILTIN_ICONS.iter().find(|&&(_, k)| k == kind).map(|&(n, _)| n).unwrap_or(IG_HARD)
}

// ── 32×32 1-bpp rasters (2 words/row, big-endian, MSB = leftmost pixel),
//    ported from `crate::desktop_icons` (EmuTOS `desk/icon.rsc`). Held as
//    `static` so the `ICONBLK` pointers (`ib_pmask`/`ib_pdata`) are stable. ──

static HARD_DATA: [u16; 64] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x3fff, 0xfff8, 0x4000, 0x0004, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006,
    0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x5c66, 0x6666,
    0x5ccc, 0xccc6, 0x4199, 0x9986, 0x4333, 0x3306, 0x4666, 0x6606, 0x4000, 0x0006, 0x3fff, 0xfffe,
    0x1fff, 0xfffc, 0x0000, 0x0000,
];
static HARD_MASK: [u16; 64] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x3fff, 0xfff8,
    0x7fff, 0xfffc, 0xffff, 0xfffe, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
    0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
    0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0x7fff, 0xffff,
    0x3fff, 0xfffe, 0x1fff, 0xfffc,
];

static FLOPPY_DATA: [u16; 64] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0fff, 0xff80, 0x0ac0, 0x06c0, 0x0d40, 0xe560, 0x0ac0, 0xe6b0, 0x0d40, 0xe570,
    0x0ac0, 0xe6b0, 0x0d40, 0x0570, 0x0abf, 0xfab0, 0x0d55, 0x5570, 0x0aaa, 0xaab0, 0x0d55, 0x5570,
    0x0bff, 0xffb0, 0x0e00, 0x00f0, 0x0a00, 0x00b0, 0x0e00, 0x00f0, 0x0a00, 0x00b0, 0x0e00, 0x00f0,
    0x0a00, 0x00b0, 0x0e00, 0x00f0, 0x0a00, 0x00b0, 0x0e00, 0x00f0, 0x0a00, 0x00b0, 0x0fff, 0xfff0,
    0x07ff, 0xfff0, 0x0000, 0x0000,
];
static FLOPPY_MASK: [u16; 64] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0fff, 0xff80, 0x1fff, 0xffc0, 0x1fff, 0xffe0, 0x1fff, 0xfff0, 0x1fff, 0xfff8, 0x1fff, 0xfff8,
    0x1fff, 0xfff8, 0x1fff, 0xfff8, 0x1fff, 0xfff8, 0x1fff, 0xfff8, 0x1fff, 0xfff8, 0x1fff, 0xfff8,
    0x1fff, 0xfff8, 0x1fff, 0xfff8, 0x1fff, 0xfff8, 0x1fff, 0xfff8, 0x1fff, 0xfff8, 0x1fff, 0xfff8,
    0x1fff, 0xfff8, 0x1fff, 0xfff8, 0x1fff, 0xfff8, 0x1fff, 0xfff8, 0x1fff, 0xfff8, 0x0fff, 0xfff8,
    0x07ff, 0xfff0, 0x0000, 0x0000,
];

static TRASH_DATA: [u16; 64] = [
    0x0000, 0x0000, 0x0007, 0xc000, 0x0008, 0x2000, 0x0008, 0x2000, 0x07ff, 0xffc0, 0x0800, 0x0020,
    0x0800, 0x0020, 0x0800, 0x0020, 0x07ff, 0xffc0, 0x03ff, 0xff80, 0x0200, 0x0080, 0x0221, 0x0880,
    0x0221, 0x0880, 0x0221, 0x0880, 0x0221, 0x0880, 0x0221, 0x0880, 0x0221, 0x0880, 0x0221, 0x0880,
    0x0221, 0x0880, 0x0221, 0x0880, 0x0221, 0x0880, 0x0221, 0x0880, 0x0221, 0x0880, 0x0221, 0x0880,
    0x0221, 0x0880, 0x0221, 0x0880, 0x0221, 0x0880, 0x0221, 0x0880, 0x0200, 0x0080, 0x01ff, 0xff00,
    0x00ff, 0xfe00, 0x0000, 0x0000,
];
static TRASH_MASK: [u16; 64] = [
    0x000f, 0xe000, 0x001f, 0xf000, 0x001f, 0xf000, 0x0fff, 0xffe0, 0x1fff, 0xfff0, 0x1fff, 0xfff0,
    0x1fff, 0xfff0, 0x1fff, 0xfff0, 0x1fff, 0xfff0, 0x0fff, 0xffe0, 0x07ff, 0xffc0, 0x07ff, 0xffc0,
    0x07ff, 0xffc0, 0x07ff, 0xffc0, 0x07ff, 0xffc0, 0x07ff, 0xffc0, 0x07ff, 0xffc0, 0x07ff, 0xffc0,
    0x07ff, 0xffc0, 0x07ff, 0xffc0, 0x07ff, 0xffc0, 0x07ff, 0xffc0, 0x07ff, 0xffc0, 0x07ff, 0xffc0,
    0x07ff, 0xffc0, 0x07ff, 0xffc0, 0x07ff, 0xffc0, 0x07ff, 0xffc0, 0x07ff, 0xffc0, 0x07ff, 0xffc0,
    0x03ff, 0xff80, 0x01ff, 0xff00,
];

static PRINTER_DATA: [u16; 64] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x001f, 0xf800, 0x0020, 0x0400, 0x0020, 0x0400,
    0x0020, 0x0400, 0x0020, 0x0400, 0x0020, 0x0400, 0x0020, 0x0400, 0x0020, 0x0400, 0x07ff, 0xffe0,
    0x1800, 0x0018, 0x1000, 0x0008, 0x2700, 0x00e4, 0x2700, 0x0004, 0x2000, 0x00e4, 0x2000, 0x0004,
    0x2000, 0x00e4, 0x2000, 0x0004, 0x2000, 0x0004, 0x23ff, 0xffc4, 0x22a0, 0x0544, 0x2340, 0x02c4,
    0x22df, 0xfb44, 0x3f80, 0x01fc, 0x00bf, 0xfd00, 0x0100, 0x0080, 0x017f, 0xfe80, 0x0200, 0x0040,
    0x03ff, 0xffc0, 0x0000, 0x0000,
];
static PRINTER_MASK: [u16; 64] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x003f, 0xfc00, 0x007f, 0xfe00, 0x007f, 0xfe00, 0x007f, 0xfe00,
    0x007f, 0xfe00, 0x007f, 0xfe00, 0x007f, 0xfe00, 0x007f, 0xfe00, 0x0fff, 0xfff0, 0x3fff, 0xfffc,
    0x3fff, 0xfffc, 0x7fff, 0xfffe, 0x7fff, 0xfffe, 0x7fff, 0xfffe, 0x7fff, 0xfffe, 0x7fff, 0xfffe,
    0x7fff, 0xfffe, 0x7fff, 0xfffe, 0x7fff, 0xfffe, 0x7fff, 0xfffe, 0x7fff, 0xfffe, 0x7fff, 0xfffe,
    0x7fff, 0xfffe, 0x7fff, 0xfffe, 0x7fff, 0xfffe, 0x03ff, 0xffc0, 0x07ff, 0xffe0, 0x07ff, 0xffe0,
    0x07ff, 0xffe0, 0x07ff, 0xffe0,
];

static FOLDER_DATA: [u16; 64] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x3ffe, 0x0000, 0x2aab, 0x0000,
    0x3555, 0xfff8, 0x2aaa, 0xaaa8, 0x3557, 0xfffc, 0x2aac, 0x0006, 0x7ff9, 0x1116, 0x4000, 0x0006,
    0x4444, 0x4446, 0x4000, 0x0006, 0x5111, 0x1116, 0x4000, 0x0006, 0x4444, 0x4446, 0x4000, 0x0006,
    0x5111, 0x1116, 0x4000, 0x0006, 0x4444, 0x4446, 0x4000, 0x0006, 0x5111, 0x1116, 0x4000, 0x0006,
    0x4444, 0x4446, 0x4000, 0x0006, 0x5111, 0x1116, 0x6aaa, 0xaaae, 0x5555, 0x5556, 0x3fff, 0xfffe,
    0x1fff, 0xfffc, 0x0000, 0x0000,
];
static FOLDER_MASK: [u16; 64] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x7fff, 0x0000, 0x7fff, 0x8000, 0x7fff, 0xfffc,
    0x7fff, 0xfffc, 0x7fff, 0xfffe, 0x7fff, 0xffff, 0x7fff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
    0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
    0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
    0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0x7fff, 0xffff,
    0x3fff, 0xfffe, 0x0000, 0x0000,
];

static DOC_DATA: [u16; 64] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x03ff, 0xf800, 0x0400, 0x1c00,
    0x0400, 0x1600, 0x0400, 0x1300, 0x0400, 0x1180, 0x0400, 0x10c0, 0x0400, 0x0fe0, 0x0400, 0x0060,
    0x0400, 0x0060, 0x0400, 0x0060, 0x0400, 0x0060, 0x0400, 0x0060, 0x0400, 0x0060, 0x0400, 0x0060,
    0x0400, 0x0060, 0x0400, 0x0060, 0x0400, 0x0060, 0x0400, 0x0060, 0x0400, 0x0060, 0x0400, 0x0060,
    0x0400, 0x0060, 0x0400, 0x0060, 0x0400, 0x0060, 0x0400, 0x0060, 0x0400, 0x0060, 0x03ff, 0xffe0,
    0x01ff, 0xffc0, 0x0000, 0x0000,
];
static DOC_MASK: [u16; 64] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x03ff, 0xf800, 0x07ff, 0xfc00, 0x0fff, 0xfe00,
    0x0fff, 0xff00, 0x0fff, 0xff80, 0x0fff, 0xffc0, 0x0fff, 0xffe0, 0x0fff, 0xfff0, 0x0fff, 0xfff0,
    0x0fff, 0xfff0, 0x0fff, 0xfff0, 0x0fff, 0xfff0, 0x0fff, 0xfff0, 0x0fff, 0xfff0, 0x0fff, 0xfff0,
    0x0fff, 0xfff0, 0x0fff, 0xfff0, 0x0fff, 0xfff0, 0x0fff, 0xfff0, 0x0fff, 0xfff0, 0x0fff, 0xfff0,
    0x0fff, 0xfff0, 0x0fff, 0xfff0, 0x0fff, 0xfff0, 0x0fff, 0xfff0, 0x0fff, 0xfff0, 0x07ff, 0xfff0,
    0x03ff, 0xffe0, 0x01ff, 0xffc0,
];

// Generic application / executable icon (32×32 1-bpp, ported byte-for-byte from
// EmuTOS `desk/icon.rsc` ICONBLK[6] "generic app": a box with a striped
// title-bar header — the classic GEM program look).
static APP_DATA: [u16; 64] = [
    0x0000, 0x0000, 0x3fff, 0xfff8, 0x46aa, 0xaac4, 0x5555, 0x5556, 0x46aa, 0xaac6, 0x7fff, 0xfffe,
    0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006,
    0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006,
    0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006,
    0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x3fff, 0xfffe,
    0x1fff, 0xfffc, 0x0000, 0x0000,
];
static APP_MASK: [u16; 64] = [
    0x3fff, 0xfff8, 0x7fff, 0xfffc, 0xffff, 0xfffe, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
    0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
    0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
    0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff,
    0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0x7fff, 0xffff,
    0x3fff, 0xfffe, 0x1fff, 0xfffc,
];

// Removable-disk icon (32×32 1-bpp, `desk/icon.rsc` ICONBLK[5]): the same
// drive body as the hard disk, with a wide eject slot where the hard disk has
// its front-panel detail. The resource gives it **no mask of its own** — it
// points at `rs_iconmask0`, the hard disk's (`icons.c`'s `icon_rs_iconblk[5]`)
// — so [`rasters`] hands out `HARD_MASK` for it, as the resource does.
static REMOV_DATA: [u16; 64] = [
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    0x3fff, 0xfff8, 0x4000, 0x0004, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006,
    0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x4000, 0x0006, 0x5c7f, 0xffc6,
    0x5c7f, 0xffc6, 0x4000, 0x0006, 0x4199, 0x9986, 0x4333, 0x3306, 0x4000, 0x0006, 0x3fff, 0xfffe,
    0x1fff, 0xfffc, 0x0000, 0x0000,
];

/// The (data, mask) raster pair for an [`IconKind`].
fn rasters(kind: IconKind) -> (&'static [u16; 64], &'static [u16; 64]) {
    match kind {
        IconKind::Hard => (&HARD_DATA, &HARD_MASK),
        IconKind::Floppy => (&FLOPPY_DATA, &FLOPPY_MASK),
        IconKind::Folder => (&FOLDER_DATA, &FOLDER_MASK),
        IconKind::Trash => (&TRASH_DATA, &TRASH_MASK),
        IconKind::Printer => (&PRINTER_DATA, &PRINTER_MASK),
        IconKind::Removable => (&REMOV_DATA, &HARD_MASK),
        IconKind::Doc => (&DOC_DATA, &DOC_MASK),
        IconKind::App => (&APP_DATA, &APP_MASK),
    }
}

/// Where the drive-letter glyph sits within the 32-px icon bitmap (x, y),
/// ported from `desktop_icons::HARD_CHAR`/`FLOPPY_CHAR`. `None` for icons
/// that carry no letter.
fn char_spot(kind: IconKind) -> Option<(i16, i16)> {
    match kind {
        // `icon_rs_iconblk[0]` and `[5]` carry the same `ib_xchar`/`ib_ychar`
        // (13, 16), `[1]` carries (13, 22) — `icons.c`, generated from
        // `icon.rsc` by `ird`.
        IconKind::Hard | IconKind::Removable => Some((13, 16)),
        IconKind::Floppy => Some((13, 22)),
        _ => None,
    }
}

// Colour-word nibbles for `ib_char`: fg = black (VDI pen 1), bg = white (0)
// (EmuTOS `[15:12]` fg / `[11:8]` bg). The low byte carries the drive letter.
const IB_CHAR_BASE: i16 = 0x1000;

/// Build a fully-populated per-instance [`IconBlk`] for `kind`, its label at
/// `ptext`, an optional drive `letter` (0 = none), sized to the current
/// `g_wicon` cell width.
///
/// Reproduces the geometry both EmuTOS placement paths agree on:
/// * the **icon bitmap** is horizontally centred in the cell
///   (`ib_xicon = (g_wicon - ib_wicon) / 2`, the `app_blddesk` recentre,
///   `deskapp.c:1391`; on NewTOS's fixed font the window path's "as-authored"
///   value is identical, so both views centre consistently);
/// * the **label box** is the fixed [`ib_wtext`] wide, inset by [`IB_XTEXT`]
///   — **never** shrunk to the name (the D1 fix).
pub fn build_iconblk(kind: IconKind, ptext: u64, letter: u8, g_wicon: i16) -> IconBlk {
    let (data, mask) = rasters(kind);
    // `ib_xchar`/`ib_ychar` are relative to the icon **bitmap**, not to the
    // object: `gr_gicon` draws the overlay character at `ib_xicon + ib_xchar`
    // (EmuTOS `gemgraf.c:802`), so the bitmap's centring offset must NOT be
    // baked in here — the AES adds it.
    let (ib_char, ib_xchar, ib_ychar) = match (letter, char_spot(kind)) {
        (0, _) | (_, None) => (IB_CHAR_BASE, 0, 0),
        (c, Some((sx, sy))) => (IB_CHAR_BASE | c as i16, sx, sy),
    };
    IconBlk {
        ib_pmask: mask.as_ptr() as u64,
        ib_pdata: data.as_ptr() as u64,
        ib_ptext: ptext,
        ib_char,
        ib_xchar,
        ib_ychar,
        ib_xicon: (g_wicon - IB_WICON) / 2,
        ib_yicon: 0,
        ib_wicon: IB_WICON,
        ib_hicon: IB_HICON,
        ib_xtext: IB_XTEXT,
        ib_ytext: IB_HICON,
        ib_wtext: ib_wtext(),
        ib_htext: ib_htext(),
    }
}
