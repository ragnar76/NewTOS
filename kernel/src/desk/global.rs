//! `desk::global` — the desktop's process-lifetime state (`DeskState`, our
//! [`GLOBES G`]), the object arena, and the background pattern/colour.
//!
//! Reimplements EmuTOS `deskglob.c` + `GLOBES` (`deskbind.h:130-245`) and the
//! `g_screen[]`/`g_screeninfo[]` arena from `deskobj.c`, per
//! `docs/api/desktop/_data-model.md`. `GLOBES` never crosses an ABI boundary
//! ("Contract: none"), so [`DeskState`] is a clean Rust design: the arena is
//! a heap [`Vec`] and the window list lives in [`super::win::WindowList`].
//!
//! [`GLOBES G`]: https://github.com/emutos/emutos/blob/master/desk/deskbind.h

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU8, Ordering};

use tos_api::aes::object::{render_tree, Object};
use tos_api::vdi::backend::{ClipRect, RasterBackend};

use crate::sync::SpinMutex;

use super::model::{ScreenInfo, Sort, View};
use super::win::WindowList;

// ── Object-tree layout constants (`deskconf.h`, our own choice of indices
//    per `_data-model.md`: "which index the desktop uses is internal"). ──────

/// The "no object / end of list" sentinel (`OBJECT` `ob_*` = -1).
pub const NIL: i16 = -1;
/// `g_screen[]` index of the desktop pseudo-window's item root (`DROOT`).
pub const DROOT: i16 = 1;
/// Number of real window slots (`NUM_WNODES`, `deskconf.h`).
pub const NUM_WNODES: usize = 7;
/// First `g_screen[]` index available for item objects: `ROOT` (0) + `DROOT`
/// (1) + one box per possible window (`WOBS_START` = `NUM_WNODES + 2`).
pub const WOBS_START: i16 = NUM_WNODES as i16 + 2;

/// Menu-bar height in pixels (`gl_hbox`; NewTOS uses its fixed 8x16 font).
pub const MENU_H: i16 = 19;

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
/// rect is the desktop work area `{0, MENU_H, 1920, 1080-MENU_H}`; `wind_calc`
/// subtracts the `WINDOW_STYLE` borders `(l=1, t=37, r=19, b=19)`
/// (`tos-api::aes::window::borders`, `GAD=19`) → a work area of `1900 × 1005`.
/// `win_bldview` then fits `wfit = 1900/108 = 17` cols and `hfit = 1005/52 = 19`
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

/// `G_BOX` graphic type.
pub const G_BOX: u16 = 20;
/// `G_IBOX` (invisible box) graphic type — used for the arena's structural
/// container objects and the menu tree.
pub const G_IBOX: u16 = 25;
/// `G_STRING` graphic type — a menu item / label / text-view line.
pub const G_STRING: u16 = 28;
/// `G_ICON` graphic type — an `ICONBLK` object (desktop + window icon items).
pub const G_ICON: u16 = 31;
/// `G_TITLE` graphic type — a menu-bar title.
pub const G_TITLE: u16 = 32;

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
/// `ob_flags` `LASTOB` — marks the final object of a tree.
pub const LASTOB: u16 = 0x0020;
/// `ob_flags` `DISABLED` — a greyed / non-selectable object (menu separators).
pub const DISABLED: u16 = 0x0008;

/// A rectangle in absolute device pixels (EmuTOS `GRECT`, x/y/w/h).
#[derive(Clone, Copy, Default)]
pub struct Grect {
    /// Left edge.
    pub x: i16,
    /// Top edge.
    pub y: i16,
    /// Width in pixels.
    pub w: i16,
    /// Height in pixels.
    pub h: i16,
}

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
// Kept as atomics, separate from the mutex-guarded `DeskState`, because the
// re-entrant desktop-background painter (`set_desktop_painter` hook, invoked
// from inside the AES redraw path) must read it **lock-free** to avoid
// deadlocking against a lock the event loop might hold. Six bytes:
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
/// ([`super::inf::save_desktop`]).
pub fn patcol_all() -> [u8; 6] {
    let mut out = [0u8; 6];
    for (o, slot) in out.iter_mut().zip(G_PATCOL.iter()) {
        *o = slot.load(Ordering::Acquire);
    }
    out
}

/// The desktop's process-lifetime global state (EmuTOS `GLOBES G`).
pub struct DeskState {
    /// Current view mode (`g_iview`).
    pub view: View,
    /// Current sort key (`g_isort`).
    pub sort: Sort,
    /// "Size windows to fit" preference (`g_ifit`, `CONF_WITH_SIZE_TO_FIT`).
    pub size_to_fit: bool,

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

    /// Desktop work-area rectangle (screen minus the menu bar) (`g_desk`).
    pub g_desk: Grect,

    /// The current window's item-root object index (`g_croot`): [`DROOT`] for
    /// the desktop, else the window's `w_root`. Set by `desk_verify`; read by
    /// `men_update`/`hndl_button` to walk the current selection.
    pub g_croot: i16,
    /// The current window's AES handle (`g_cwin`); [`super::win::DESKWH`] for
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

    /// Parsed `NEWDESK.INF` desktop-icon grid positions, one per `#M`/`#T`/`#O`
    /// line: `(a_type code, drive letter, a_xspot, a_yspot)` in **grid units**.
    /// Consulted by [`DeskState::inf_desk_pos`] during `app_blddesk`. Empty when
    /// no INF file was loaded (icons then take their built-in default spots).
    pub inf_desk_icons: alloc::vec::Vec<(u8, u8, u8, u8)>,
    /// Parsed `NEWDESK.INF` window rectangles (`#W` lines, in file order,
    /// converted to pixels), indexed by window slot. Consulted when a directory
    /// window opens.
    pub inf_windows: alloc::vec::Vec<Grect>,
}

impl DeskState {
    /// Construct the desktop state for a `screen_w`x`screen_h` display, with
    /// the field defaults. The window list and object arena are set up
    /// afterwards by [`DeskState::win_start`] then [`DeskState::obj_init`]
    /// (see [`init`]).
    pub fn new(screen_w: i16, screen_h: i16) -> Self {
        Self {
            view: View::Icon,
            sort: Sort::Name,
            size_to_fit: true,
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
            g_desk: Grect { x: 0, y: MENU_H, w: screen_w, h: screen_h - MENU_H },
            g_croot: DROOT,
            g_cwin: super::win::DESKWH,
            g_wlastsel: super::win::DESKWH,
            g_screen: Vec::new(),
            g_screeninfo: Vec::new(),
            g_screenfree: NIL,
            windows: WindowList::default(),
            inf_desk_icons: Vec::new(),
            inf_windows: Vec::new(),
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

        // ROOT (index 0): full-screen container, parent of DROOT..last_box.
        screen[0] = Object {
            ob_next: NIL,
            ob_head: DROOT,
            ob_tail: last_box,
            ob_type: G_IBOX,
            ob_flags: 0,
            ob_state: 0,
            ob_spec: 0,
            ob_x: 0,
            ob_y: 0,
            ob_width: screen_w,
            ob_height: screen_h,
        };

        // DROOT (index 1): the desktop itself (its work-area rect).
        screen[DROOT as usize] = Object {
            ob_next: DROOT + 1,
            ob_head: NIL,
            ob_tail: NIL,
            ob_type: G_IBOX,
            ob_flags: 0,
            ob_state: 0,
            ob_spec: 0,
            ob_x: self.g_desk.x,
            ob_y: self.g_desk.y,
            ob_width: self.g_desk.w,
            ob_height: self.g_desk.h,
        };

        // One empty box per possible window (indices 2..=last_box). Each is a
        // sibling of DROOT; the last one points its `ob_next` back at ROOT and
        // carries LASTOB. Their rects are set by `win_alloc` when a window
        // opens (Task #6).
        for i in 0..NUM_WNODES {
            let idx = 2 + i;
            let is_last = i == NUM_WNODES - 1;
            screen[idx] = Object {
                ob_next: if is_last { 0 } else { (idx + 1) as i16 },
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

    /// Link `child` as the last child of `parent` (EmuTOS `ob_add`). The
    /// child's `ob_next` points back at the parent, the classic AES
    /// sibling-chain terminator that `objc_draw`/`objc_find` stop on when they
    /// reach the parent's `ob_tail`.
    fn ob_add(&mut self, parent: i16, child: i16) {
        self.g_screen[child as usize].ob_next = parent;
        if self.g_screen[parent as usize].ob_head == NIL {
            self.g_screen[parent as usize].ob_head = child;
        } else {
            let tail = self.g_screen[parent as usize].ob_tail;
            self.g_screen[tail as usize].ob_next = child;
        }
        self.g_screen[parent as usize].ob_tail = child;
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
}

/// The single desktop-state instance. Lazily initialised by [`init`]; guarded
/// by the kernel spinlock (single boot CPU, no `OnceCell`/`spin` dependency).
static DESK: SpinMutex<Option<DeskState>> = SpinMutex::new(None);

/// One-time desktop-state setup for a `screen_w`x`screen_h` display. Builds
/// [`DeskState`], runs `win_start()` (view/sort defaults + window list) then
/// `obj_init()` (the object arena), in that order. Idempotent — a second call
/// is a no-op.
pub fn init(screen_w: i16, screen_h: i16) {
    let mut guard = DESK.lock();
    if guard.is_none() {
        let mut state = DeskState::new(screen_w, screen_h);
        state.win_start();
        state.obj_init();
        *guard = Some(state);
    }
}

/// Run `f` against the desktop state under the lock. Returns `None` if the
/// state has not been [`init`]ialised yet. The consumer of the arena/window
/// APIs from later tasks (view build, event dispatch) goes through here.
pub fn with<R>(f: impl FnOnce(&mut DeskState) -> R) -> Option<R> {
    let mut guard = DESK.lock();
    guard.as_mut().map(f)
}

/// Base address of the object arena (`g_screen`), for handing the tree to
/// `objc_draw` / `render_tree`. Stable for the desktop's lifetime: the arena
/// is allocated once by `obj_init` and never reallocated (see
/// [`DeskState::obj_ialloc`]), so this address — and the `ob_spec`/`ib_ptext`
/// raw pointers baked into the tree — stay valid across the draw. [`None`]
/// before [`init`].
///
/// The lock is released when this returns; the caller uses the address in an
/// `objc_draw` that does not re-enter [`DESK`], so there is no re-entrancy on
/// the single boot CPU.
pub fn screen_addr() -> Option<u64> {
    let guard = DESK.lock();
    guard.as_ref().map(|s| s.g_screen.as_ptr() as u64)
}

/// Render the desktop-icon subtree (`DROOT` + children) clipped to `clip` ∩
/// the desktop work area, straight through `backend` (no AES trap) — the
/// desktop-icon half of the `crate::desk` background painter.
///
/// Runs **inside** `window::redraw_all` (the AES SVC handler), so it draws
/// via [`render_tree`] rather than a re-entrant `objc_draw` trap. It locks
/// [`DESK`]; this is deadlock-free because the desktop never holds that lock
/// across a `wind_*` call (the only thing that triggers `redraw_all`) — every
/// [`with`] closure completes and releases before any AES trap is issued.
pub fn render_desktop_icons(backend: &dyn RasterBackend, clip: ClipRect) {
    let guard = DESK.lock();
    let Some(s) = guard.as_ref() else {
        return;
    };
    let d = s.g_desk;
    // Intersect the caller's clip with the desktop work area.
    let x0 = clip.xmn.max(d.x);
    let y0 = clip.ymn.max(d.y);
    let x1 = clip.xmx.min(d.x + d.w - 1);
    let y1 = clip.ymx.min(d.y + d.h - 1);
    if x1 < x0 || y1 < y0 {
        return;
    }
    let tree = s.g_screen.as_ptr();
    // SAFETY: `tree` is a valid OBJECT array; `DROOT` is in range; the arena
    // is not reallocated while the lock is held.
    unsafe {
        render_tree(tree, DROOT, MAX_DEPTH, x0, y0, x1 - x0 + 1, y1 - y0 + 1, backend);
    }
}
