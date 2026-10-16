//! `desktop::global` — the desktop's process-lifetime state (`DeskState`, our
//! [`GLOBES G`]), the object arena, and the background pattern/colour.
//!
//! Reimplements EmuTOS `deskglob.c` + `GLOBES` (`deskbind.h:130-245`) and the
//! `g_screen[]`/`g_screeninfo[]` arena from `deskobj.c`, per
//! `docs/api/desktop/_data-model.md`. `GLOBES` never crosses an ABI boundary
//! ("Contract: none"), so [`DeskState`] is a clean Rust design: the arena is
//! a heap [`Vec`] and the window list lives in [`super::win::WindowList`].
//!
//! [`GLOBES G`]: https://github.com/emutos/emutos/blob/master/desk/deskbind.h

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::ops::Deref;
use core::sync::atomic::{AtomicI16, AtomicU8, Ordering};

use tos_types::aes::{Grect, Object, DESKWH, G_BOX, G_IBOX, LASTOB, NIL};

use super::model::{AType, DateFmt, ScreenInfo, Sort, TimeFmt, View};
use super::win::WindowList;

// ── Object-tree layout constants (`deskconf.h`, our own choice of indices
//    per `_data-model.md`: "which index the desktop uses is internal"). ──────

/// `g_screen[]` index of the desktop pseudo-window's item root (`DROOT`).
pub const DROOT: i16 = 1;
/// Number of real window slots (`NUM_WNODES`, `deskconf.h`).
pub const NUM_WNODES: usize = 7;
/// First `g_screen[]` index available for item objects: `ROOT` (0) + `DROOT`
/// (1) + one box per possible window (`WOBS_START` = `NUM_WNODES + 2`).
pub const WOBS_START: i16 = NUM_WNODES as i16 + 2;

/// The `objc_draw`/`render_tree` recursion depth the desktop uses (`MAX_DEPTH`
/// in EmuTOS `do_wredraw`) — deep enough for `ROOT → window/DROOT → items`.
pub const MAX_DEPTH: i16 = 8;

/// The **fixed** number of item-object slots the free chain is seeded with by
/// [`DeskState::obj_init`]. This is **our choice** (`_data-model.md`: the
/// arena's indexing/sizing is internal), but it must cover the worst case up
/// front: [`DeskState::obj_ialloc`] **does not grow** the arena — it pops from
/// the free chain and returns `None` on exhaustion (the placement loop then
/// stops, EmuTOS parity). A fixed arena is required so the raw
/// `ob_spec`/`ib_ptext` pointers baked into the object tree never move under
/// `objc_draw` (a `Vec` regrow would reallocate and dangle them).
///
/// Worst case = `(NUM_WNODES + 1)` surfaces (the 7 windows + the desktop) ×
/// [`MAX_WIN_ITEMS`] visible cells in a maximum-size window. The densest layout
/// is `V_ICON` (`win_view`): `g_iwspc = g_wicon(100) + min_wint(8) = 108`,
/// `g_ihspc = g_hicon(50) + MIN_HINT(2) = 52` (`app.rs`/`win.rs`).
///
/// **At 1920×1080 (ADR-017 Stage 1, native boot)** a maximised window's border
/// rect is the desktop work area `{0, 19, 1920, 1080-19}` (below the
/// `gl_hbox`-high menu bar); `wind_calc` subtracts the `WINDOW_STYLE` borders `(l=1, t=42, r=23, b=23)`
/// (`tos-api::aes::window::borders`, the 3D branch: `adj_?box = gl_?box + 4 =
/// 23`, plus a full `gl_hbox = 19` for the INFO line) → a work area of
/// `1896 × 996`.
/// `win_bldview` then fits `wfit = 1896/108 = 17` cols and `hfit = 996/52 = 19`
/// rows; with its deliberate `+1` fringe row (`o_hfit = pnrow+1 = 20`, columns
/// stay capped at `17` in size-to-fit) the loop places at most
/// `17 × 20 = 340` cells. The desktop's own drive/trash/printer icons under
/// `DROOT` are ≤ 28 (26 drives + Trash + Printer). [`MAX_WIN_ITEMS`] = 350 is a
/// per-surface cap covering the 340-cell window with a small margin (and the
/// 28-icon desktop many times over), so the pool `8 × 350 = 2800` covers even
/// all 7 windows *and* the desktop simultaneously maxed (`7×340 + 28 = 2408`).
///
/// The extra static cost over the old 640×480 `48`-cell cap is one-time heap
/// (the arena is a `Vec`, allocated once in `obj_init` and never regrown):
/// `(2800-384) = 2416` more `Object` (~32 B) + `ScreenInfo` (~104 B) pairs,
/// ≈ 330 KB — negligible against the framebuffer's own ~8.3 MB.
pub const MAX_WIN_ITEMS: usize = 350;

/// See [`MAX_WIN_ITEMS`]. `(NUM_WNODES + 1) * MAX_WIN_ITEMS`.
pub const INITIAL_ITEM_SLOTS: usize = (NUM_WNODES + 1) * MAX_WIN_ITEMS;

// ── `OBJECT` graphic types + flag bits (`obdefs.h`) needed here and by the
//    menu builder in `mod.rs`. ────────────────────────────────────────────

/// EmuTOS `BORDER_TEXT_COLOURS` (`deskapp.c:80`): `(BLACK << 12) | (BLACK << 8)`
/// = `0x1100`. OR'd with the persisted background pattern/colour byte to form
/// `DROOT`'s box colour word (`ob_spec`) — black border/text nibbles above the
/// pattern (bits 4-6) and fill-colour (bits 0-3) the byte supplies.
pub const BORDER_TEXT_COLOURS: u64 = 0x1100;

// ── Grid-metric gaps (`deskbind.h:53-56`) ───────────────────────────────────

/// `MIN_HINT` (`deskbind.h:56`) — the fixed 2-px vertical inter-icon gap.
pub const MIN_HINT: i16 = 2;

/// `MIN_WINT` (`deskbind.h:53-55`) — the horizontal inter-icon gap: 4 px in
/// low/medium-height modes (`gl_height <= 300`), else 8 px. `screen_h` is the
/// full display height.
pub fn min_wint(screen_h: i16) -> i16 {
    if screen_h <= 300 {
        4
    } else {
        8
    }
}

/// The full window-gadget widget set every desktop window uses (EmuTOS
/// `WINDOW_STYLE`, `deskwin.c:48-49`): NAME|CLOSER|MOVER|FULLER|INFO|SIZER|
/// UPARROW|DNARROW|VSLIDE|LFARROW|RTARROW|HSLIDE = `0x0FFF`.
pub const WINDOW_STYLE: u16 = 0x0FFF;

/// An x/y pair in device pixels (EmuTOS `Point`, `include/gemdos.h`-adjacent
/// `obdefs.h:34-38`). Used for the drag-outline polylines and for the list of
/// selected-object origins `gr_accobs` builds.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Point {
    /// Horizontal offset.
    pub x: i16,
    /// Vertical offset.
    pub y: i16,
}

/// `NUM_ICON_POINTS` (`deskbind.h:123`) — the vertex count of the drag
/// outline of an item shown as an **icon** (on the desktop or in a window).
pub const NUM_ICON_POINTS: usize = 9;
/// `NUM_TEXT_POINTS` (`deskbind.h:124`) — the vertex count of the drag
/// outline of an item shown as **text** (in a window): a plain rectangle.
pub const NUM_TEXT_POINTS: usize = 5;

/// A fully-empty `OBJECT`, used to grow the arena before it is wired up.
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

// ── Background pattern/colour (`g_patcol`) ─────────────────────────────────
//
// Kept as atomics, separate from the `RefCell`-held `DeskState`, so the persisted
// pattern/colour byte can be read/written (EMUDESK.INF `#Q` load/save, the
// Set-Background dialog) without borrowing `DeskState`. This byte
// is the single source of truth that feeds `DROOT`'s box `ob_spec` via
// [`DeskState::sync_desk_background`] — the AES then draws that box as the screen
// background (`WF_NEWDESK`/`w_drawdesk`). Six bytes:
// `[desk0, win0, desk1, win1, desk2, win2]`, pattern+colour each; NewTOS is a
// >2-plane display, so it uses the index-4/5 pair. Defaults are the EmuTOS
// `#Q` defaults (IP_4PATT + green desktop). Reading/writing `NEWDESK.INF`
// lands in `inf.rs` (Task #9).
static G_PATCOL: [AtomicU8; 6] = [
    AtomicU8::new(0x41),
    AtomicU8::new(0x40),
    AtomicU8::new(0x43),
    AtomicU8::new(0x40),
    AtomicU8::new(0x43),
    AtomicU8::new(0x40),
];

/// The current desktop background byte (`g_patcol[2].desktop`): high nibble =
/// AES inside-pattern index, low nibble = VDI colour.
pub fn patcol_desktop() -> u8 {
    G_PATCOL[4].load(Ordering::Acquire)
}

/// Set the desktop background byte for the current (>2-plane) bucket
/// (`g_patcol[2].desktop`), written by the Set-background dialog
/// ([`super::inf::set_background`]).
pub fn set_patcol_desktop(v: u8) {
    G_PATCOL[4].store(v, Ordering::Release);
}

/// Overwrite all six background pattern/colour bytes
/// (`[desk0, win0, desk1, win1, desk2, win2]`), applied from a parsed
/// `NEWDESK.INF` `#Q` line ([`super::inf::load_and_apply`]).
pub fn set_patcol_all(vals: [u8; 6]) {
    for (slot, v) in G_PATCOL.iter().zip(vals.iter()) {
        slot.store(*v, Ordering::Release);
    }
}

/// Read all six background pattern/colour bytes, for serialising the `#Q` line
/// ([`super::inf::app_save`]).
pub fn patcol_all() -> [u8; 6] {
    let mut out = [0u8; 6];
    for (o, slot) in out.iter_mut().zip(G_PATCOL.iter()) {
        *o = slot.load(Ordering::Acquire);
    }
    out
}

// ── Process-local statics ───────────────────────────────────────────────────

/// A `static` that belongs to the desktop alone — EmuTOS's desktop keeps its
/// state in plain C globals (`GLOBES G`, `deskglob.c`; the menu, `deskmain.c`)
/// that only its own single thread of control ever touches. Rust demands
/// `Sync` of every `static`; this wrapper states that invariant **once** for
/// all desktop statics instead of per type, so the cells inside can be the
/// single-threaded `RefCell`/`UnsafeCell` they really are (DESK43 U8a: the
/// kernel `SpinMutex` the state used to sit behind is a kernel type the
/// desktop cannot take along into its own crate, and a lock buys nothing for
/// one thread — a nested borrow is a bug either way, a `RefCell` reports it as
/// a panic where the spinlock hung).
///
/// **The invariant.** Every access comes from the desktop's own control flow,
/// one call at a time:
/// * the desktop is the EL0 program `NEWDESK` (DESK43 U9), one single-threaded
///   process, and these statics live in its own address space; no interrupt
///   handler, no AES callback and no other process names any desktop item
///   (since DESK43 U10 the kernel does not link this crate at all —
///   `tools/desk-drift-probe.sh` group C);
/// * while one of its traps is parked (an `evnt_multi` that yields to an
///   accessory, ACC03) no desktop code runs: the accessory is an EL0 process
///   with its own code, and the desktop resumes where it stopped.
///
/// A second thread or an interrupt-context caller would break the invariant;
/// neither exists, and adding one has to replace this wrapper first.
pub struct DeskStatic<T>(T);

// SAFETY: see the invariant above — every access to a `DeskStatic` comes from
// the desktop's single thread of control, so no two threads ever share one.
unsafe impl<T> Sync for DeskStatic<T> {}

impl<T> DeskStatic<T> {
    /// Wrap `value`. `const fn`, for `static` initialisers.
    // NEWTOS: Rust plumbing — a C global needs no `Sync` wrapper.
    pub const fn new(value: T) -> Self {
        Self(value)
    }
}

impl<T> Deref for DeskStatic<T> {
    type Target = T;
    // NEWTOS: Rust plumbing — a C global needs no `Sync` wrapper.
    fn deref(&self) -> &T {
        &self.0
    }
}

// ── Character-cell metrics (`gl_wchar`/`gl_hchar`, `gl_wschar`/`gl_hschar`) ──
//
// EmuTOS keeps these as plain globals beside `G`, not inside it
// (`include/gsxdefs.h:82-86`). The desktop fills the normal-font pair from
// `graf_handle(&gl_wchar, &gl_hchar, &gl_wbox, &gl_hbox)` at the start of every
// `deskmain` run (`deskmain.c:1956`). The small-font pair is an AES global the
// ROM desktop reads directly (`deskapp.c:519-520,810-811,994-1002`); an
// application has no AES call for it, so [`super::deskmain`] asks the VDI the
// way the AES itself computes it (`gsx_start`, `aes/gemgraf.c:405-410`, via
// [`super::gsx::small_font_cell`]). Kept as atomics, like [`G_PATCOL`], so
// code running inside [`with`] (e.g. `app_start`) can read them without
// re-borrowing [`DESK`]. Zero until the first `deskmain` sets them;
// nothing in the desktop reads them before that.
static GL_WCHAR: AtomicI16 = AtomicI16::new(0);
static GL_HCHAR: AtomicI16 = AtomicI16::new(0);
static GL_WSCHAR: AtomicI16 = AtomicI16::new(0);
static GL_HSCHAR: AtomicI16 = AtomicI16::new(0);

/// `gl_wchar` (`include/gsxdefs.h:82`) — the normal (IBM) font's character
/// cell width, as `graf_handle` reported it (`deskmain.c:1956`).
pub fn gl_wchar() -> i16 {
    GL_WCHAR.load(Ordering::Relaxed)
}

/// `gl_hchar` (`include/gsxdefs.h:83`) — the normal (IBM) font's character
/// cell height, as `graf_handle` reported it (`deskmain.c:1956`).
pub fn gl_hchar() -> i16 {
    GL_HCHAR.load(Ordering::Relaxed)
}

/// `gl_wschar` (`include/gsxdefs.h:85`) — the small font's character cell
/// width, `vst_height(ws_chminh)`'s cell width (`aes/gemgraf.c:407`).
pub fn gl_wschar() -> i16 {
    GL_WSCHAR.load(Ordering::Relaxed)
}

/// `gl_hschar` (`include/gsxdefs.h:86`) — the small font's character cell
/// height, `vst_height(ws_chminh)`'s cell height (`aes/gemgraf.c:407`).
pub fn gl_hschar() -> i16 {
    GL_HSCHAR.load(Ordering::Relaxed)
}

/// Store the four cell metrics `deskmain` learned at start-up
/// (`deskmain.c:1956` for `gl_wchar`/`gl_hchar`; `aes/gemgraf.c:407` for
/// `gl_wschar`/`gl_hschar`).
pub fn set_cell_metrics(wchar: i16, hchar: i16, wschar: i16, hschar: i16) {
    GL_WCHAR.store(wchar, Ordering::Relaxed);
    GL_HCHAR.store(hchar, Ordering::Relaxed);
    GL_WSCHAR.store(wschar, Ordering::Relaxed);
    GL_HSCHAR.store(hschar, Ordering::Relaxed);
}

/// EmuTOS `WSAVE` (`deskapp.h:120-131`) — one window's saved context: its frame
/// rectangle in **pixels**, its scroll position and the directory it shows.
///
/// A slot with an empty [`Self::pth_save`] is unused, exactly as EmuTOS's
/// `pth_save[0] == 0` marks one (`cnx_put`, `deskmain.c:1405-1409`).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct WSave {
    /// Window frame x (`x_save`).
    pub x_save: i16,
    /// Window frame y (`y_save`).
    pub y_save: i16,
    /// Window frame width (`w_save`).
    pub w_save: i16,
    /// Window frame height (`h_save`).
    pub h_save: i16,
    /// Horizontal scroll position — the window's `w_cvcol` (`hsl_save`).
    pub hsl_save: i16,
    /// Vertical scroll position — the window's `w_cvrow` (`vsl_save`).
    pub vsl_save: i16,
    /// The directory the window shows (`pth_save`, EmuTOS `w_pnode.p_spec`).
    pub pth_save: String,
}

/// EmuTOS `CSAVE` (`deskapp.h:148-156`), `G.g_cnxsave` — the desktop's **own**
/// context save area: the preferences and the window layout, held between the
/// `NEWDESK.INF` image and the live desktop.
///
/// It is written by `app_start` (from the file) and by `cnx_put` (from the live
/// desktop), and read by `cnx_get` (to the live desktop) and by `app_save` (to
/// the file). The AES window table is deliberately **not** the place for this:
/// it is wiped on every shell pass, which is precisely why EmuTOS's desktop
/// keeps its own copy (gap G2 of `docs/analysis/desk-aes-gaps-2026-09-14.md`).
///
/// **Field subset, deliberate.** EmuTOS's `CSAVE` also carries `cs_blitter`
/// and `cs_cache`. Those mirror `GLOBES` fields that exist only because
/// `inf_pref`/`inf_conf` (the Preferences and Configuration dialogs) can edit
/// them, and NewTOS has no live state behind either: they are m68k hardware
/// (`Blitmode`, the 68030 cache), the hardware-layer carve-out of
/// `docs/COMPATIBILITY.md`.
///
/// Their bits still survive a load/save round trip — the `#E` bytes they live
/// in are preserved verbatim by [`super::inf`]'s session copy — they are simply
/// not routed through this structure.
///
/// `cs_fullpath` **was** in that list until DESK19, on the grounds that its
/// consumer — `build_selected_path` (`deskfun.c:903-928`) out of
/// `fun_win2win`'s drop-on-an-application branch (`:958-970`) — did not exist
/// here. It does now ([`super::dir`]'s `build_selected_path`), so the field is
/// carried like the others.
///
/// `cs_appdir` **was** in that list until DESK18, on the grounds that NewTOS's
/// launch path set no current directory at all. It does now
/// ([`super::main`]'s `set_launch_dir` → `set_default_path`), so the field is
/// carried like the others.
///
/// `cs_confcpy` and `cs_confovwr` **were** in that list until DESK13, on the
/// grounds that NewTOS's copy engine asked neither question. It asks both now
/// (`super::dir`'s `dir_op` confirmation box and `output_fname`'s name-conflict
/// dialog), so they are carried like the other four.
#[derive(Clone)]
pub struct CSave {
    /// Saved view mode (`cs_view`).
    pub cs_view: View,
    /// Saved sort key (`cs_sort`).
    pub cs_sort: Sort,
    /// Saved "size windows to fit" preference (`cs_sizefit`).
    pub cs_sizefit: bool,
    /// Saved "confirm deletes" preference (`cs_confdel`).
    pub cs_confdel: bool,
    /// Saved "confirm copies" preference (`cs_confcpy`).
    pub cs_confcpy: bool,
    /// Saved "confirm overwrites" preference (`cs_confovwr`). Note the `#E`
    /// bit it serialises to is **inverted** (`INF_E2_ALLOWOVW`, "yes, it's
    /// backwards" — `deskapp.c:98`).
    pub cs_confovwr: bool,
    /// Saved double-click speed index 0..4 (`cs_dblclick`); 0 is the slowest
    /// (EmuTOS `gl_dcrates`, our `tos-api/src/aes/event.rs` `DCRATES`).
    pub cs_dblclick: u8,
    /// Saved clock format (`cs_timefmt`).
    pub cs_timefmt: TimeFmt,
    /// Saved date order (`cs_datefmt`).
    pub cs_datefmt: DateFmt,
    /// Saved *Default dir* switch (`cs_appdir`, `deskapp.h:153`) — see
    /// [`DeskState::g_appdir`]. Serialises to `INF_E5_APPDIR`.
    pub cs_appdir: bool,
    /// Saved *Parameter* switch (`cs_fullpath`, `deskapp.h:154`) — see
    /// [`DeskState::g_fullpath`]. Serialises to `INF_E5_ISFULL`.
    pub cs_fullpath: bool,
    /// One [`WSave`] per window slot (`cs_wnode[NUM_WNODES]`); slot 0 is the
    /// **bottom-most** window (`cnx_put`'s comment, `deskmain.c:1367-1369`).
    pub cs_wnode: [WSave; NUM_WNODES],
}

impl Default for CSave {
    fn default() -> Self {
        Self {
            cs_view: View::Icon,
            cs_sort: Sort::Name,
            cs_sizefit: true,
            cs_confdel: true,
            cs_confcpy: true,
            cs_confovwr: true,
            cs_dblclick: DEF_DCLICK,
            cs_timefmt: TimeFmt::H24,
            cs_datefmt: DateFmt::Mdy,
            cs_appdir: true,
            cs_fullpath: true,
            cs_wnode: Default::default(),
        }
    }
}

/// The double-click speed index the desktop starts with when no `NEWDESK.INF`
/// supplies one: `INF_E1_DEFAULT`'s low three bits (`deskapp.c:92`,
/// `INF_E1_CONFDEL|INF_E1_CONFCPY|2`).
pub const DEF_DCLICK: u8 = 2;

/// EmuTOS `ANODE` (`deskapp.h:83-99`) — one entry of the desktop's application
/// list `G.g_ahead`, in the **`AF_ISDESK` shape**: an icon that sits on the
/// desktop itself.
///
/// **Deliberate subset, and where the rest lives.** EmuTOS keeps *one* list for
/// two different things: the desktop icons (`AF_ISDESK`: disk / trash /
/// printer, plus the file and folder shortcuts of
/// `CONF_WITH_DESKTOP_SHORTCUTS`) and the file-type / installed-application
/// associations (`#D`/`#G`/`#Y`/`#F`/`#P`/`#I`/`#N`). NewTOS splits them: the
/// association half has lived in [`super::inf`]'s session `NEWDESK.INF` model
/// since the Install-Application work, and this list carries the desktop-icon
/// half. Both halves serialise into the same file, in the same `ANODE` line
/// grammar — the split is internal.
///
/// Since DESK30 the list also holds desktop **shortcuts**
/// (`CONF_WITH_DESKTOP_SHORTCUTS`): a file or folder icon parked on the
/// desktop, `a_type` `AT_ISFILE`/`AT_ISFOLD`, written as an `#X`/`#V` line.
/// For those, `a_pappl` is the **full path** and `a_pdata` the bare file name
/// (`deskapp.h:88-90`, usage (2); `ins_shortcut`, `deskins.c:1380-1381`);
/// for a drive, the trash and the printer `a_pappl` stays the icon's label
/// (usage (1)) and `a_pdata` is empty. `a_pargs` still has no reader: EmuTOS
/// writes it only for the non-`AF_ISDESK` association nodes
/// (`app_save`, `deskapp.c:1263-1270`), which live in [`super::inf`].
#[derive(Clone)]
pub struct Anode {
    /// `a_type` — `AT_ISDISK` / `AT_ISTRSH` / `AT_ISPRNT`, and since DESK30
    /// also `AT_ISFILE` / `AT_ISFOLD` for a shortcut
    /// (`app_parse`, `deskapp.c:333-344`).
    pub a_type: AType,
    /// `a_letter` — the drive letter drawn into the bitmap, `0` for none.
    pub a_letter: u8,
    /// `a_aicon` — the icon number, in EmuTOS's `IG_*` numbering
    /// ([`super::icons::ig_of`]). A drive/trash/printer icon has no separate
    /// data icon; a shortcut copies both numbers from the `ANODE` that governs
    /// the file, so [`Anode::a_dicon`] carries the second one.
    pub a_aicon: i16,
    /// `a_dicon` — the *data* icon number, `NIL` when the node has none.
    /// `app_blddesk` chooses between the two (`deskapp.c:1374-1378`).
    pub a_dicon: i16,
    /// `a_flags` — the [`AF_ISEXEC`] bit and nothing else. `AF_ISDESK` is
    /// implicit (every node of `G.g_ahead` *is* a desktop node, which is what
    /// makes this list the `AF_ISDESK` half of EmuTOS's single one); the
    /// remaining `AF_*` bits belong to the association list in
    /// [`super::inf`].
    pub a_flags: u16,
    /// `a_pappl` — the icon's label for a drive/trash/printer, the **full
    /// path** for a shortcut.
    pub a_pappl: String,
    /// `a_pdata` — the bare file name of a shortcut, which is also its label;
    /// empty for every other node type.
    pub a_pdata: String,
    /// `a_xspot` — desired spot on the desktop, in **pixels relative to
    /// [`DROOT`]**. EmuTOS's is screen-absolute because its `app_blddesk` sizes
    /// `DROOT` to the whole screen (`deskapp.c:1344`); ours sizes it to the work
    /// area (`app_blddesk`), so every item coordinate under it is work-area
    /// relative and `snap_icon` already produces that form.
    pub a_xspot: i16,
    /// `a_yspot` — see [`Anode::a_xspot`].
    pub a_yspot: i16,
    /// `a_obid` — the `g_screen[]` index `app_blddesk` gave this icon, or
    /// [`NIL`] before the first build (`deskapp.c:1369`).
    pub a_obid: i16,
}

/// `AF_ISEXEC` (`deskapp.h:37`) — "this is an executable file". Set on a
/// desktop shortcut whose name has a program extension (`ins_shortcut`,
/// `deskins.c:1372-1373`); `app_blddesk` reads it to prefer the *application*
/// icon over the data icon (`deskapp.c:1374-1378`).
pub const AF_ISEXEC: u16 = 0x4000;

/// The desktop's process-lifetime global state (EmuTOS `GLOBES G`).
pub struct DeskState {
    /// Current view mode (`g_iview`).
    pub view: View,
    /// Current sort key (`g_isort`).
    pub sort: Sort,
    /// "Size windows to fit" preference (`g_ifit`, `CONF_WITH_SIZE_TO_FIT`).
    pub size_to_fit: bool,

    /// "Confirm deletes" preference (`g_cdelepref`, `deskinf.c:735`): gate for
    /// the delete confirmation in [`super::dir`] (`deskdir.c:906`,
    /// `confirm = G.g_cdelepref`). Edited by `inf_pref`, persisted through
    /// [`CSave::cs_confdel`].
    pub g_cdelepref: bool,
    /// "Confirm copies" preference (`g_ccopypref`, `deskinf.c:740`): gate for
    /// the copy/move confirmation box in [`super::dir::dir_op`]
    /// (`deskdir.c:939`, `confirm = G.g_ccopypref`; a move or rename ORs
    /// [`Self::g_cdelepref`] in, `:943`).
    pub g_ccopypref: bool,
    /// "Confirm overwrites" preference (`g_covwrpref`, `deskinf.c:745`): gate
    /// for the per-file name-conflict dialog in `output_fname`
    /// (`deskdir.c:325`, `if (!G.g_covwrpref) break;`). `false` means a copy
    /// replaces an existing file without asking.
    pub g_covwrpref: bool,
    /// Double-click speed index 0..4 (`g_cdclkpref`, `deskinf.c:751`). The
    /// desktop does not consume it: it is the AES's rate, queried and set with
    /// `evnt_dclick` (`deskmain.c:1460`), and held here only so `cnx_put`/
    /// `app_save` can write it into the `#E` line.
    pub g_cdclkpref: u8,
    /// Clock format for the text view and Show Info (`g_ctimeform`,
    /// `deskinf.c:82`).
    pub g_ctimeform: TimeFmt,
    /// Date order for the text view and Show Info (`g_cdateform`,
    /// `deskinf.c:138`).
    pub g_cdateform: DateFmt,
    /// "Default dir is the application's dir" (`g_appdir`, `deskbind.h:180`):
    /// the *Default dir* switch of `inf_conf`, and the one thing that decides
    /// which directory a **not**-installed program is launched in — `true` ⇒
    /// the program's own, `false` ⇒ the top window's when there is one
    /// (`do_aopen`, `desksupp.c:896-912`; NewTOS's [`super::main`]
    /// `set_launch_dir`). An *installed* application answers the same question
    /// from its own `INF_AT_APPDIR` flag instead, so this global does not
    /// apply to it. Persisted through [`CSave::cs_appdir`].
    pub g_appdir: bool,
    /// "Pass the full path as the argument" (`g_fullpath`, `deskbind.h:181`):
    /// the *Parameter* switch of `inf_conf`, read by exactly one function —
    /// `build_selected_path` (`deskfun.c:903-928`), which turns the first
    /// selected file of the **source** window into the command tail when that
    /// selection is dropped on a program icon (`fun_win2win`, `:958-970`;
    /// NewTOS's [`super::dir`] `fun_win2win`). `true` ⇒ the file's full path,
    /// `false` ⇒ its bare name. An *installed* application answers the same
    /// question for a **document** launch from its own `INF_AT_ISFULL` flag
    /// instead, so this global does not apply there. Persisted through
    /// [`CSave::cs_fullpath`].
    pub g_fullpath: bool,
    /// The current `_IDT` cookie word (`g_idt`, `deskbind.h:183`) — the machine
    /// separator / field order / clock that [`TimeFmt::Idt`] and [`DateFmt::Idt`]
    /// defer to (see [`super::model`]'s `_IDT` block for the bit layout and for
    /// why this is a GEMDOS `Ssystem` read and not `Supexec`).
    ///
    /// Refreshed where EmuTOS refreshes it — in `do_wredraw`
    /// (`desksupp.c:219`) — and additionally once at desktop start, because
    /// NewTOS formats a text-view line when the **view** is built
    /// (`super::win`'s `build_item`) rather than at draw time as EmuTOS's
    /// `dr_fnode` does (`deskinf.c:320`), so the value has to be valid before
    /// the first redraw rather than during it.
    pub g_idt: u32,

    /// One item's own horizontal extent (`g_iwext`) — set by `win_view()`
    /// (Task #6).
    pub g_iwext: i16,
    /// One item's own vertical extent (`g_ihext`) — set by `win_view()`.
    pub g_ihext: i16,
    /// Inter-item horizontal gap (`g_iwint`) — set by `win_view()`.
    pub g_iwint: i16,
    /// Inter-item vertical gap (`g_ihint`) — set by `win_view()`.
    pub g_ihint: i16,
    /// Column pitch in a window (`g_iwspc = g_iwext + g_iwint`) — set by
    /// `win_view()`.
    pub g_iwspc: i16,
    /// Row pitch in a window (`g_ihspc = g_ihext + g_ihint`) — set by
    /// `win_view()`.
    pub g_ihspc: i16,
    /// Max columns fitting a full-size window (`g_icols`) — set by
    /// `win_view()`.
    pub g_icols: i16,
    /// Work-area width of a maximum-size window at the current resolution
    /// (`wind_calc_grect(WC_WORK, WINDOW_STYLE, g_desk)`), cached at boot so
    /// `win_view()` can recompute `g_icols` on a view switch without a fresh
    /// AES call. Fixed for a fixed resolution.
    pub full_work_w: i16,

    /// Universal icon-cell width (`g_wicon`) — set by `app_start()` (Task #6).
    pub g_wicon: i16,
    /// Universal icon-cell height (`g_hicon`) — set by `app_start()`.
    pub g_hicon: i16,

    /// Desktop grid pitch, x (`g_icw`) — set by `app_start()`.
    pub g_icw: i16,
    /// Desktop grid pitch, y (`g_ich`) — set by `app_start()`.
    pub g_ich: i16,

    /// `G.g_xyicon` (`deskbind.h:190`) — "outline for dragging file as icon",
    /// the 9-vertex polyline of one icon cell (bitmap column narrower than the
    /// label box below it). Built by `app_start` (`deskapp.c:990-1005`) from
    /// `g_wicon`/`g_hicon`, so it follows the icon metrics.
    pub g_xyicon: [Point; NUM_ICON_POINTS],
    /// `G.g_xytext` (`deskbind.h:191`) — "outline for dragging file as text",
    /// the 5-vertex rectangle `DRAG_BOX_WIDTH` characters wide and one text row
    /// high (`deskapp.c:1007-1011`).
    pub g_xytext: [Point; NUM_TEXT_POINTS],

    /// Desktop work-area rectangle (screen minus the menu bar) (`g_desk`).
    pub g_desk: Grect,

    /// The current window's item-root object index (`g_croot`): [`DROOT`] for
    /// the desktop, else the window's `w_root`. Set by `desk_verify`; read by
    /// `men_update`/`hndl_button` to walk the current selection.
    pub g_croot: i16,
    /// The current window's AES handle (`g_cwin`); [`DESKWH`] for
    /// the desktop. Set by `desk_verify`.
    pub g_cwin: i16,
    /// The window whose selection state a subsequent menu action applies to
    /// (`g_wlastsel`) — the last window verified. Set by `desk_verify`.
    pub g_wlastsel: i16,

    /// The whole desktop+windows+items `OBJECT` array — this **is** the AES
    /// tree the desktop hands to `objc_draw` (`g_screen`).
    pub g_screen: Vec<Object>,
    /// Parallel per-object auxiliary data, index-aligned with `g_screen`
    /// (`g_screeninfo`).
    pub g_screeninfo: Vec<ScreenInfo>,
    /// Head of the item-object free chain (index into `g_screen`, chained via
    /// `ob_next`), or [`NIL`] when exhausted (`g_screenfree`).
    pub g_screenfree: i16,

    /// The window list (`g_wdesktop`/`g_wlist`/`g_wfirst`/`g_wcnt`).
    pub windows: WindowList,

    /// `G.g_ahead` — the desktop-icon list ([`Anode`]), the single source of
    /// truth for what `app_blddesk` puts on the desktop. Filled from the
    /// `#M`/`#T`/`#O` lines by [`super::inf::load_and_apply`] (`app_start`,
    /// `deskapp.c:851-987`) or, when the file supplies none, from the drive map
    /// ([`super::inf::build_desk_icons`] = `build_inf`, `deskapp.c:696-752`);
    /// edited by [`super::ins`] (`ins_devices` / `ins_icon` / `rmv_icon`) and by
    /// `move_drvicon`; written back by [`super::inf::app_save`]
    /// (`app_save`).
    ///
    /// **Order.** EmuTOS's `app_alloc` *prepends* (`deskapp.c:215-216`), so
    /// `G.g_ahead` runs newest-first and both `app_blddesk` and `app_save`
    /// bracket their walk with `app_revit()` to get creation order back
    /// (`:1352`, `:1211`). A [`Vec`] that `push`es is already in creation order,
    /// so there is no `app_revit` here and both walks are plain forward loops —
    /// the same visit order, without the list surgery.
    pub g_ahead: Vec<Anode>,
    /// `G.g_cnxsave` — the desktop's context save area ([`CSave`]): the saved
    /// view/sort/size-to-fit and the per-slot window layout. Filled from the
    /// `#E`/`#W` lines by [`super::inf::load_and_apply`] (`app_start`) and from
    /// the live desktop by [`super::main::cnx_put`]; read back by
    /// [`super::main::cnx_get`] and by [`super::inf::app_save`]
    /// (`app_save`).
    pub g_cnxsave: CSave,
}

impl DeskState {
    /// Construct the desktop state for a `screen_w`x`screen_h` display, with
    /// the field defaults. The window list and object arena are set up
    /// afterwards by [`DeskState::win_start`] then [`DeskState::obj_init`]
    /// (see [`init`]).
    pub fn new(g_desk: Grect) -> Self {
        Self {
            view: View::Icon,
            sort: Sort::Name,
            size_to_fit: true,
            g_cdelepref: true,
            g_ccopypref: true,
            g_covwrpref: true,
            g_cdclkpref: DEF_DCLICK,
            g_ctimeform: TimeFmt::H24,
            g_cdateform: DateFmt::Mdy,
            // `INF_E5_DEFAULT` carries `INF_E5_APPDIR|INF_E5_ISFULL`
            // (`deskapp.c:115`), so the no-`.INF` defaults are the
            // application's own directory and the full path as the argument.
            g_appdir: true,
            g_fullpath: true,
            g_idt: 0,
            g_iwext: 0,
            g_ihext: 0,
            g_iwint: 0,
            g_ihint: 0,
            g_iwspc: 0,
            g_ihspc: 0,
            g_icols: 0,
            full_work_w: 0,
            g_wicon: 0,
            g_hicon: 0,
            g_icw: 0,
            g_ich: 0,
            // `bzero(G.g_xyicon, …)` / `bzero(G.g_xytext, …)` before
            // `app_start` fills them (`deskapp.c:989,1007`).
            g_xyicon: [Point::default(); NUM_ICON_POINTS],
            g_xytext: [Point::default(); NUM_TEXT_POINTS],
            g_desk,
            g_croot: DROOT,
            g_cwin: DESKWH,
            g_wlastsel: DESKWH,
            g_screen: Vec::new(),
            g_screeninfo: Vec::new(),
            g_screenfree: NIL,
            windows: WindowList::default(),
            g_ahead: Vec::new(),
            g_cnxsave: CSave::default(),
        }
    }

    /// `obj_init` — allocate and lay out the object arena (EmuTOS
    /// `deskobj.c:113-147`).
    ///
    /// Objects `0..WOBS_START` are the structural containers: `ROOT` (0,
    /// full screen), `DROOT` (1, the desktop work area), and one box per
    /// possible window (2..=`DROOT+NUM_WNODES`), all made children of `ROOT`.
    /// The remaining [`INITIAL_ITEM_SLOTS`] objects are threaded onto the item
    /// free chain (head = [`WOBS_START`]).
    ///
    /// **Ordering note:** EmuTOS sizes this allocation from `g_iwspc`/
    /// `g_ihspc` and therefore requires `win_view()` to run first
    /// (`deskwin.c:117-118`). NewTOS instead seeds a **fixed** pool sized for
    /// the worst case ([`INITIAL_ITEM_SLOTS`]) — the arena's sizing is internal
    /// per `_data-model.md` — so `obj_init` does not depend on the grid metrics
    /// `win_view()`/`app_start()` compute. The pool never grows afterwards (see
    /// [`DeskState::obj_ialloc`]).
    pub fn obj_init(&mut self) {
        let n_struct = WOBS_START as usize;
        let total = n_struct + INITIAL_ITEM_SLOTS;

        let mut screen = Vec::with_capacity(total);
        let mut info = Vec::with_capacity(total);
        screen.resize(total, EMPTY_OBJ);
        info.resize(total, ScreenInfo::default());

        let screen_w = self.g_desk.w;
        let screen_h = self.g_desk.y + self.g_desk.h;
        let last_box = DROOT + NUM_WNODES as i16; // = 8

        // ROOT (index 0): full-screen container. Its child links are laid by
        // the `objc_add` loop below, exactly as in EmuTOS `deskobj.c:145`.
        screen[0] = Object {
            ob_next: NIL,
            ob_head: NIL,
            ob_tail: NIL,
            ob_type: G_IBOX,
            ob_flags: 0,
            ob_state: 0,
            ob_spec: 0,
            ob_x: 0,
            ob_y: 0,
            ob_width: screen_w,
            ob_height: screen_h,
        };

        // DROOT (index 1): the desktop itself (its work-area rect). It is a
        // drawable `G_BOX` whose `ob_spec` carries the visible desktop
        // background pattern/colour (EmuTOS `gl_sampob[1]`, `deskobj.c:39`, and
        // `G.g_screen[DROOT].ob_spec = g_patcol.desktop | BORDER_TEXT_COLOURS`,
        // `deskapp.c:1327`). The AES draws this box — and the icon children under
        // it — as the screen background via `w_drawdesk` (`WF_NEWDESK`). The
        // low-byte fill is refreshed from the persisted byte by
        // [`DeskState::sync_desk_background`].
        screen[DROOT as usize] = Object {
            ob_next: NIL,
            ob_head: NIL,
            ob_tail: NIL,
            ob_type: G_BOX,
            ob_flags: 0,
            ob_state: 0,
            ob_spec: patcol_desktop() as u64 | BORDER_TEXT_COLOURS,
            ob_x: self.g_desk.x,
            ob_y: self.g_desk.y,
            ob_width: self.g_desk.w,
            ob_height: self.g_desk.h,
        };

        // One empty box per possible window (indices 2..=last_box), the last
        // carrying LASTOB. Their rects are set by `win_alloc` when a window
        // opens (Task #6).
        for i in 0..NUM_WNODES {
            let idx = 2 + i;
            let is_last = i == NUM_WNODES - 1;
            screen[idx] = Object {
                ob_next: NIL,
                ob_head: NIL,
                ob_tail: NIL,
                ob_type: G_IBOX,
                ob_flags: if is_last { LASTOB } else { 0 },
                ob_state: 0,
                ob_spec: 0,
                ob_x: 0,
                ob_y: 0,
                ob_width: 0,
                ob_height: 0,
            };
        }

        // Link DROOT and the per-window boxes under ROOT through the Object
        // Manager, not by hand: EmuTOS `deskobj.c:141-146` is
        // `for (ii = 0; ii < NUM_WNODES+1; ii++) objc_add(G.g_screen, ROOT,
        // DROOT+ii);`. `objc_add` lays `ob_head`/`ob_tail` on ROOT and the
        // sibling chain whose last link points back at the parent.
        let tree = screen.as_ptr() as u64;
        for obj in DROOT..=last_box {
            super::objc_add(tree, 0, obj);
        }

        // Thread the item slots onto the free chain.
        let last = total - 1;
        for (i, obj) in screen.iter_mut().enumerate().skip(n_struct) {
            obj.ob_next = if i < last { (i + 1) as i16 } else { NIL };
        }
        self.g_screenfree = if INITIAL_ITEM_SLOTS > 0 { WOBS_START } else { NIL };

        self.g_screen = screen;
        self.g_screeninfo = info;
    }

    /// `obj_ialloc` — pop one item `OBJECT` off the free chain, place it at
    /// `(x, y, w, h)`, and link it as the **last child** of `parent` (EmuTOS
    /// `deskobj.c:220`). Returns the `g_screen[]` index, or [`None`] when the
    /// free chain is exhausted.
    ///
    /// **No grow on exhaustion** (unlike a naive `Vec::push`): EmuTOS's
    /// `g_screen[]` is a fixed allocation and `obj_ialloc` failure simply
    /// stops the placement loop (`deskwin.c` "free chain exhausted → break").
    /// Keeping the arena fixed after [`DeskState::obj_init`] is also what makes
    /// the raw `ob_spec`/`ib_ptext` pointers into `g_screeninfo` stay valid
    /// across the `objc_draw` that reads them.
    pub fn obj_ialloc(&mut self, parent: i16, x: i16, y: i16, w: i16, h: i16) -> Option<i16> {
        let idx = self.g_screenfree;
        if idx == NIL {
            return None;
        }
        self.g_screenfree = self.g_screen[idx as usize].ob_next;
        {
            let o = &mut self.g_screen[idx as usize];
            o.ob_next = NIL;
            o.ob_head = NIL;
            o.ob_tail = NIL;
            o.ob_type = 0;
            o.ob_flags = 0;
            o.ob_state = 0;
            o.ob_spec = 0;
            o.ob_x = x;
            o.ob_y = y;
            o.ob_width = w;
            o.ob_height = h;
        }
        self.g_screeninfo[idx as usize] = ScreenInfo::default();
        self.ob_add(parent, idx);
        Some(idx)
    }

    /// Link `child` as the last child of `parent` — the AES `objc_add` (40),
    /// which is what EmuTOS's desktop calls here (`deskobj.c:232`, inside
    /// `obj_ialloc`, and `:145` for the structural boxes). The sibling-chain
    /// bookkeeping (`ob_next` of the last child pointing back at the parent)
    /// belongs to the Object Manager, not to the desktop; it used to be
    /// open-coded on `g_screen` right here.
    fn ob_add(&mut self, parent: i16, child: i16) {
        super::objc_add(self.g_screen.as_ptr() as u64, parent, child);
    }

    /// `obj_wfree` — free **all** of `root`'s child item objects back onto the
    /// free chain and resize `root` to `(x, y, w, h)` with no children
    /// (EmuTOS `deskobj.c:187`). Called by `win_bldview` to reset a window's
    /// item tree (and reposition its root box to the new work-area rect)
    /// before rebuilding.
    pub fn obj_wfree(&mut self, root: i16, x: i16, y: i16, w: i16, h: i16) {
        let root_tail = self.g_screen[root as usize].ob_tail;
        let mut child = self.g_screen[root as usize].ob_head;
        while child != NIL {
            let next = self.g_screen[child as usize].ob_next;
            let is_tail = child == root_tail;
            // Push `child` onto the head of the item free chain.
            self.g_screen[child as usize].ob_next = self.g_screenfree;
            self.g_screenfree = child;
            if is_tail {
                break;
            }
            child = next;
        }
        let r = &mut self.g_screen[root as usize];
        r.ob_head = NIL;
        r.ob_tail = NIL;
        r.ob_x = x;
        r.ob_y = y;
        r.ob_width = w;
        r.ob_height = h;
    }

    /// Refresh `DROOT`'s box colour word from the persisted desktop
    /// pattern/colour byte (EmuTOS `deskapp.c:1327`:
    /// `G.g_screen[DROOT].ob_spec = G.g_patcol[n].desktop | BORDER_TEXT_COLOURS`).
    /// The AES draws `DROOT` (via `WF_NEWDESK`/`w_drawdesk`) as the screen
    /// background, so this makes a background-pattern change visible on the next
    /// redraw. Called from `app_blddesk` (build) and the Set-Background dialog.
    pub fn sync_desk_background(&mut self) {
        self.g_screen[DROOT as usize].ob_spec = patcol_desktop() as u64 | BORDER_TEXT_COLOURS;
    }
}

/// The single desktop-state instance (EmuTOS `GLOBES G`, `deskglob.c`).
/// Lazily initialised by [`init`]; one thread, so a `RefCell` (see
/// [`DeskStatic`]): a nested [`with`] panics with a borrow error instead of
/// spinning forever.
static DESK: DeskStatic<RefCell<Option<DeskState>>> = DeskStatic::new(RefCell::new(None));

/// One-time desktop-state setup for the desktop work area `g_desk` — the
/// `wind_get(DESKWH, WF_WXYWH)` rectangle EmuTOS stores in `G.g_desk`
/// (`deskmain.c:1959`). Builds [`DeskState`], runs `win_start()` (view/sort
/// defaults + window list) then `obj_init()` (the object arena), in that order.
/// Idempotent — a second call is a no-op.
pub fn init(g_desk: Grect) {
    let mut guard = DESK.borrow_mut();
    if guard.is_none() {
        let mut state = DeskState::new(g_desk);
        state.win_start();
        state.obj_init();
        *guard = Some(state);
    }
}

/// Drop the desktop state so the next [`init`] builds it from scratch — the
/// desktop's share of [`super::reset`].
///
/// On EmuTOS the desktop is an ordinary shell program: it ends, its memory goes
/// back to the pool (`deskmain.c:2131-2133`, `deskstart.S:57-59`), and the next
/// `sh_ldapp` runs `deskstart` again, which re-runs `app_start`/`win_start`/
/// `cnx_get` on a zeroed `G`. NewTOS's desktop is such a program since DESK43
/// U9 and gets that for free when it ends. What remains is the restart *inside*
/// the process, *Read .INF file* (`deskmain.c:2140-2147`): its state is this
/// lazily-initialised singleton and [`init`] is a no-op once built, so the
/// "memory goes back" half has to be stated there. (Until DESK43 U10 the
/// kernel's built-in copy also needed it before every start, because its state
/// outlived the call.)
///
/// Dropping the state frees the object arena and the cached `.INF` vectors
/// (all `alloc` allocations). Nothing else holds a pointer into them by then:
/// the AES's `gl_newdesk` registration, which does, has been cleared by
/// `deskmain`'s own epilog (`wind_set(DESKWH, WF_NEWDESK, NULL)`, `:2163`),
/// which precedes the one caller, [`super::run`]'s restart loop.
pub fn reset() {
    *DESK.borrow_mut() = None;
}

/// Run `f` against the desktop state (mutably borrowed for the duration of
/// `f`). Returns `None` if the state has not been [`init`]ialised yet. The
/// consumer of the arena/window APIs from later tasks (view build, event
/// dispatch) goes through here. `f` must not call `with` again: the borrow is
/// exclusive, and a nested one panics (it hung under the old spinlock).
pub fn with<R>(f: impl FnOnce(&mut DeskState) -> R) -> Option<R> {
    let mut guard = DESK.borrow_mut();
    guard.as_mut().map(f)
}

/// Base address of the object arena (`g_screen`), for handing the tree to the
/// AES via `wind_set(DESKWH, WF_NEWDESK, …)` and to `objc_draw` (`do_wredraw`).
/// Stable for the desktop's lifetime: the arena is allocated once by `obj_init`
/// and never reallocated (see [`DeskState::obj_ialloc`]), so this address — and
/// the `ob_spec`/`ib_ptext` raw pointers baked into the tree — stay valid across
/// the draw. [`None`] before [`init`].
///
/// The borrow ends when this returns; the AES background composite
/// (`w_drawdesk`/`paint_bg_clip`) reads the tree without borrowing [`DESK`],
/// so there is no re-entrancy.
pub fn screen_addr() -> Option<u64> {
    let guard = DESK.borrow();
    guard.as_ref().map(|s| s.g_screen.as_ptr() as u64)
}
