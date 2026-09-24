//! `desk::menu` — the desktop menu bar: the `OBJECT` tree (EmuTOS `desktop.rsc`
//! `ADMENU`), its geometry (`adjust_menu`), the enable/disable/check recompute
//! (`men_update` — the **D6** fix), and the keyboard-shortcut lookup.
//!
//! Reimplements the menu-owning half of EmuTOS `deskmain.c` per
//! `docs/api/desktop/deskmain.md` and `_rebuild-plan.md` §8 (the full
//! EmuTOS-default item set, minus Format / Alt-Desktop-Graphics / Easter-Egg).
//! Each drop-down item carries a semantic [`Item`] id so `hndl_menu` can
//! dispatch a `MN_SELECTED` (object index → [`Item`]) and `men_update` can gate
//! it — the object indices themselves are an internal layout detail.
//!
//! ## `men_update` goes through `menu_ienable` / `menu_icheck`
//!
//! EmuTOS `men_update` gates its items with the AES bindings
//! (`deskmain.c:456,534-585`), and so does [`men_update`] here — opcodes 32
//! and 31 via the GEM trap.
//!
//! This module used to write the `DISABLED`/`CHECKED` `ob_state` bits straight
//! into its own tree, on the stated grounds that the two AES calls "redraw the
//! item unconditionally" and would smear a closed drop-down across the desktop.
//! That was not true of our AES: `menu_icheck` passes `dodraw = false`
//! (`aes/menu.rs:717`, matching `gemsuper.c:172`'s
//! `do_chg(…, CHECK_IT, FALSE, FALSE)`), and `menu_ienable` takes `dodraw` from
//! bit 15 of the item word (`gemsuper.c:175-176`), which `men_update` never
//! sets. Neither call paints.
//!
//! The one direct `ob_state` write that remains is the separator disabling in
//! [`build_menu`], which is exactly where EmuTOS writes the bit by hand too
//! (`deskmain.c:475-486`).

use tos_api::vdi::font::{CELL_H, CELL_W};

use super::global::{self, DISABLED, G_BOX, G_IBOX, G_STRING, G_TITLE, LASTOB, MENU_H, NIL};
use super::model::{Sort, View};

use core::cell::UnsafeCell;

use tos_api::aes::object::Object;

/// Fixed system-font character width in pixels (NewTOS 8×16 font, EmuTOS
/// `deskmain.h` `CHAR_WIDTH` assumption).
const CHAR_W: i16 = CELL_W;

/// Minimum width of the *Desk* drop-down in characters — EmuTOS
/// `MIN_DESKMENU_WIDTH` (`deskmain.c:80`, "compatible with Atari TOS"):
/// `adjust_menu` seeds the first box's width search with
/// `MIN_DESKMENU_WIDTH - 1` (`:1602`) so that after its `m++` (`:1620`) the
/// Desk box is at least 20 cells wide whatever its entries measure.
const MIN_DESKMENU_WIDTH: i16 = 20;

/// Height of `THEBAR`, the white `G_BOX` behind the titles — the RSC's
/// `{0, 0, 80, 513}` (`desk_rsc.c:249-253`): `ob_height = 0x0201` = one
/// character cell plus 2 pixels, which `rsrc_obfix` turns into `gl_hchar + 2`
/// (`aes/gemrslib.c:76-82`) = 18 on the 8×16 font. One row short of
/// `gl_hbox` (`MENU_H`): `mn_bar` draws the box and then the separator on
/// row `gl_hbox - 1` (`aes/gemmnlib.c:718-722`), the row `THEBAR` ends above.
/// `THEACTIVE` and the titles stay `gl_hbox` tall (`769` = `gl_hchar + 3`,
/// `desk_rsc.c:255-283`).
const BAR_BOX_H: i16 = CELL_H + 2;

/// Height of one drop-down entry — the RSC's `{0, row, 20, 1}`
/// (`desk_rsc.c:297-341`, and every other ADMENU box's items): `ob_y = row`,
/// `ob_height = 1`, both **character cells**, which `rsrc_obfix` multiplies
/// by `gl_hchar` (`fix_chpos` offsets 1 and 3, `aes/gemrslib.c:53-99`) =
/// `CELL_H` = 16 px on the 8×16 font. The drop-down box itself is as many
/// cells tall as it has entries (`{8, 0, 19, 16}` for the 16-entry *File*
/// box, `desk_rsc.c:345-349`), i.e. `n · ENTRY_H`.
///
/// This is **not** `gl_hbox` (`MENU_H` = `gl_hchar + 3` = 19): the bar row is
/// 19 px because `gl_hbox` is the gadget box height (`aes/gemgraf.c:412-415`),
/// the drop-down rows are plain text cells. `adjust_menu` recomputes the
/// *widths* (`deskmain.c:1608-1635`) and never touches a height, so the
/// entries keep the RSC's.
const ENTRY_H: i16 = CELL_H;

/// `SHORTCUT_SIZE` (`deskmain.h:17`) — the characters every drop-down entry
/// reserves at its right edge for `' ^X'`. `copy_menu_items` adds
/// `1 + SHORTCUT_SIZE` to the widest entry of a menu (`deskmain.c:1781`) and
/// [`install_shortcuts`] writes `'^'` and the key into the last two of them.
const SHORTCUT_SIZE: i16 = 3;

/// `NUM_SHORTCUTS` (`deskmain.h:18`) — the length of the `menu_shortcuts[]`
/// array and of the `NEWDESK.INF` `#K` line.
pub(super) const NUM_SHORTCUTS: usize = 33;

/// Capacity of one RAM drop-down label: the widest entry text of any menu
/// (*Options*' `"  Desktop configuration..."`, 26 characters) plus
/// [`SHORTCUT_SIZE`], a NUL, and head room.
const LABEL_CAP: usize = 40;

/// `shortcut_mapping[]` (`deskmain.c:169-260`) — which menu item each of the
/// 33 `#K` slots names, in EmuTOS's fixed *File* / *View* / *Options* order.
/// `None` is EmuTOS's `0`: "this menu item does not exist in this binary; any
/// shortcut in the corresponding entry of `menu_shortcuts[]` will be ignored"
/// (`:171-173`). The slot **order is part of the `NEWDESK.INF` contract** —
/// a `#K` line written by EmuTOS has to land on the same items here — so the
/// three items NewTOS does not build (`FORMITEM`, `BLITITEM`, `CACHITEM`,
/// all m68k-hardware or floppy features) keep their slots as `None` rather
/// than being squeezed out.
const SHORTCUT_MAPPING: [Option<Item>; NUM_SHORTCUTS] = [
    // 'File' menu.
    Some(Item::Open),          // OPENITEM
    Some(Item::InfoRename),    // SHOWITEM
    Some(Item::Search),        // SRCHITEM
    Some(Item::NewFolder),     // NFOLITEM
    Some(Item::CloseFolder),   // CLOSITEM
    Some(Item::CloseWindow),   // CLSWITEM
    Some(Item::Cycle),         // BTOPITEM
    Some(Item::SelectAll),     // SLCTITEM
    Some(Item::SetMask),       // MASKITEM
    Some(Item::Delete),        // DELTITEM
    None,                      // FORMITEM — no floppy format (CONF_WITH_FORMAT off)
    Some(Item::ExecEmuCon),    // CLIITEM
    Some(Item::Shutdown),      // QUITITEM
    // 'View' menu.
    Some(Item::ShowIcons),     // ICONITEM
    Some(Item::ShowText),      // TEXTITEM
    Some(Item::SortName),      // NAMEITEM
    Some(Item::SortType),      // TYPEITEM
    Some(Item::SortSize),      // SIZEITEM
    Some(Item::SortDate),      // DATEITEM
    Some(Item::NoSort),        // NSRTITEM
    Some(Item::SizeToFit),     // FITITEM
    Some(Item::SetBackground), // BACKGRND
    // 'Options' menu.
    Some(Item::InstallIcon),    // IICNITEM
    Some(Item::InstallApp),     // IAPPITEM
    Some(Item::InstallDevices), // IDSKITEM
    Some(Item::RemoveIcon),     // RICNITEM
    Some(Item::SetPrefs),       // PREFITEM
    Some(Item::DeskConfig),     // CONFITEM
    Some(Item::ChangeRez),      // RESITEM
    Some(Item::ReadInf),        // READITEM
    Some(Item::SaveDesktop),    // SAVEITEM
    None,                       // BLITITEM — m68k blitter
    None,                       // CACHITEM — 68030 cache
];

/// `default_shortcuts[]` (`deskmain.c:269-307`) — the shortcut letters a
/// desktop starts with when `NEWDESK.INF` carries no `#K` line. Entries are the
/// **uppercase letter as displayed**; the key that fires is the corresponding
/// control character.
pub(super) const DEFAULT_SHORTCUTS: [u8; NUM_SHORTCUTS] = [
    // 'File' menu.
    b'O', b'I', 0, b'N', b'H', b'U', b'W', b'A', 0, b'D', 0, b'Z', 0,
    // 'View' menu.
    0, 0, 0, 0, 0, 0, 0, 0, 0,
    // 'Options' menu.
    0, 0, 0, 0, 0, 0, b'R', 0, b'S', 0, 0,
];

/// A top-level menu (the four ADMENU titles).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Title {
    /// The Desk menu (About/Info + registered accessories).
    Desk,
    /// The File menu (Open/Close/Cycle + the Task-#9 file ops).
    File,
    /// The View menu (icon/text view + sort order + size-to-fit).
    View,
    /// The Options menu (install / preferences / resolution — all Task #9).
    Options,
}

/// A selectable menu item, carried on its drop-down `OBJECT` so a
/// `MN_SELECTED` message (or a keyboard shortcut) resolves to a semantic
/// action. Discriminants are dense (`0..N_ITEMS`) for the object-index maps.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Item {
    // Desk.
    /// Desktop info / About box.
    About = 0,
    // File.
    /// Open the selected folder / drive.
    Open,
    /// Info/Rename dialog (Task #9).
    InfoRename,
    /// Search (Task #9).
    Search,
    /// New folder (Task #9).
    NewFolder,
    /// Close the current folder back toward its root.
    CloseFolder,
    /// Close the top window entirely.
    CloseWindow,
    /// Cycle windows bottom-to-top.
    Cycle,
    /// Select every item in the window (Task #9 selection machine).
    SelectAll,
    /// Set the file mask (Task #9).
    SetMask,
    /// Delete the selection (Task #9).
    Delete,
    /// Launch EmuCON (Task #9 shell integration).
    ExecEmuCon,
    /// Shut the desktop down (Task #9 shutdown wiring).
    Shutdown,
    // View.
    /// Show the directory as icons.
    ShowIcons,
    /// Show the directory as text rows.
    ShowText,
    /// Sort by name.
    SortName,
    /// Sort by type (extension).
    SortType,
    /// Sort by size.
    SortSize,
    /// Sort by date.
    SortDate,
    /// Leave the directory unsorted.
    NoSort,
    /// Size windows to fit their content.
    SizeToFit,
    /// Set the background pattern — `BACKGRND` (46), the **last** entry of
    /// *View* (`desk_rsc.h:35-48`, DESK21).
    SetBackground,
    // Options.
    /// Install icon (Task #9).
    InstallIcon,
    /// Install application (Task #9).
    InstallApp,
    /// Install disk devices (Task #9).
    InstallDevices,
    /// Remove a desktop icon (Task #9).
    RemoveIcon,
    /// Set preferences (Task #9).
    SetPrefs,
    /// Desktop configuration (Task #9).
    DeskConfig,
    /// Change resolution (Task #9).
    ChangeRez,
    /// Read a NEWDESK.INF file (Task #9).
    ReadInf,
    /// Save the desktop (Task #9).
    SaveDesktop,
}

/// Number of [`Item`] variants.
const N_ITEMS: usize = 31;

/// Every [`Item`] in discriminant order — the object-index → item back-map.
const ALL_ITEMS: [Item; N_ITEMS] = [
    Item::About,
    Item::Open,
    Item::InfoRename,
    Item::Search,
    Item::NewFolder,
    Item::CloseFolder,
    Item::CloseWindow,
    Item::Cycle,
    Item::SelectAll,
    Item::SetMask,
    Item::Delete,
    Item::ExecEmuCon,
    Item::Shutdown,
    Item::ShowIcons,
    Item::ShowText,
    Item::SortName,
    Item::SortType,
    Item::SortSize,
    Item::SortDate,
    Item::NoSort,
    Item::SizeToFit,
    Item::SetBackground,
    Item::InstallIcon,
    Item::InstallApp,
    Item::InstallDevices,
    Item::RemoveIcon,
    Item::SetPrefs,
    Item::DeskConfig,
    Item::ChangeRez,
    Item::ReadInf,
    Item::SaveDesktop,
];

/// One drop-down entry: a separator (a run of dashes) or a selectable item.
/// Both labels are **NUL-terminated** (the G_STRING renderer reads a C string).
enum Entry {
    /// A disabled separator line (its dash-run label, box-width sized).
    Sep(&'static [u8]),
    /// A selectable item and its label.
    Act(Item, &'static [u8]),
    /// A **desk-accessory placeholder** under *Desk* — EmuTOS's `"1"`…`"6"`
    /// entries of `ADMENU` (`desk/desk_rsc.c:345-407`). Built like an
    /// [`Entry::Act`] with no [`Item`]: the AES's `menu_fixup` overwrites its
    /// `ob_spec` with a registered accessory's title pointer and unlinks the
    /// ones it does not need (`aes/gemmnlib.c:89-125`), and a click on it is
    /// routed to the accessory as `AC_OPEN`, never to the desktop as
    /// `MN_SELECTED`. The label text is only what shows if `menu_fixup` never
    /// runs.
    Acc(&'static [u8]),
}

use Entry::{Acc, Act, Sep};

/// One top-level menu: its title and its drop-down entries. Labels are padded
/// to a common width for a rectangular drop-down (EmuTOS `copy_menu_items`).
struct Def {
    title: Title,
    label: &'static [u8],
    entries: &'static [Entry],
}

/// The four desktop menus (`_rebuild-plan.md` §8 item set). All labels are
/// NUL-terminated; separator dash-runs match their menu's item width.
const MENUS: [Def; 4] = [
    // The *Desk* drop-down carries the EmuTOS `ADMENU` reservation: the About
    // item, a separator, and **six blank placeholders** (`desk_rsc.c:345-407`,
    // objects 9…16, whose `ob_spec` strings are literally `"1"`…`"6"`).
    // `menu_fixup` (`aes/gemmnlib.c:89-125`) relinks this box from scratch on
    // every `menu_bar(…, 1)` and every `menu_register`, linking `1` child with
    // no accessory registered (About only — the separator disappears too) and
    // `2 + n` with `n` of them, writing each registered title pointer into the
    // `n`-th placeholder's `ob_spec`. Without the placeholders the desktop's
    // tree would have nothing for the accessories to be spliced into.
    Def {
        title: Title::Desk,
        label: b" Desk \0",
        entries: &[
            Act(Item::About, b"  Desktop info... \0"),
            Sep(b"------------------\0"),
            Acc(b"  1               \0"),
            Acc(b"  2               \0"),
            Acc(b"  3               \0"),
            Acc(b"  4               \0"),
            Acc(b"  5               \0"),
            Acc(b"  6               \0"),
        ],
    },
    Def {
        title: Title::File,
        label: b" File \0",
        entries: &[
            Act(Item::Open, b"  Open             \0"),
            Act(Item::InfoRename, b"  Info/Rename...   \0"),
            Act(Item::Search, b"  Search...        \0"),
            Sep(b"-------------------\0"),
            Act(Item::NewFolder, b"  New folder...    \0"),
            Act(Item::CloseFolder, b"  Close folder     \0"),
            Act(Item::CloseWindow, b"  Close top window \0"),
            Act(Item::Cycle, b"  Cycle windows    \0"),
            Act(Item::SelectAll, b"  Select all items \0"),
            Act(Item::SetMask, b"  Set file mask... \0"),
            Sep(b"-------------------\0"),
            Act(Item::Delete, b"  Delete...        \0"),
            Sep(b"-------------------\0"),
            Act(Item::ExecEmuCon, b"  Execute EmuCON   \0"),
            Act(Item::Shutdown, b"  Shutdown         \0"),
        ],
    },
    Def {
        title: Title::View,
        label: b" View \0",
        entries: &[
            Act(Item::ShowIcons, b"  Show as icons    \0"),
            Act(Item::ShowText, b"  Show as text     \0"),
            Sep(b"-------------------\0"),
            Act(Item::SortName, b"  Sort by name     \0"),
            Act(Item::SortType, b"  Sort by type     \0"),
            Act(Item::SortSize, b"  Sort by size     \0"),
            Act(Item::SortDate, b"  Sort by date     \0"),
            Act(Item::NoSort, b"  No sort          \0"),
            Sep(b"-------------------\0"),
            Act(Item::SizeToFit, b"  Size to fit      \0"),
            // `SEP_VW2` (45) + `BACKGRND` (46) close the View box: object 46 is
            // the **last child** of the View drop-down (`desk_rsc.c:446,
            // :473-483`), and the Options box does not start until `IICNITEM`
            // (48) — `desk_rsc.h:35-48`. DESK21 moved the entry here from the
            // head of Options; `shortcut_mapping[]` never had to move, it has
            // carried `BACKGRND` in the View group all along
            // (`deskmain.c:211-217`).
            Sep(b"-------------------\0"),
            Act(Item::SetBackground, b"  Set background...\0"),
        ],
    },
    Def {
        title: Title::Options,
        label: b" Options \0",
        entries: &[
            Act(Item::InstallIcon, b"  Install icon...         \0"),
            Act(Item::InstallApp, b"  Install application...  \0"),
            Act(Item::InstallDevices, b"  Install devices         \0"),
            Act(Item::RemoveIcon, b"  Remove desktop icon     \0"),
            Sep(b"--------------------------\0"),
            Act(Item::SetPrefs, b"  Set preferences...      \0"),
            Act(Item::DeskConfig, b"  Desktop configuration...\0"),
            Act(Item::ChangeRez, b"  Change resolution...    \0"),
            Sep(b"--------------------------\0"),
            Act(Item::ReadInf, b"  Read .INF file...       \0"),
            Act(Item::SaveDesktop, b"  Save desktop...         \0"),
        ],
    },
];

/// Object-array capacity: 3 structural (ROOT/THEBAR/THEACTIVE) + 4 titles + 1
/// THEMENUS + per-menu (1 box + entries). The real total is ~51; 96 is head
/// room.
const MENU_CAP: usize = 96;

const EMPTY_OBJ: Object = Object {
    ob_next: NIL,
    ob_head: NIL,
    ob_tail: NIL,
    ob_type: 0,
    ob_flags: 0,
    ob_state: 0,
    ob_spec: 0,
    ob_x: 0,
    ob_y: 0,
    ob_width: 0,
    ob_height: 0,
};

/// The persistent menu tree + the semantic-index maps. The AES keeps a pointer
/// to `obj` for drop-down tracking, so it must outlive `menu_bar`. Single boot
/// CPU only (like the rest of the desktop).
struct MenuTree {
    /// The `OBJECT` array handed to `menu_bar`.
    obj: UnsafeCell<[Object; MENU_CAP]>,
    /// The drop-down entry strings, **in RAM** — EmuTOS `copy_menu_items`
    /// (`deskmain.c:1755-1813`) moves them out of ROM for exactly one reason:
    /// [`install_shortcuts`] writes `'^'` + the shortcut key into the padding
    /// at their right edge. One NUL-terminated string per object index.
    labels: UnsafeCell<[[u8; LABEL_CAP]; MENU_CAP]>,
    /// Object index of each [`Title`] (`Title` order).
    title_obj: UnsafeCell<[i16; 4]>,
    /// Object index of each [`Item`] (`Item` discriminant order).
    item_obj: UnsafeCell<[i16; N_ITEMS]>,
    /// Object index → [`Item`] discriminant (`-1` = not a selectable item).
    obj_item: UnsafeCell<[i16; MENU_CAP]>,
    /// `menu_shortcuts[]` (`deskmain.c:119`) — the shortcut letter of each of
    /// the [`NUM_SHORTCUTS`] slots, `0` for none. Seeded from
    /// [`DEFAULT_SHORTCUTS`], overwritten by the `#K` line
    /// ([`super::inf`]), edited by the Desktop-configuration dialog.
    shortcuts: UnsafeCell<[u8; NUM_SHORTCUTS]>,
    built: UnsafeCell<bool>,
}
// SAFETY: only touched from the single-threaded desktop on the boot CPU.
unsafe impl Sync for MenuTree {}
static MENU: MenuTree = MenuTree {
    obj: UnsafeCell::new([EMPTY_OBJ; MENU_CAP]),
    labels: UnsafeCell::new([[0; LABEL_CAP]; MENU_CAP]),
    title_obj: UnsafeCell::new([0; 4]),
    item_obj: UnsafeCell::new([0; N_ITEMS]),
    obj_item: UnsafeCell::new([-1; MENU_CAP]),
    shortcuts: UnsafeCell::new(DEFAULT_SHORTCUTS),
    built: UnsafeCell::new(false),
};

/// Length of a NUL-terminated label in characters (excludes the terminator).
fn label_len(s: &[u8]) -> i16 {
    s.iter().position(|&b| b == 0).unwrap_or(s.len()) as i16
}

/// The measured pixel geometry of the four titles + their drop-down boxes.
struct MenuGeom {
    /// Left edge of `THEACTIVE` on the bar — the RSC's 2 character cells
    /// (`desk_rsc.c:255-259`, object 2 `{2, 0, 28, 769}`), which
    /// `adjust_menu` keeps unless the bar overflows (`deskmain.c:1584-1589`).
    /// Not 0: "the menu bar cannot start at character offset 0 because, in
    /// that case, the outline of the Desk dropdown box will not be drawn
    /// properly" (`deskmain.c:1537-1541`) — the boxes carry an *outside*
    /// border (see [`build_menu`]), and a box at x = 0 would put it off-screen.
    active_x: i16,
    /// Width of `THEACTIVE` = the sum of the title widths, `mbar->ob_width = x`
    /// (`deskmain.c:1528-1534`) — **not** the screen width. `menu_bar` takes
    /// this rectangle as the Control Manager's `gl_ctwait` (`gemmnlib.c:717`),
    /// so the bar row right of the last title stays the application's (A102).
    active_w: i16,
    /// Left edge of each title, relative to `THEACTIVE`.
    title_x: [i16; 4],
    /// Width of each title cell.
    title_w: [i16; 4],
    /// Left edge of each drop-down box.
    box_x: [i16; 4],
    /// Width of each drop-down box.
    box_w: [i16; 4],
}

/// `adjust_menu` (EmuTOS `deskmain.c:1511-1641`) — measure every title and
/// drop-down and compute their pixel geometry, in character cells as EmuTOS
/// does (it runs before `rsrc_obfix`, `:1895-1900`, so a unit is
/// `CHAR_WIDTH` = 8 px, `:1509`): each title is exactly `strlen` cells wide
/// and laid left-to-right (`title->ob_x = x; title->ob_width = n; x += n`,
/// `:1528-1533` — no padding: the blank in `" Desk "` is the gap),
/// `THEACTIVE` at the RSC's 2-cell origin and as wide as the titles' sum
/// (`mbar->ob_width = x`, `:1534`), each drop-down box `maxlen + 1` cells
/// wide — the widest entry (`:1608-1614`) plus the `m++` of the separator
/// loop (`:1620`) — with the Desk box seeded at `MIN_DESKMENU_WIDTH - 1`
/// (`:1602`, i.e. never under 20 cells; the 2-cell check column lives in
/// the item strings' leading blanks, not code), and every box under its
/// title — `THEACTIVE`'s origin plus the title's offset (`:1617`) —
/// **shifted left if it would run off the right screen edge** (`:1626-1631`,
/// the box then loses one pixel of width so its outside border lands on the
/// last column).
///
/// EmuTOS additionally shrinks/truncates an over-wide title bar (its
/// French-desktop half-character hack); NewTOS's menu text fits ST-High width,
/// so that path is defensive here — if the titles would overflow `sw`, they are
/// clamped so the last title still ends at the screen edge rather than off it,
/// preserving the measure-then-fit strategy (`deskmain.md`) without the m68k
/// sub-pixel packing trick.
fn adjust_menu(sw: i16) -> MenuGeom {
    let mut g = MenuGeom {
        active_x: 2 * CHAR_W,
        active_w: 0,
        title_x: [0; 4],
        title_w: [0; 4],
        box_x: [0; 4],
        box_w: [0; 4],
    };
    let mut tx = 0i16;
    for (ti, def) in MENUS.iter().enumerate() {
        // `title->ob_width = n` = `strlen(ob_spec)` cells (`deskmain.c:1531`).
        let tw = label_len(def.label) * CHAR_W;
        g.title_x[ti] = tx;
        g.title_w[ti] = tw;
        tx += tw;

        // Drop-down width: `m` = the widest entry, the Desk menu's search
        // seeded at `MIN_DESKMENU_WIDTH - 1` (`:1602-1603`, `m = 0` for the
        // others), then `m++` (`:1620`) — `(maxlen + 1)` cells, `ob_width`
        // of the box and of every entry (`:1633-1635`). The 2-cell check
        // column is not added by code — it lives in the two leading spaces
        // every item label carries (EmuTOS menu-string convention, e.g.
        // "  Open"), so the object renderer draws the text at `ob_x` and the
        // check mark sits over the leading blank (see `object::just_draw` D1).
        //
        // Two details of the source that the measurement depends on. (1) The
        // strings `adjust_menu` measures are the **RAM** ones
        // `copy_menu_items` has already padded to `widest + 1 + SHORTCUT_SIZE`
        // minus the NUL (`deskmain.c:1781,1799-1805`), so the shortcut reserve
        // is part of `maxlen` — reproduced by the `+ SHORTCUT_SIZE` below.
        // (2) A **separator** is not in that measurement: `copy_menu_items`
        // skips it (`:1773`, `if (*q != '-')`), so its `ob_spec` still points
        // at the resource's one-character `"-"` when `adjust_menu` runs and it
        // can never be the widest entry. [`build_menu`] then fills the
        // separator with `m` dashes, which is EmuTOS's
        // `separator+MAXLEN_SEPARATOR-m` (`:1623`).
        let seed = if ti == 0 { MIN_DESKMENU_WIDTH - 1 } else { 0 };
        let widest = def
            .entries
            .iter()
            .filter(|e| matches!(e, Act(_, _)))
            .map(|e| label_len(entry_label(e)))
            .fold(0, i16::max);
        let m = seed.max(widest + SHORTCUT_SIZE) + 1;
        g.box_w[ti] = m * CHAR_W;
    }
    // `mbar->ob_width = x` (`deskmain.c:1534`): THEACTIVE is exactly as wide
    // as its titles.
    g.active_w = tx;
    // Defensive over-wide-bar clamp: keep the last title on-screen.
    if tx > sw {
        let overflow = tx - sw;
        for ti in 0..4 {
            g.title_x[ti] = (g.title_x[ti] - overflow).max(0);
        }
    }
    // "if the menu bar is too far to the right, move it to the left, but only
    // as far as char posn 1" (`deskmain.c:1584-1589`).
    if g.active_x + tx > sw {
        g.active_x = CHAR_W;
    }
    // Drop-down x under its title — `dropbox->ob_x = mbar->ob_x + title->ob_x`
    // (`deskmain.c:1617`, THEACTIVE's origin plus the title's offset in it).
    // "make sure the menu is not too far on the right of the screen"
    // (`:1626-1631`): a box that would reach the screen's right edge
    // (`ob_x + m >= width`, in cells) is moved to `width - m` and made
    // `(m - 1) | ((CHAR_WIDTH - 1) << 8)` wide — `m` cells less one pixel,
    // so its outside border (`th = -1`) lands on the last screen column.
    for ti in 0..4 {
        let mut bx = g.active_x + g.title_x[ti];
        if bx + g.box_w[ti] >= sw {
            bx = (sw - g.box_w[ti]).max(0);
            g.box_w[ti] -= 1;
        }
        g.box_x[ti] = bx;
    }
    g
}

/// Build one `OBJECT` from its fields (menu-tree helper).
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

/// Build the ADMENU tree (ROOT / THEBAR / THEACTIVE / titles + per-title
/// drop-down boxes and items) into the persistent static, recording the
/// title/item object-index maps. Idempotent. Returns the tree base pointer.
fn build_menu() -> *mut Object {
    // SAFETY: single-CPU AES; the tree is built once and only read afterwards.
    unsafe {
        let o = &mut *MENU.obj.get();
        if *MENU.built.get() {
            return o.as_mut_ptr();
        }
        let title_obj = &mut *MENU.title_obj.get();
        let item_obj = &mut *MENU.item_obj.get();
        let obj_item = &mut *MENU.obj_item.get();
        let labels = &mut *MENU.labels.get();

        let (sw, sh) = screen_dims();
        let geom = adjust_menu(sw);
        let nt = MENUS.len();
        let first_title = 3usize;
        let themenus = first_title + nt; // index of the THEMENUS box container

        // ROOT / THEBAR / THEACTIVE. `THEBAR` is [`BAR_BOX_H`] (18) tall —
        // the RSC's `{0, 0, 80, 513}` (`desk_rsc.c:249-253`) — one row above
        // the separator `mn_bar` draws on `gl_hbox - 1` (`gemmnlib.c:722`).
        o[0] = mk(G_IBOX, 0, 0, 0, 0, sw, sh, -1, 1, themenus as i16);
        o[1] = mk(G_BOX, 0, 0, 0, 0, sw, BAR_BOX_H, themenus as i16, 2, 2);
        // THEACTIVE: the RSC's `{2, 0, 28, 769}` (`desk_rsc.c:255-259`) with
        // `adjust_menu`'s width — `mbar->ob_width = x`, the titles' sum
        // (`deskmain.c:1534`). Its rectangle is what `menu_bar` hands the
        // Control Manager as `gl_ctwait` (`gemmnlib.c:717`).
        o[2] = mk(G_IBOX, 0, 0, geom.active_x, 0, geom.active_w, MENU_H, 1, first_title as i16, (themenus - 1) as i16);

        // Titles, left to right (adjust_menu's measured geometry).
        for (ti, def) in MENUS.iter().enumerate() {
            let last = ti == nt - 1;
            title_obj[def.title as usize] = (first_title + ti) as i16;
            o[first_title + ti] = mk(
                G_TITLE,
                if last { LASTOB } else { 0 },
                def.label.as_ptr() as u64,
                geom.title_x[ti],
                0,
                geom.title_w[ti],
                MENU_H,
                if last { 2 } else { (first_title + ti + 1) as i16 },
                -1,
                -1,
            );
        }

        // Per-title drop-down boxes + their items.
        let mut k = themenus + 1; // first box index
        let themenus_first_child = k;
        let mut last_box = themenus + 1;
        for (ti, def) in MENUS.iter().enumerate() {
            let bx_x = geom.box_x[ti];
            let iw = geom.box_w[ti];
            let n_items = def.entries.len();
            // The box is `n` character cells tall — the RSC's `{…, 0, 19, 16}`
            // for the 16-entry File box (`desk_rsc.c:345-349`), one
            // [`ENTRY_H`] per entry.
            let bh = n_items as i16 * ENTRY_H;
            let bi = k;
            last_box = bi;
            k += 1;
            let first_item = k;
            // `copy_menu_items` (`deskmain.c:1755-1813`) works in the box's
            // width in **character cells**, which is what `ob_width` carries
            // here in pixels — recovered with the same ceiling division
            // `install_shortcuts` uses (`:1845`).
            let cells = ((iw + CHAR_W - 1) / CHAR_W).clamp(1, LABEL_CAP as i16 - 1) as usize;
            for (ii, entry) in def.entries.iter().enumerate() {
                let last_item = ii == n_items - 1;
                let last_overall = (ti == nt - 1) && last_item;
                let mut flags = 0u16;
                if last_overall {
                    flags |= LASTOB;
                }
                // The entry's string, in RAM. An action entry is the label
                // padded with blanks to `cells - 1` characters — EmuTOS's
                // "copy string, pad with spaces" up to `p+width-1`, then the
                // NUL (`:1799-1805`) — which leaves [`SHORTCUT_SIZE`]
                // characters of blank at the right edge for
                // [`install_shortcuts`]. A separator is `cells` dashes, which
                // is `separator + MAXLEN_SEPARATOR - m` (`:1623`).
                let dst = &mut labels[k];
                let src = entry_label(entry);
                match entry {
                    Sep(_) => {
                        dst[..cells].fill(b'-');
                        dst[cells] = 0;
                    }
                    Act(_, _) | Acc(_) => {
                        let n = (label_len(src) as usize).min(cells - 1);
                        dst[..n].copy_from_slice(&src[..n]);
                        dst[n..cells - 1].fill(b' ');
                        dst[cells - 1] = 0;
                    }
                }
                // `{0, row, 20, 1}` (`desk_rsc.c:297-341`) after `rsrc_obfix`:
                // the entry spans the box's width at `row · gl_hchar`, one
                // character cell tall ([`ENTRY_H`]).
                o[k] = mk(
                    G_STRING,
                    flags,
                    labels[k].as_ptr() as u64,
                    0,
                    ii as i16 * ENTRY_H,
                    iw,
                    ENTRY_H,
                    if last_item { bi as i16 } else { (k + 1) as i16 },
                    -1,
                    -1,
                );
                match entry {
                    Sep(_) => o[k].ob_state |= DISABLED, // DISABLED is an ob_state bit
                    Act(item, _) => {
                        item_obj[*item as usize] = k as i16;
                        obj_item[k] = *item as i16;
                    }
                    // A desk-accessory placeholder carries no desktop `Item`:
                    // a click on it never reaches `hndl_menu` (the AES routes
                    // it to the accessory as `AC_OPEN`), so `obj_item[k]` stays
                    // at its `-1` default.
                    Acc(_) => {}
                }
                k += 1;
            }
            // The box itself: `ob_spec = 0xFF1100` as every ADMENU drop-down
            // box in the RSC (`desk_rsc.c:291-295`, object 8 `16716032L`):
            // thickness byte 0xFF = `th = -1`, a 1-px border drawn **outside**
            // the box rectangle; colour word 0x1100 = border BLACK, text BLACK,
            // IP_HOLLOW interior — which `gr_rect` paints opaque WHITE
            // (`gemgraf.c:547-559`). The entries fill the box rectangle from
            // its origin (`desk_rsc.c:297-301`: `{0, 0, 20, 1}` …), so the
            // item XOR of a hover (`ob_change`, `gemoblib.c:1295-1304`) never
            // reaches the border (A100).
            let next_box = if ti == nt - 1 { themenus as i16 } else { k as i16 };
            o[bi] = mk(
                G_BOX,
                0,
                0x00FF_1100,
                bx_x,
                MENU_H,
                iw,
                bh,
                next_box,
                first_item as i16,
                (k - 1) as i16,
            );
        }
        o[themenus] = mk(
            G_IBOX,
            0,
            0,
            0,
            0,
            sw,
            sh,
            0,
            themenus_first_child as i16,
            last_box as i16,
        );

        *MENU.built.get() = true;
        o.as_mut_ptr()
    }
}

/// The NUL-terminated label of a drop-down entry (the dash-run for a
/// separator, the item text for an action).
fn entry_label(e: &Entry) -> &'static [u8] {
    match *e {
        Sep(s) | Act(_, s) | Acc(s) => s,
    }
}

/// The screen dimensions (for the ROOT/THEMENUS extents). `menu_bar` fixes up
/// `THEBAR`'s width, so only a `>= screen` bound is needed here.
fn screen_dims() -> (i16, i16) {
    let r = super::screen_rect();
    (r.w, r.h)
}

/// Install the desktop menu bar (`menu_bar`, show = 1). Builds the tree on the
/// first call.
///
/// [`install_shortcuts`] runs here for the same reason EmuTOS's `deskmain`
/// runs it at `:2023`: the shortcut letters have to reach the item strings
/// after `app_start` has read the `NEWDESK.INF` `#K` line into
/// `menu_shortcuts[]` and after `copy_menu_items` has put those strings in RAM.
/// The desktop's boot path calls this once the INF is applied
/// (`super::run`), so both are true here; the call is idempotent, so the
/// whole-screen-recovery caller ([`super::redraw`]) re-asserting it costs
/// nothing.
pub fn install_menu_bar() {
    let tree = build_menu();
    install_shortcuts();
    super::menu_bar(tree as u64, 1);
}

/// The menu tree base pointer (built if necessary) — `hndl_menu` and the
/// shortcut path pass it to `menu_tnormal`.
pub fn tree_ptr() -> u64 {
    build_menu() as u64
}

/// The object index of [`Title`] `t` (`msg[3]` value the AES sends, and the
/// `menu_tnormal` argument).
pub fn title_obj(t: Title) -> i16 {
    build_menu();
    // SAFETY: single-CPU; built above.
    let map = unsafe { &*MENU.title_obj.get() };
    map[t as usize]
}

/// The object index of [`Item`] `item` (`msg[4]` value for a `MN_SELECTED`).
fn item_obj(item: Item) -> i16 {
    build_menu();
    // SAFETY: single-CPU; built above.
    let map = unsafe { &*MENU.item_obj.get() };
    map[item as usize]
}

/// Map a drop-down object index (`MN_SELECTED`'s `msg[4]`) back to its
/// [`Item`]; `None` for a separator or non-item object.
pub fn item_of_obj(obj: i16) -> Option<Item> {
    build_menu();
    if obj < 0 || obj as usize >= MENU_CAP {
        return None;
    }
    // SAFETY: single-CPU; built above.
    let map = unsafe { &*MENU.obj_item.get() };
    let disc = map[obj as usize];
    (disc >= 0).then(|| ALL_ITEMS[disc as usize])
}

/// The top-level menu an [`Item`] belongs to — routes `hndl_menu` dispatch.
pub fn item_title(item: Item) -> Title {
    match item {
        Item::About => Title::Desk,
        Item::Open
        | Item::InfoRename
        | Item::Search
        | Item::NewFolder
        | Item::CloseFolder
        | Item::CloseWindow
        | Item::Cycle
        | Item::SelectAll
        | Item::SetMask
        | Item::Delete
        | Item::ExecEmuCon
        | Item::Shutdown => Title::File,
        Item::ShowIcons
        | Item::ShowText
        | Item::SortName
        | Item::SortType
        | Item::SortSize
        | Item::SortDate
        | Item::NoSort
        | Item::SizeToFit
        | Item::SetBackground => Title::View,
        _ => Title::Options,
    }
}

/// Whether [`Item`] `item` is currently disabled (its `ob_state` `DISABLED`
/// bit) — the shortcut lookup treats a disabled item as no match.
fn item_disabled(item: Item) -> bool {
    let obj = item_obj(item);
    // SAFETY: single-CPU; valid index.
    let o = unsafe { &(*MENU.obj.get())[obj as usize] };
    o.ob_state & DISABLED != 0
}

/// `menu_ienable(tree, item, enable)` — EmuTOS `men_update`/`men_list`
/// (`deskmain.c:453-457,534-585`) go through the AES binding for every item
/// they gate, and so does this.
fn set_disabled(item: Item, disabled: bool) {
    super::menu_ienable(tree_ptr(), item_obj(item), !disabled);
}

/// `menu_icheck(tree, item, checked)` — the AES binding EmuTOS's `men_update`
/// uses for the View/Sort radio marks (`deskmain.c:798-820`).
fn set_checked(item: Item, checked: bool) {
    super::menu_icheck(tree_ptr(), item_obj(item), checked);
}

// ── `menu_shortcuts[]`: the editable shortcut array (`deskmain.c`) ──────────
//
// EmuTOS keeps the shortcut letters in a 33-byte array, *not* in the menu
// strings: the strings only ever carry a rendering of it. Four functions move
// the data between the two, and the Desktop-configuration dialog (DESK05,
// `deskinf.c` `inf_conf`) edits the array between them:
//
//   deskmain:2013  memcpy(menu_shortcuts, default_shortcuts, NUM_SHORTCUTS)
//   app_start      the `#K` line overwrites all 33 (`deskapp.c:967-974`)
//   deskmain:2023  install_shortcuts()   array → strings
//   inf_conf       save_shortcut()/DCMNUCLR edit the array
//       on OK      install_shortcuts()   array → strings
//       otherwise  recover_shortcuts()   strings → array (undo)
//
// The array is the only thing `lookup_ascii_shortcut` reads, so an edit takes
// effect on the keyboard whether or not the menu has been redrawn.

/// The shortcut letter of slot `n`, `0` for none (`menu_shortcuts[n]`).
pub(super) fn shortcut_key(n: usize) -> u8 {
    // SAFETY: single-CPU desktop.
    unsafe { (*MENU.shortcuts.get()).get(n).copied().unwrap_or(0) }
}

/// Write the shortcut letter of slot `n` (`menu_shortcuts[n] = key`).
pub(super) fn set_shortcut_key(n: usize, key: u8) {
    // SAFETY: single-CPU desktop.
    unsafe {
        if let Some(slot) = (*MENU.shortcuts.get()).get_mut(n) {
            *slot = key;
        }
    }
}

/// The whole array — read by `app_save` for the `#K` line.
pub(super) fn shortcuts() -> [u8; NUM_SHORTCUTS] {
    // SAFETY: single-CPU desktop.
    unsafe { *MENU.shortcuts.get() }
}

/// Replace the whole array — `app_start`'s `case 'K'` (`deskapp.c:967-974`)
/// and `DCMNUCLR`'s `memset(menu_shortcuts, 0x00, NUM_SHORTCUTS)`
/// (`deskinf.c:1220`).
pub(super) fn set_shortcuts(v: [u8; NUM_SHORTCUTS]) {
    // SAFETY: single-CPU desktop.
    unsafe { *MENU.shortcuts.get() = v };
}

/// The [`Item`] shortcut slot `n` belongs to, `None` for a slot whose menu item
/// this build does not have (`shortcut_mapping[n] == 0`).
pub(super) fn shortcut_item(n: usize) -> Option<Item> {
    SHORTCUT_MAPPING.get(n).copied().flatten()
}

/// The menu item text of shortcut slot `n`, blank-padded into `dst`: the live
/// drop-down string **minus its two leading blanks and minus the `' ^X'`
/// reserve at its right edge**. EmuTOS `init_conf_shortcuts`' middle three
/// lines (`deskinf.c:1013-1016`): `strcpy(G.g_work, obj->ob_spec+2)` then
/// `memset(G.g_work+strlen(G.g_work)-SHORTCUT_SIZE, ' ', 40)` — "insert menu
/// item text, without shortcut & with enough spaces to fill text field".
/// A slot with no menu item leaves `dst` blank.
pub(super) fn shortcut_item_text(n: usize, dst: &mut [u8]) {
    dst.fill(b' ');
    let Some(item) = shortcut_item(n) else {
        return;
    };
    let obj = item_obj(item) as usize;
    // SAFETY: single-CPU; `item_obj` built the tree and returns a valid index.
    let s = unsafe { &(*MENU.labels.get())[obj] };
    let len = s.iter().position(|&b| b == 0).unwrap_or(0);
    let keep = len.saturating_sub(2 + SHORTCUT_SIZE as usize).min(dst.len());
    dst[..keep].copy_from_slice(&s[2..2 + keep]);
}

/// The index of the `'^'` byte inside a drop-down string of `cells` characters
/// — EmuTOS's `(ob_width + CHAR_WIDTH - 1) / CHAR_WIDTH - SHORTCUT_SIZE`
/// (`deskmain.c:1845`, `deskinf.c:1084`), with `ob_width` already in pixels.
fn shortcut_offset(obj: usize) -> Option<usize> {
    // SAFETY: single-CPU; the tree is built by every caller's `item_obj`.
    let w = unsafe { (*MENU.obj.get())[obj].ob_width };
    let cells = (w + CHAR_W - 1) / CHAR_W;
    let off = cells - SHORTCUT_SIZE;
    // The string is `cells - 1` characters long, so `off` and `off + 1` are its
    // last two; a menu whose box was widened past its strings (the
    // `MIN_DESKMENU_WIDTH` seed) has no room and is skipped rather than
    // overrun.
    (off >= 0 && (off as usize) + 1 < LABEL_CAP).then_some(off as usize)
}

/// `install_shortcuts` (`deskmain.c:1825-1858`) — render `menu_shortcuts[]`
/// into the drop-down strings: `'^'` + the letter in the reserve at the right
/// edge, or two blanks where there is no shortcut. A slot whose menu item this
/// build does not have is **cleared in the array** on the way past, "to ensure
/// that the shortcut array matches the displayed menu items" (`:1833-1839`) —
/// which is also what keeps a foreign `#K` line from re-saving a shortcut for
/// an item that is not there.
pub(super) fn install_shortcuts() {
    build_menu();
    for n in 0..NUM_SHORTCUTS {
        let Some(item) = shortcut_item(n) else {
            set_shortcut_key(n, 0);
            continue;
        };
        let obj = item_obj(item) as usize;
        let Some(off) = shortcut_offset(obj) else {
            continue;
        };
        let c = shortcut_key(n);
        // SAFETY: single-CPU; `off + 1 < LABEL_CAP` checked above.
        let s = unsafe { &mut (*MENU.labels.get())[obj] };
        if c != 0 {
            s[off] = b'^';
            s[off + 1] = c;
        } else {
            s[off] = b' ';
            s[off + 1] = b' ';
        }
    }
}

/// `recover_shortcuts` (`deskinf.c:1062-1090`) — the inverse: rebuild
/// `menu_shortcuts[]` from the strings as they currently stand. The
/// Desktop-configuration dialog calls it on every exit that is not OK, which is
/// how it undoes edits the user made and then cancelled.
pub(super) fn recover_shortcuts() {
    build_menu();
    for n in 0..NUM_SHORTCUTS {
        let mut c = 0u8;
        if let Some(item) = shortcut_item(n) {
            let obj = item_obj(item) as usize;
            if let Some(off) = shortcut_offset(obj) {
                // SAFETY: single-CPU; `off + 1 < LABEL_CAP` checked above.
                let s = unsafe { &(*MENU.labels.get())[obj] };
                if s[off] == b'^' {
                    c = s[off + 1];
                }
            }
        }
        set_shortcut_key(n, c);
    }
}

/// `lookup_ascii_shortcut` (EmuTOS `deskmain.c:1169-1198`) — resolve a
/// **control** character (`ascii < 0x20`) to the menu item its
/// `menu_shortcuts[]` entry names. `ascii | 0x40` is the displayed uppercase
/// letter, matched against the array in slot order. A currently **disabled**
/// item is treated as no match and stops the scan (EmuTOS `break`, `:1187`).
/// Returns `None` for a non-control character or no/disabled match.
pub fn lookup_ascii_shortcut(ascii: u8) -> Option<Item> {
    if ascii >= 0x20 {
        return None;
    }
    let letter = ascii | 0x40;
    for n in 0..NUM_SHORTCUTS {
        let Some(item) = shortcut_item(n) else {
            continue; // `if (!n) continue;` (`:1181-1182`)
        };
        if shortcut_key(n) == letter {
            return (!item_disabled(item)).then_some(item);
        }
    }
    None
}

/// `men_update` (EmuTOS `deskmain.c:464-609`) — recompute every menu item's
/// enabled/disabled + checked state from the current selection count and open
/// window count. **The D6 fix.**
///
/// Task #8 scope: the Desk/View items and the File Open/Close/Cycle items are
/// live; every file-modifying / install / preference / resolution item is
/// **present but disabled** until its Task-#9 handler exists — so no menu item
/// is dead-clickable and the shortcut lookup skips them (see the report for the
/// exact disabled set).
pub fn men_update() {
    let (nsel, nwin, napp, (ndesk, ntrash), view, sort, fit) = global::with(|g| {
        (
            g.count_selected(g.g_croot),
            g.win_count(),
            g.count_selected_apps(),
            g.count_selected_desk_icons(),
            g.view,
            g.sort,
            g.size_to_fit,
        )
    })
    .unwrap_or((0, 0, 0, (0, 0), View::Icon, Sort::Name, true));

    for &item in &ALL_ITEMS {
        set_disabled(item, !item_enabled(item, nsel, nwin, napp, ndesk, ntrash));
    }

    // View / sort / size-to-fit check marks.
    set_checked(Item::ShowIcons, view == View::Icon);
    set_checked(Item::ShowText, view == View::Text);
    set_checked(Item::SortName, sort == Sort::Name);
    set_checked(Item::SortType, sort == Sort::Type);
    set_checked(Item::SortSize, sort == Sort::Size);
    set_checked(Item::SortDate, sort == Sort::Date);
    set_checked(Item::NoSort, sort == Sort::NoSort);
    set_checked(Item::SizeToFit, fit);
}

/// Whether `item` is enabled for the current `nsel`/`nwin`/`napp`/`ndesk`/
/// `ntrash` — EmuTOS `men_update`'s four counts (`deskmain.c:496-530`):
/// `napp` selected native programs, `ndesk` selected desktop icons, `ntrash`
/// of those that are a trash can or a printer.
fn item_enabled(item: Item, nsel: usize, nwin: usize, napp: usize, ndesk: usize, ntrash: usize) -> bool {
    match item {
        // Always available.
        Item::About
        | Item::ShowIcons
        | Item::ShowText
        | Item::SortName
        | Item::SortType
        | Item::SortSize
        | Item::SortDate
        | Item::NoSort
        | Item::SizeToFit => true,
        // File — live, gated on selection / window count (EmuTOS `men_update`
        // `ILL_NOSEL`/`ILL_MULTSEL`/`ILL_NOWIN` + BTOPITEM ≥ 2 windows).
        Item::Open => nsel == 1,
        Item::CloseFolder | Item::CloseWindow => nwin >= 1,
        Item::Cycle => nwin >= 2,
        // Task-#9 file ops: Delete needs a selection; Info/Rename a single
        // item; New folder / Select all / Set mask an open window (EmuTOS
        // `ILL_NOSEL`/`ILL_NOWIN` — `MASKITEM` is in `ILL_NOWIN`).
        //
        // `DELTITEM` carries a second gate on top of `ILL_NOSEL`: "disable
        // 'Delete' iff either trash or printer is selected"
        // (`deskmain.c:533-535`). Neither can be thrown away, and EmuTOS says
        // so by greying the entry rather than by refusing later.
        Item::Delete => nsel >= 1 && ntrash == 0,
        Item::InfoRename => nsel == 1,
        Item::NewFolder | Item::SelectAll | Item::SetMask => nwin >= 1,
        // Search (`CONF_WITH_SEARCH`): EmuTOS disables it only when there is
        // neither an open window nor a selection; NewTOS's Search scans the
        // current window, so it needs an open window (`deskmain.c:548-556`).
        Item::Search => nwin >= 1,
        // Task-#14 deskinf/NEWDESK.INF ops: always available (EmuTOS
        // `CONF_WITH_BACKGROUNDS`/`CONF_WITH_READ_INF` + the always-live Save).
        // `BACKGRND` sits in the *View* menu since DESK21; its gating did not
        // move with it, because EmuTOS names it in no `ILL_*` table and in no
        // `menu_ienable` call either.
        // `PREFITEM` likewise: EmuTOS lists it in none of `ILL_NOSEL` /
        // `ILL_MULTSEL` / `ILL_NOWIN` (`deskmain.c:143-160`), so Set
        // preferences is enabled whatever is selected (DESK02).
        //
        // `CONFITEM` is in the same position (DESK05): it appears in no `ILL_*`
        // table and in no `menu_ienable` call, and `CONF_WITH_DESKTOP_CONFIG`
        // defaults to 1 (`include/config.h:1774-1776`) on our
        // Falcon030/TOS4 profile, so *Desktop configuration…* is always
        // available — its dialog edits globals, not a selection.
        Item::SetBackground | Item::ReadInf | Item::SaveDesktop | Item::SetPrefs | Item::DeskConfig => true,
        // Install Application (deskins #20): enabled iff a native program is
        // selected (EmuTOS disables `IAPPITEM` when `napp == 0`,
        // `deskmain.c:536-538`).
        Item::InstallApp => napp >= 1,
        // Install devices / Install icon (DESK03/DESK04): EmuTOS's `men_update`
        // names neither `IDSKITEM` nor `IICNITEM` in any `ILL_*` table and
        // disables neither by hand (`deskmain.c:143-160`, `:530-545`), so both
        // are always available — *Install icon* with nothing selected is how a
        // brand-new icon is made (`ins_icon(0)`, `deskins.c:1240-1267`).
        Item::InstallDevices | Item::InstallIcon => true,
        // Remove desktop icon: "disable 'Remove desktop icon' iff no desktop
        // icons are selected" (`deskmain.c:540-542`). A disabled menu entry is
        // the whole of EmuTOS's answer to "what happens with no selection" —
        // `rmv_icon` is never reached, and there is no alert.
        Item::RemoveIcon => ndesk >= 1,
        // Shutdown: `menu_ienable(tree, QUITITEM, can_shutdown())`
        // (`deskmain.c:582`), and `can_shutdown()` is `FALSE` on everything
        // that is not a FireBee, an Amiga, a Lisa or a native-features host
        // (`bios/bios.c:754-770`) — so on our Falcon030/TOS4 profile EmuTOS
        // shows the entry permanently greyed, which is exactly this arm. A real
        // Pi 4 could power down (PM_RSTC/watchdog), but that would be an own
        // extension, not EmuTOS conformance (DESK07).
        Item::Shutdown => false,
        // Execute EmuCON: EmuTOS never disables `CLIITEM` — it appears in no
        // `ILL_*` table (`deskmain.c:143-160`) and in no `menu_ienable` call,
        // and `WITH_CLI` is on in the Falcon030/TOS4 profile. It needs neither
        // a selection nor a window: `do_filemenu`'s arm runs `pro_run(FALSE,
        // DEF_CONSOLE, …)` unconditionally and only the optional `shel_wdef`
        // step looks at the top window (`deskmain.c:750-770`). Live since
        // DESK06 (`main::do_execcli`).
        Item::ExecEmuCon => true,
        // The remaining Options items stay disabled (deskrez is
        // config.txt-driven, ADR-017 rev 2).
        _ => false,
    }
}
