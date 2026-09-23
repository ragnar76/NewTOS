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
    // Options.
    /// Set the background pattern (Task #9).
    SetBackground,
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
}

use Entry::{Act, Sep};

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
    Def {
        title: Title::Desk,
        label: b" Desk \0",
        entries: &[Act(Item::About, b"  Desktop info... \0"), Sep(b"------------------\0")],
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
            Act(Item::ShowIcons, b"  Show as icons \0"),
            Act(Item::ShowText, b"  Show as text  \0"),
            Sep(b"----------------\0"),
            Act(Item::SortName, b"  Sort by name  \0"),
            Act(Item::SortType, b"  Sort by type  \0"),
            Act(Item::SortSize, b"  Sort by size  \0"),
            Act(Item::SortDate, b"  Sort by date  \0"),
            Act(Item::NoSort, b"  No sort       \0"),
            Sep(b"----------------\0"),
            Act(Item::SizeToFit, b"  Size to fit   \0"),
        ],
    },
    Def {
        title: Title::Options,
        label: b" Options \0",
        entries: &[
            Act(Item::SetBackground, b"  Set background...       \0"),
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
    /// Object index of each [`Title`] (`Title` order).
    title_obj: UnsafeCell<[i16; 4]>,
    /// Object index of each [`Item`] (`Item` discriminant order).
    item_obj: UnsafeCell<[i16; N_ITEMS]>,
    /// Object index → [`Item`] discriminant (`-1` = not a selectable item).
    obj_item: UnsafeCell<[i16; MENU_CAP]>,
    built: UnsafeCell<bool>,
}
// SAFETY: only touched from the single-threaded desktop on the boot CPU.
unsafe impl Sync for MenuTree {}
static MENU: MenuTree = MenuTree {
    obj: UnsafeCell::new([EMPTY_OBJ; MENU_CAP]),
    title_obj: UnsafeCell::new([0; 4]),
    item_obj: UnsafeCell::new([0; N_ITEMS]),
    obj_item: UnsafeCell::new([-1; MENU_CAP]),
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
        let seed = if ti == 0 { MIN_DESKMENU_WIDTH - 1 } else { 0 };
        let m = def.entries.iter().map(|e| label_len(entry_label(e))).fold(seed, i16::max) + 1;
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
            for (ii, entry) in def.entries.iter().enumerate() {
                let last_item = ii == n_items - 1;
                let last_overall = (ti == nt - 1) && last_item;
                let mut flags = 0u16;
                if last_overall {
                    flags |= LASTOB;
                }
                // `{0, row, 20, 1}` (`desk_rsc.c:297-341`) after `rsrc_obfix`:
                // the entry spans the box's width at `row · gl_hchar`, one
                // character cell tall ([`ENTRY_H`]).
                o[k] = mk(
                    G_STRING,
                    flags,
                    entry_label(entry).as_ptr() as u64,
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
        Sep(s) | Act(_, s) => s,
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
pub fn install_menu_bar() {
    let tree = build_menu();
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
        | Item::SizeToFit => Title::View,
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

/// The control-character keyboard shortcut for `item` (EmuTOS
/// `default_shortcuts[]`, `deskmain.c:269-307`) — the displayed uppercase
/// letter, or `0` for none. Only the Task-#8-enabled File items are actually
/// reachable; the rest map to items `men_update` keeps disabled.
fn shortcut_letter(item: Item) -> u8 {
    match item {
        Item::Open => b'O',
        Item::InfoRename => b'I',
        Item::NewFolder => b'N',
        Item::CloseFolder => b'H',
        Item::CloseWindow => b'U',
        Item::Cycle => b'W',
        Item::SelectAll => b'A',
        Item::Delete => b'D',
        Item::ExecEmuCon => b'Z',
        Item::ChangeRez => b'R',
        Item::SaveDesktop => b'S',
        _ => 0,
    }
}

/// `lookup_ascii_shortcut` (EmuTOS `deskmain.c:1169-1198`) — resolve a
/// **control** character (`ascii < 0x20`) to the `(title, item)` its
/// `menu_shortcuts[]` entry names. `ascii | 0x40` is the displayed uppercase
/// letter matched against [`shortcut_letter`]. A currently **disabled** item is
/// treated as no match (its shortcut is inert). Returns `None` for a
/// non-control character or no/disabled match.
pub fn lookup_ascii_shortcut(ascii: u8) -> Option<Item> {
    if ascii >= 0x20 {
        return None;
    }
    let letter = ascii | 0x40;
    for &item in &ALL_ITEMS {
        if shortcut_letter(item) == letter {
            // A matching but disabled item stops the scan (EmuTOS `break`).
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
    let (nsel, nwin, napp, ndesk, view, sort, fit) = global::with(|g| {
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
    .unwrap_or((0, 0, 0, 0, View::Icon, Sort::Name, true));

    for &item in &ALL_ITEMS {
        set_disabled(item, !item_enabled(item, nsel, nwin, napp, ndesk));
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

/// Whether `item` is enabled for the current `nsel`/`nwin`/`napp`/`ndesk`
/// (`napp` = the number of selected native programs, `ndesk` = the number of
/// selected desktop icons — EmuTOS `men_update`'s two icon counts).
fn item_enabled(item: Item, nsel: usize, nwin: usize, napp: usize, ndesk: usize) -> bool {
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
        Item::Delete => nsel >= 1,
        Item::InfoRename => nsel == 1,
        Item::NewFolder | Item::SelectAll | Item::SetMask => nwin >= 1,
        // Search (`CONF_WITH_SEARCH`): EmuTOS disables it only when there is
        // neither an open window nor a selection; NewTOS's Search scans the
        // current window, so it needs an open window (`deskmain.c:548-556`).
        Item::Search => nwin >= 1,
        // Task-#14 deskinf/NEWDESK.INF ops: always available (EmuTOS
        // `CONF_WITH_BACKGROUNDS`/`CONF_WITH_READ_INF` + the always-live Save).
        // `PREFITEM` likewise: EmuTOS lists it in none of `ILL_NOSEL` /
        // `ILL_MULTSEL` / `ILL_NOWIN` (`deskmain.c:143-160`), so Set
        // preferences is enabled whatever is selected (DESK02).
        Item::SetBackground | Item::ReadInf | Item::SaveDesktop | Item::SetPrefs => true,
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
        // The remaining Options items stay disabled (deskrez is
        // config.txt-driven, ADR-017 rev 2; Desktop configuration is DESK05).
        _ => false,
    }
}
