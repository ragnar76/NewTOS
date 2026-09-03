//! AES Window Manager (`wind_*`, opcodes 100–109) — Batch 1.
//!
//! Spec: [`docs/api/aes/window-graphics.md`]. Source: EmuTOS @ `1a7484ab`
//! `aes/gemwmlib.c` (`wm_*`, `wm_calc`, `w_getsize`). A window has a
//! **border** rectangle (the full frame) and a **work** rectangle (the
//! client area inside the gadgets); `wm_calc` converts between them from
//! the `kind` widget bitmask. The frame is drawn **procedurally** at our
//! altitude (each gadget rect computed from the border rect + `kind`),
//! rather than via EmuTOS's 19-object `W_ACTIVE` tree — same visible
//! frame and hit-testing without porting the tree machinery.
//!
//! **Batch-1 scope:** the window table + lifecycle (`create`/`open`/
//! `close`/`delete`), geometry (`calc`/`get`/`set`/`find`), and frame
//! drawing (border, title + name, closer/fuller/sizer, scroll arrows +
//! slider tracks). The `WM_*` message protocol (redraw/topped/moved/…),
//! the rectangle list (visible pieces), interactive drag/resize, and
//! `graf_*` are Batch 2+. No save/restore under windows yet (see the
//! Menu-Manager note); closing erases to the desktop background.

use super::object::{render_tree_app, render_tree_lp64, Object};
use super::Aespb;
use crate::bdos::process::Abi;
use crate::host::{host, Host};
use crate::vdi::backend::{ClipRect, Pel, PatternFill, RasterBackend, WriteMode};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicI16, AtomicU64, AtomicU8, Ordering};

// ── kind (border widget) bits, aesdefs.h:519-531 ──
const NAME: u16 = 0x0001;
const CLOSER: u16 = 0x0002;
const FULLER: u16 = 0x0004;
const MOVER: u16 = 0x0008;
const INFO: u16 = 0x0010;
const SIZER: u16 = 0x0020;
const UPARROW: u16 = 0x0040;
const DNARROW: u16 = 0x0080;
const VSLIDE: u16 = 0x0100;
const LFARROW: u16 = 0x0200;
const RTARROW: u16 = 0x0400;
const HSLIDE: u16 = 0x0800;

const VGADGETS: u16 = UPARROW | DNARROW | VSLIDE;
const HGADGETS: u16 = LFARROW | RTARROW | HSLIDE;
/// Top-row gadgets — EmuTOS `TGADGETS` (gemwmlib.h:16). **Includes `MOVER`**:
/// in the 3D branch a window with only a `MOVER` still gets a full title band
/// (the non-3D branch tests `NAME|CLOSER|FULLER` and gives it none).
const TGADGETS: u16 = NAME | CLOSER | FULLER | MOVER;

// ── WF_* fields, aesdefs.h:534-552 ──
// `WF_KIND` (1) is deliberately absent: `wm_get`/`wm_set` do not handle it in
// this EmuTOS build (both fall through to `return FALSE`, gemwmlib.c:1591,1745),
// so there is nothing for the dispatcher to match on.
const WF_NAME: i16 = 2;
const WF_INFO: i16 = 3;
const WF_WXYWH: i16 = 4;
const WF_CXYWH: i16 = 5;
const WF_HSLIDE: i16 = 8;
const WF_VSLIDE: i16 = 9;
const WF_HSLSIZ: i16 = 15;
const WF_VSLSIZ: i16 = 16;
const WF_PXYWH: i16 = 6;
const WF_FXYWH: i16 = 7;
const WF_TOP: i16 = 10;
const WF_FIRSTXYWH: i16 = 11;
const WF_NEXTXYWH: i16 = 12;
const WF_NEWDESK: i16 = 14;
const WF_SCREEN: i16 = 17;
const WF_COLOR: i16 = 18;
const WF_DCOLOR: i16 = 19;
const WF_OWNER: i16 = 20;

// ── wind_update modes, aesdefs.h:559-562 ──
const END_UPDATE: i16 = 0;
const BEG_UPDATE: i16 = 1;
const END_MCTRL: i16 = 2;
const BEG_MCTRL: i16 = 3;

/// Number of window gadget-colour slots — one per `W_*` gadget object (EmuTOS
/// `NUM_ELEM`, gemlib.h:58). `WF_COLOR`/`WF_DCOLOR` index the per-window /
/// global default colour-word arrays by gadget number.
const NUM_ELEM: usize = 19;

// The `W_*` gadget object indices (gemlib.h:38-56) the default colour init
// special-cases; only the named ones matter here.
const W_NAME_IDX: usize = 3;
const W_VSLIDE_IDX: usize = 12;
const W_HSLIDE_IDX: usize = 17;

/// Global default per-gadget colour words for a **topped** window (EmuTOS
/// `gl_wtcolor`, `wm_start` gemwmlib.c:1347-1354): every slot `0x1101`
/// (border+text black, transparent fill, pattern 0), with `W_NAME |= 0xa0` and
/// the two sliders `|= 0x10`. These are the AES colour-word values a
/// `wind_get(WF_DCOLOR)` must return 1:1; our procedural [`draw_frame`] does not
/// consume them (see the note there), so they are purely round-tripped state and
/// changing/adding them cannot alter a single pixel.
const GL_WTCOLOR_DEFAULT: [i16; NUM_ELEM] = {
    let mut a = [0x1101i16; NUM_ELEM];
    a[W_NAME_IDX] = 0x1101 | 0xa0; // 0x11a1
    a[W_VSLIDE_IDX] = 0x1101 | 0x10; // 0x1111
    a[W_HSLIDE_IDX] = 0x1101 | 0x10; // 0x1111
    a
};

/// Global default per-gadget colour words for an **untopped** window (EmuTOS
/// `gl_wbcolor`, `wm_start` gemwmlib.c:1348-1357): every slot `0x1101`, the two
/// sliders `|= 0x10` (3D object support on — our Falcon030/TOS4 target profile).
const GL_WBCOLOR_DEFAULT: [i16; NUM_ELEM] = {
    let mut a = [0x1101i16; NUM_ELEM];
    a[W_VSLIDE_IDX] = 0x1101 | 0x10; // 0x1111
    a[W_HSLIDE_IDX] = 0x1101 | 0x10; // 0x1111
    a
};

/// `objc_draw`/`ob_draw` recursion depth for the desktop-background composite
/// (EmuTOS `MAX_DEPTH`, `obdefs.h:30`): deep enough to draw the registered
/// desktop tree's `DROOT` box **and** every icon child under it.
const MAX_DEPTH: i16 = 8;

// ── wind_calc types ──
const WC_BORDER: i16 = 0;
const WC_WORK: i16 = 1;

// ── gadget glyphs (ST system font 0x01-0x07) ──
const G_UP: u16 = 0x01;
const G_DN: u16 = 0x02;
const G_RT: u16 = 0x03;
const G_LF: u16 = 0x04;
const G_CLOSE: u16 = 0x05;
const G_SIZE: u16 = 0x06;
const G_FULL: u16 = 0x07;

/// System-font cell width — EmuTOS `gl_wchar` (`gemgraf.c:405-410`): 8 for the
/// 8×16 desktop font, the value `graf_handle` reports (`aes/graf.rs:456`). Used
/// as the sizer drag's minimum window width for a gadget-less window
/// (`hctl_window`, `gemctrl.c:252`).
const GL_WCHAR: i16 = 8;

/// System-font cell height — EmuTOS `gl_hchar`: 16 for the 8×16 desktop font
/// (`gl_hbox = gl_hchar + 3 = 19`, `gemgraf.c:412`). The sizer drag's minimum
/// window height for a window without vertical gadgets (`gemctrl.c:253`).
const GL_HCHAR: i16 = 16;

/// Unadjusted gadget box width — EmuTOS `gl_wbox`
/// (`gl_wbox = max(gl_wchar+4, gl_hbox·aspect)`, `gemgraf.c:412-415`): 19 for
/// the 8×16 system font. This is what `graf_handle` reports (`gemsuper.c:287-291`)
/// and what the *glyph cell* of every window gadget measures — the 3D window
/// layout inflates the surrounding box ([`adj_wbox`]), never this.
pub(super) const GL_WBOX: i16 = 19;

/// Unadjusted gadget box height — EmuTOS `gl_hbox` (`gl_hchar + 3`,
/// `gemgraf.c:412`): 19 for the 8×16 system font. Also the menu-bar height
/// (`gl_rmenu`, `gemgraf.c:427` → `menu::BAR_H`).
pub(super) const GL_HBOX: i16 = 19;

/// Standard pixel adjustment for 3D objects — EmuTOS `ADJ3DSTD`
/// (`obdefs.h:136`). Our target profile is Falcon030/TOS4 with
/// `CONF_WITH_3D_OBJECTS` on (`config.h:1259`), so every window gadget sits in
/// a box grown by `2 * ADJ3DSTD` and is inset by `ADJ3DSTD` inside it.
const ADJ3DSTD: i16 = 2;

/// EmuTOS `MAX_COORDINATE` (`include/sysconf.h:63`) — the "no limit" extent the
/// Control Manager gives `gr_dragbox` for the vertical direction, so a window
/// can be dragged off the bottom of the screen.
const MAX_COORDINATE: i16 = 10000;

/// 3D-adjusted gadget box width — EmuTOS `adj_wbox`, set in `wm_init`
/// (`gemwmlib.c:1311-1324`): `gl_wbox + 2*ADJ3DSTD`, shrunk by 1 on a screen
/// narrower than 640 (and by 1 more when such a screen is at least 400 tall) to
/// mimic TOS4's ST-Low work area. Computed from the live screen instead of
/// cached at init; for a fixed resolution the value is the same either way.
fn adj_wbox() -> i16 {
    let mut a = GL_WBOX + 2 * ADJ3DSTD;
    let s = screen_rect();
    if s.w < 640 {
        a -= 1;
        if s.h >= 400 {
            a -= 1;
        }
    }
    a
}

/// 3D-adjusted gadget box height — EmuTOS `adj_hbox` (`gemwmlib.c:1312`):
/// `gl_hbox + 2*ADJ3DSTD`, with no resolution-dependent correction.
const fn adj_hbox() -> i16 {
    GL_HBOX + 2 * ADJ3DSTD
}

const WHITE: Pel = 0;
const BLACK: Pel = 1;
const GRAY_LT: Pel = 8; // 3D face (EmuTOS LWHITE)
const GRAY_DK: Pel = 9; // 3D shadow (EmuTOS LBLACK)

/// The classic GEM "move-bar" dither on an **active** window's name area
/// (EmuTOS `TOPPED_COLOR` = `IP_2PATT`, a light 25 % black stipple). The
/// inactive window's name bar is left plain — the presence of the stipple is
/// the active/inactive cue, matching EmuTOS.
const TITLE_STIPPLE: [u16; 4] = [0x0000, 0x5555, 0x0000, 0x5555];

/// Number of application window handles (1..=`MAX_WINDOWS`). EmuTOS sizes its
/// table with `NUM_WIN` (`sysconf.h:26-35`), *including* the desktop window; we
/// keep the eight application handles this AES has always offered and add the
/// desktop slot on top, so the count of usable handles does not change.
const MAX_WINDOWS: usize = 8;
/// Window-table size — the application handles **plus the desktop window at
/// handle 0** (EmuTOS `NUM_WIN`, `gemlib.h:136` `WINDOW w_win[NUM_WIN]`).
const NUM_WIN: usize = MAX_WINDOWS + 1;
const NAME_CAP: usize = 80;

/// The desktop window's AES handle (EmuTOS `DESKWH`) — the bottom of the window
/// z-order, and **a real window**: `wm_init` runs `w_setup(ppd, DESKWH, NONE)`
/// on it (`gemwmlib.c:1290`), which sets `VF_INUSE`, so `get_pwin(0)` succeeds
/// and `wm_get` answers every field for it exactly as for any other window. Its
/// slot is [`slots`]`[0]`, initialised by [`wm_init`].
const DESKWH: i16 = 0;

/// The desktop window's `kind` — EmuTOS `NONE` (`gemwmlib.c:1290`, no gadgets).
const NONE: u16 = 0;

/// A rectangle (GEM `GRECT`).
#[derive(Clone, Copy, Default, PartialEq)]
struct Grect {
    x: i16,
    y: i16,
    w: i16,
    h: i16,
}

/// One window slot.
#[derive(Clone, Copy)]
struct Window {
    in_use: bool,
    open: bool,
    kind: u16,
    z: i16,     // stacking order (higher = nearer the top)
    owner: i16, // owning process id (WM_* message target)
    full: Grect, // maximum extent (WF_FXYWH)
    curr: Grect, // current border rect (WF_CXYWH)
    prev: Grect, // previous border rect (WF_PXYWH)
    /// Work-area rectangle (`WF_WXYWH`) — **stored**, not derived, exactly like
    /// EmuTOS' `w_work`: `w_move` recomputes it with `wm_calc(WC_WORK, …)` from
    /// the new `WS_CURR` (`gemwmlib.c:1073-1077`), and `wm_init` sets the
    /// desktop window's to `gl_rfull` directly (`gemwmlib.c:1294`), which is
    /// **not** what `wm_calc` would produce for `kind = NONE`. Maintained by
    /// [`set_curr`].
    work: Grect,
    name: [u8; NAME_CAP],
    info: [u8; NAME_CAP],
    /// Vertical slider position (`WF_VSLIDE`, 1..1000 per-mille of travel)
    /// and thumb size (`WF_VSLSIZ`, 1..1000 per-mille of the track, or **−1**
    /// = "use default size", one gadget cell). `wind_create` inits the size to
    /// −1 (EmuTOS `w_vslsiz`, gemwmlib.c:232); the getter round-trips −1.
    vslide: i16,
    vslsize: i16,
    /// Horizontal slider position (`WF_HSLIDE`) and thumb size (`WF_HSLSIZ`),
    /// same per-mille encoding as the vertical pair (mirror axis); size −1 =
    /// default.
    hslide: i16,
    hslsize: i16,
    /// Per-gadget AES colour words for the topped / untopped states (EmuTOS
    /// `w_tcolor`/`w_bcolor`, gemlib.h:81-82). Set from the live global defaults
    /// at [`wind_create`], read/written by `WF_COLOR`. Round-tripped only — the
    /// procedural [`draw_frame`] renders the fixed TOS4 3D look and does not
    /// consume these, so they never affect rendering.
    tcolor: [i16; NUM_ELEM],
    bcolor: [i16; NUM_ELEM],
}

impl Window {
    const EMPTY: Window = Window {
        in_use: false,
        open: false,
        kind: 0,
        z: 0,
        owner: 0,
        full: Grect { x: 0, y: 0, w: 0, h: 0 },
        curr: Grect { x: 0, y: 0, w: 0, h: 0 },
        prev: Grect { x: 0, y: 0, w: 0, h: 0 },
        work: Grect { x: 0, y: 0, w: 0, h: 0 },
        name: [0; NAME_CAP],
        info: [0; NAME_CAP],
        vslide: 0,
        vslsize: -1,
        hslide: 0,
        hslsize: -1,
        tcolor: GL_WTCOLOR_DEFAULT,
        bcolor: GL_WBCOLOR_DEFAULT,
    };
}

/// The window table. Single boot CPU only → `UnsafeCell` with a hand-rolled
/// `Sync`, mirroring the cookie jar and AES process table.
struct WinTable {
    slots: UnsafeCell<[Window; NUM_WIN]>,
    next_z: UnsafeCell<i16>, // monotonically increasing stacking counter
}
// SAFETY: only the single boot CPU touches the AES; no concurrent access.
unsafe impl Sync for WinTable {}

static TABLE: WinTable = WinTable {
    slots: UnsafeCell::new([Window::EMPTY; NUM_WIN]),
    next_z: UnsafeCell::new(1),
};

/// Saved copy of the desktop's window set for the single-tasking **screen
/// handoff** ([`save_windows`] / [`restore_windows`]). Disjoint from [`TABLE`]:
/// [`save_windows`] snapshots the live table here and clears the live one so a
/// launched GEM app gets a clean window list; [`restore_windows`] copies it back
/// on the child's `Pterm`. `active` guards a restore with no prior save.
struct WinSave {
    slots: UnsafeCell<[Window; NUM_WIN]>,
    next_z: UnsafeCell<i16>,
    active: UnsafeCell<bool>,
}
// SAFETY: only the single boot CPU touches the AES; no concurrent access.
unsafe impl Sync for WinSave {}

static SAVE: WinSave = WinSave {
    slots: UnsafeCell::new([Window::EMPTY; NUM_WIN]),
    next_z: UnsafeCell::new(1),
    active: UnsafeCell::new(false),
};

/// Set once [`wm_init`] has built the desktop window's slot.
static WM_READY: AtomicBool = AtomicBool::new(false);

/// EmuTOS `wm_init` (`gemwmlib.c:1287-1294`), reduced to the part that is state
/// rather than object-tree bookkeeping: make handle 0 a **real** window.
///
/// ```c
/// w_setup(ppd, DESKWH, NONE);                 /* VF_INUSE, sliders 0/0/-1/-1 */
/// w_setsize(WS_CURR, DESKWH, &gl_rscreen);
/// w_setsize(WS_PREV, DESKWH, &gl_rscreen);
/// w_setsize(WS_FULL, DESKWH, &gl_rfull);
/// w_setsize(WS_WORK, DESKWH, &gl_rfull);
/// ```
///
/// `gl_rscreen` is the whole screen and `gl_rfull` the screen below the menu
/// row, both fixed at `gsx_start` (`gemgraf.c:423-424`) — i.e. after the
/// resolution is known. EmuTOS runs `wm_init` at AES start-up for the same
/// reason; we run it on first use of the table, which is the first AES window
/// call and therefore equally after the screen is up. `VF_ISOPEN` is *not* set
/// (the desktop is the tree root, never an open child), so every `in_use &&
/// open` walk skips it exactly as EmuTOS' `W_TREE` walks do.
///
/// This is the desktop-window half of EmuTOS's `wm_init`; the public
/// [`wm_init`] runs it together with the window sweep and the `gl_newdesk`
/// reset, which is the whole of `gemwmlib.c:1247-1310` that is observable here.
fn init_desk_window() {
    // SAFETY: single-CPU AES; exclusive logical owner, no live borrow here.
    let d = unsafe { &mut (*TABLE.slots.get())[DESKWH as usize] };
    *d = Window::EMPTY;
    d.in_use = true; // VF_INUSE
    d.kind = NONE;
    d.curr = screen_rect(); // gl_rscreen
    d.prev = screen_rect();
    d.full = desktop_work(); // gl_rfull
    d.work = desktop_work();
}

#[allow(clippy::mut_from_ref)]
fn slots() -> &'static mut [Window; NUM_WIN] {
    // EmuTOS runs `wm_init` after `gsx_start`, i.e. once the resolution is
    // known; we run it on first use of the table. Latch it only once a raster
    // backend actually exists — before that [`screen_rect`] would answer with
    // its 640×480 fallback and freeze the desktop window at the wrong size.
    if !WM_READY.load(Ordering::Relaxed) {
        init_desk_window();
        if host().and_then(|h| h.raster()).is_some() {
            WM_READY.store(true, Ordering::Relaxed);
        }
    }
    // SAFETY: single-CPU AES; exclusive logical owner.
    unsafe { &mut *TABLE.slots.get() }
}

/// Next stacking value (raises a window to the top of the z-order).
fn bump_z() -> i16 {
    // SAFETY: single-CPU AES.
    unsafe {
        let p = TABLE.next_z.get();
        let z = *p;
        *p = z + 1;
        z
    }
}

// ── wind_update state (P5-005) ───────────────────────────────────────────
//
// EmuTOS `wm_update` (gemwmlib.c:1778-1797) maintains two counters: the
// counting window-tree lock `wind_spb.sy_tas` (BEG/END_UPDATE) and the
// mouse-control-ownership counter `ml_ocnt` (BEG/END_MCTRL, via `fm_own`,
// gemfmlib.c:52). Single boot CPU, so plain `Relaxed` atomics suffice.

/// Counting window-tree lock — EmuTOS `wind_spb.sy_tas`. `BEG_UPDATE` takes it
/// (`tak_flag`), `END_UPDATE` releases it (`unsync`, clamped at 0). Single-
/// tasking has no contended waiter, so the take always succeeds; the count is
/// tracked so nested BEG/END pairs and [`wind_new`]'s release stay balanced.
static UPDATE_LOCK: AtomicI16 = AtomicI16::new(0);

/// Mouse-control-ownership counter — EmuTOS `ml_ocnt`. `BEG_MCTRL` increments,
/// `END_MCTRL` decrements. While `> 0` the app has taken over the mouse via
/// `wind_update(BEG_MCTRL)`, so the AES frame interception ([`intercept`])
/// switches off. This is the counter P4-011's full `fm_own` (menu-tree hold +
/// control-region transfer) builds on.
static MCTRL_CNT: AtomicI16 = AtomicI16::new(0);

/// Global default per-gadget colour words (`WF_DCOLOR`): the topped/untopped
/// defaults new windows inherit (EmuTOS `gl_wtcolor`/`gl_wbcolor`,
/// gemwmlib.c:138-139). Mutable so `wind_set(WF_DCOLOR)` can change them; single
/// boot CPU, so an `UnsafeCell` with a hand-rolled `Sync`, like [`TABLE`].
struct WinColors {
    tcolor: UnsafeCell<[i16; NUM_ELEM]>,
    bcolor: UnsafeCell<[i16; NUM_ELEM]>,
}
// SAFETY: only the single boot CPU touches the AES; no concurrent access.
unsafe impl Sync for WinColors {}

static GL_WCOLORS: WinColors = WinColors {
    tcolor: UnsafeCell::new(GL_WTCOLOR_DEFAULT),
    bcolor: UnsafeCell::new(GL_WBCOLOR_DEFAULT),
};

#[allow(clippy::mut_from_ref)]
fn gl_wtcolor() -> &'static mut [i16; NUM_ELEM] {
    // SAFETY: single-CPU AES; exclusive logical owner.
    unsafe { &mut *GL_WCOLORS.tcolor.get() }
}

#[allow(clippy::mut_from_ref)]
fn gl_wbcolor() -> &'static mut [i16; NUM_ELEM] {
    // SAFETY: single-CPU AES; exclusive logical owner.
    unsafe { &mut *GL_WCOLORS.bcolor.get() }
}

/// Clamp a `WF_COLOR`/`WF_DCOLOR` gadget number to a valid array index. EmuTOS
/// indexes `w_tcolor`/`w_bcolor` with the raw `pinwds[0]` unchecked
/// (gemwmlib.c:1568); we bound it so a bogus gadget can't index out of range and
/// panic (fatal in `no_std`).
fn gadget_index(g: i16) -> usize {
    g.clamp(0, NUM_ELEM as i16 - 1) as usize
}

/// Handle of the top-most open window (highest `z`), or 0 (desktop).
fn get_top() -> i16 {
    let mut best = 0i16;
    let mut best_z = i16::MIN;
    for (i, w) in slots().iter().enumerate() {
        if w.in_use && w.open && w.z >= best_z {
            best_z = w.z;
            best = i as i16;
        }
    }
    best
}

/// Handle of the bottom-most open window (lowest `z`), or −1 (`NIL`) if none is
/// open — EmuTOS `W_TREE[ROOT].ob_head`, the first/oldest child of the window
/// tree root (bottom of the z-order), reported by `WF_OWNER` (gemwmlib.c:1588).
fn get_bottom() -> i16 {
    let mut best = -1i16;
    let mut best_z = i16::MAX;
    for (i, w) in slots().iter().enumerate() {
        if w.in_use && w.open && w.z <= best_z {
            best_z = w.z;
            best = i as i16;
        }
    }
    best
}

/// Raise `handle` to the top of the stack — EmuTOS `wm_mktop`→`draw_change`,
/// **top-request branch** (`gemwmlib.c:1600-1611,1112-1136`).
///
/// A top request repaints exactly two things and nothing else: the old top's
/// frame with *deactivated* decorations and the new top's frame with activated
/// ones, each clipped to its own owner-rectangle list ([`draw_frame_pieces`] =
/// `w_cpwalk(…, MAX_DEPTH, TRUE)`→`do_walk`, `:363-376,295-311`). In particular
/// it does **not** repaint the desktop background: `start` stays `w_handle`, so
/// `draw_change`'s `if (start == DESKWH) w_drawdesk(&c)` never fires
/// (`:1215-1219`) — and it does not touch any window's client region, because
/// `W_DATA`/`W_WORK` are borderless `G_IBOX`es that `ob_draw` skips
/// (`:67-68,87-88`, `aes/gemoblib.c:663-670`). Both windows keep their content.
///
/// Whether the newly-topped window is asked to redraw depends on the `VF_BROKEN`
/// bookkeeping: if the window was unobstructed before the reorder (`wasclr`)
/// *and* the old top is unobstructed after it (`clrold`), nothing was uncovered
/// and `draw_change` returns after the two border draws (`:1128-1135`).
/// Otherwise it falls through to `w_update(start = w_handle, …, stop = DESKWH)`,
/// whose single loop turn re-draws that one window's border and calls
/// `w_redraw`, posting one `WM_REDRAW` for `pt ∩ WS_WORK ∩ union(rlist)`
/// (`:1004-1048,887-907`) — with the window now on top, its whole work area.
fn top_window(handle: i16) {
    let old_top = get_top(); // current top before the bump (may equal handle)
    // Not a top request at all: `draw_change` returns immediately when the
    // window is already `W_TREE[ROOT].ob_tail` (`gemwmlib.c:1117-1119`).
    if old_top == handle {
        return;
    }
    // `wasclr` is read *before* the reorder, from the `VF_BROKEN` flag the
    // previous `newrect` pass left (`gemwmlib.c:1066`).
    let wasclr = !is_broken(handle);
    if let Some(w) = win(handle) {
        w.z = bump_z();
    }
    let Some(backend) = host().and_then(|h| h.raster()) else {
        return;
    };
    // Counting cursor bracket around the frame repaints — our cursor is a
    // save-under sprite, so painting under a shown sprite would leave its
    // backing store stale (see [`compose`]; EmuTOS has the same `gsx_moff`
    // bracket inside `w_update`, `gemwmlib.c:1011/1047`).
    crate::vdi::cursor_hide(backend);
    // "draw oldtop covered with deactivated borders" (`gemwmlib.c:1120-1126`).
    // `get_top` answers `DESKWH` when no window was topped (EmuTOS `w_top`,
    // `:315-318`); the desktop window has no frame to deactivate, and EmuTOS
    // then takes `clrold = TRUE` (`:1125`).
    let clrold = if old_top != DESKWH {
        draw_frame_pieces(old_top, false, backend, None);
        // `clrold` is read *after* the rectangle lists were rebuilt with the
        // new order (`:1122`), i.e. with `handle` already above `old_top`.
        !is_broken(old_top)
    } else {
        true
    };
    // The new top's frame, activated — `w_cpwalk(gl_wtop, 0, MAX_DEPTH, TRUE)`
    // in the clear case (`:1133`), `w_update`'s `w_cpwalk` in the other
    // (`:1029`); both draw the same frame clipped to the same rectangle list.
    draw_frame_pieces(handle, true, backend, None);
    crate::vdi::cursor_show(backend);
    if !(clrold && wasclr) {
        if let Some(r) = win(handle).map(|w| w.curr) {
            send_redraw_in(handle, r);
        }
    }
}

/// `VF_BROKEN` (`aes/gemlib.h:62`) — "the window is overlapped, can't be
/// blitted": `newrect`/`mkrect`/`brkrct` set the flag on a window whenever a
/// window **above** it breaks one of its owner rectangles
/// (`aes/gemwrect.c:100-160,163-200`), and `draw_change` clears + rebuilds the
/// whole set with `everyobj(gl_wtree, ROOT, NIL, newrect, …)` (`gemwmlib.c:1088`)
/// before reading it. Since the flag is a pure function of the current z-order
/// and geometry, it is computed on demand here instead of being cached: a
/// window is broken exactly when some open window above it overlaps its border
/// rectangle.
fn is_broken(handle: i16) -> bool {
    let Some((rect, myz)) = win(handle).filter(|w| w.open).map(|w| (w.curr, w.z)) else {
        return false;
    };
    slots()
        .iter()
        .any(|ow| ow.in_use && ow.open && ow.z > myz && intersect_grect(ow.curr, rect).is_some())
}

/// `handle`'s **owner rectangle list** — EmuTOS `D.w_win[wh].w_rlist`: the
/// window's own `WS_TRUE` rectangle with the rectangle of every window above it
/// broken out of it, rebuilt by `newrect`/`mkrect`/`brkrct`
/// (`aes/gemwrect.c:64-205`) at the top of every `draw_change`
/// (`aes/gemwmlib.c:1129`), i.e. before anything is drawn. Returns the piece
/// count; `out[..n]` holds them.
///
/// This is *the* list the whole window manager clips against: `do_walk`
/// ([`walk_rects`]) walks it for both `w_drawdesk` and `w_cpwalk`, and
/// `WF_FIRSTXYWH` reports it intersected with the work rectangle
/// ([`compute_rlist`]).
///
/// [`DESKWH`] (the desktop window) is the **bottom** of the stack: its own
/// rectangle is the whole screen (`w_setsize(WS_CURR, DESKWH, &gl_rscreen)`,
/// `aes/gemwmlib.c:1290`) and every open window is above it (`i16::MIN` z), so
/// the loop subtracts them all. The menu row is kept out by `do_walk`'s
/// `gl_rfull` clip, not by this list — exactly as in EmuTOS.
///
/// A closed or invalid handle owns nothing (`newrect` returns early for a
/// zero-size window, `aes/gemwrect.c:189-192`).
fn border_pieces(handle: i16, out: &mut [Grect; RCAP]) -> usize {
    let (rect, myz) = match win(handle) {
        Some(w) if handle == DESKWH => (w.curr, i16::MIN),
        Some(w) if w.open => (w.curr, w.z),
        _ => return 0,
    };
    *out = [Grect::default(); RCAP];
    out[0] = rect;
    let mut n = 1;
    for ow in slots().iter() {
        if ow.in_use && ow.open && ow.z > myz {
            n = subtract(out, n, ow.curr);
        }
    }
    n
}

/// EmuTOS `do_walk` (`aes/gemwmlib.c:286-312`), reduced to the rectangles it
/// would `gsx_sclip` and draw in:
///
/// ```c
/// if (pc) rc_intersect(&gl_rfull, pc); else pc = &gl_rfull;
/// for (po = D.w_win[wh].w_rlist; po; po = po->o_link) {
///     rc_copy(&po->o_gr, &t);
///     if (rc_intersect(pc, &t)) { gsx_sclip(&t); ob_draw(tree, obj, depth); }
/// }
/// ```
///
/// `pc` = `None` is the C `pc == NULL` case (clip to `gl_rfull` alone). Returns
/// the count; `out[..n]` are the clip rectangles, each of which the caller draws
/// its tree in once. An empty result means the caller draws nothing at all —
/// which is the whole point: a window that owns no visible piece, or a desktop
/// wholly covered by windows, paints nothing.
fn walk_rects(wh: i16, pc: Option<Grect>, out: &mut [Grect; RCAP]) -> usize {
    // `rc_intersect(&gl_rfull, pc)` — `gl_rfull` is the screen minus the menu
    // row (`aes/gemgraf.c:423-424`, set at `gsx_start` whether or not a menu bar
    // is installed), so no `do_walk` ever touches the menu strip.
    let full = desktop_work();
    let pc = match pc {
        Some(p) => match intersect_grect(p, full) {
            Some(t) => t,
            None => return 0,
        },
        None => full,
    };
    let mut list = [Grect::default(); RCAP];
    let n = border_pieces(wh, &mut list);
    let mut m = 0usize;
    for r in list.iter().take(n) {
        if let Some(t) = intersect_grect(*r, pc) {
            out[m] = t;
            m += 1;
        }
    }
    m
}

/// Draw `handle`'s frame once per visible piece of its owner rectangle list —
/// EmuTOS `w_cpwalk(wh, 0, MAX_DEPTH, …)`→`do_walk` (`gemwmlib.c:363-379`,
/// `:286-312`), which walks `D.w_win[wh].w_rlist` and `ob_draw`s the window
/// element tree inside each rectangle. Clipping the draw to the rectangle list
/// is what stops a window that is *below* another one from painting its frame
/// over it. `istop` selects the activated / deactivated decorations
/// (`w_bldactive`'s `istop`, `:495-560`).
///
/// `pc` is `w_cpwalk`'s clip: `Some(damage)` for the `usetrue == FALSE` call
/// inside `w_update`, which runs under `gsx_sclip(pt)` and picks the current
/// clip up again with `gsx_gclip` (`:1029-1031,368-374`); `None` for the
/// `usetrue == TRUE` calls, whose `WS_TRUE` clip is already implied by the
/// rectangle list (every piece is inside the window).
fn draw_frame_pieces(handle: i16, istop: bool, backend: &dyn RasterBackend, pc: Option<Grect>) {
    let mut pieces = [Grect::default(); RCAP];
    let n = walk_rects(handle, pc, &mut pieces);
    if n == 0 {
        return;
    }
    let Some(w) = win(handle) else {
        return;
    };
    for r in pieces.iter().take(n) {
        draw_frame(w, istop, backend, rect_clip(r.x, r.y, r.w, r.h));
    }
}

/// The registered desktop-background object tree (EmuTOS `gl_newdesk`,
/// `gemwmlib.c:54`): the base VA of the desktop application's own `OBJECT`
/// array, set via `wind_set(DESKWH, WF_NEWDESK, tree, 1, root)`. `0` = none →
/// [`paint_bg_clip`] draws the AES-internal fallback root instead.
///
/// The registering caller records its origin + ABI in [`NEWDESK_APP`] /
/// [`NEWDESK_ABI`] so the later background composite reads the tree at the right
/// record width and treats a `G_USERDEF` callback at the right privilege. The
/// NewTOS desktop registers its own **native (LP64) kernel-built** `g_screen`
/// (drawn via `render_tree_lp64`); an EL0 GEM app (TeraDesk) registers an
/// **app-provided** tree in EL0 memory (drawn via `render_tree_app`). It stays
/// stable for its owner's lifetime (the arena is never reallocated), so holding
/// the raw pointer across draws is sound.
static NEWDESK_TREE: AtomicU64 = AtomicU64::new(0);

/// The object index within [`NEWDESK_TREE`] that the background composite draws
/// from (EmuTOS `gl_newroot`, `gemwmlib.c:55`; the desktop passes its `DROOT`).
static NEWDESK_ROOT: AtomicI16 = AtomicI16::new(0);

/// `true` iff the tree in [`NEWDESK_TREE`] was supplied by an **EL0 GEM app**
/// (e.g. TeraDesk via `wind_set(WF_NEWDESK)`), `false` for the kernel/desktop's
/// own native `g_screen`. Recorded in [`set_newdesk`] from whether a user
/// process is running at registration time (`process::current()` non-null).
/// Drives [`paint_bg_clip`]: an app tree is drawn as **not** kernel-built so a
/// `G_USERDEF`'s EL0 `ub_code` is deferred by `ob_user` instead of being called
/// at EL1 (the WF_NEWDESK crash: TeraDesk `ub_bckbox` → PXN instruction abort).
static NEWDESK_APP: AtomicBool = AtomicBool::new(false);

/// The ABI ([`Abi`] as `u8`) of the caller that registered [`NEWDESK_TREE`],
/// captured in [`set_newdesk`] via `current_abi()`. The background composite
/// runs later — possibly when a different (or no) process is current — so the
/// tree's record width must be replayed from this captured value rather than
/// re-read from `current_abi()` at draw time. A kernel/desktop registration
/// records `Lp64`. Only consulted when [`NEWDESK_APP`] is `true`.
static NEWDESK_ABI: AtomicU8 = AtomicU8::new(Abi::Lp64 as u8);

/// `wind_set(DESKWH, WF_NEWDESK, …)` handler (EmuTOS `wm_set`,
/// `gemwmlib.c:1685-1689`): register (non-null tree) or un-register (null tree)
/// the desktop's background object tree + root object. Never fails.
///
/// The tree pointer + root arrive by the same dual convention as `WF_NAME`
/// ([`wf_str_ptr`], D-094): the native desktop passes the tree in `addr_in[0]`
/// (`ain_len == 1`) with the root in `int_in[2]`; a classic AES caller packs the
/// tree pointer into `int_in[2..3]` with the root in `int_in[4]` (`pinwds[2]`).
fn set_newdesk(pb: &Aespb) {
    let tree = wf_str_ptr(pb);
    let root = if pb.ain_len() >= 1 { pb.intin(2) } else { pb.intin(4) };
    if tree == 0 {
        NEWDESK_TREE.store(0, Ordering::Release);
    } else {
        // Capture the tree's origin + ABI at registration time; the background
        // composite runs later, when a different (or no) process may be current.
        // A running user process (`current()` non-null) means an EL0 GEM app is
        // registering its own background tree — its `G_USERDEF` `ub_code` lives
        // in EL0 and must not be up-called from EL1; a null `current()` is the
        // kernel/desktop registering its native `g_screen`. `current_abi()`
        // records the record width now (the app is current at this instant).
        let is_app = !crate::bdos::process::current().is_null();
        NEWDESK_APP.store(is_app, Ordering::Release);
        NEWDESK_ABI.store(crate::bdos::process::current_abi() as u8, Ordering::Release);
        // NEWDESK_ROOT is taken verbatim from caller input (EmuTOS trusts the
        // desktop's `gl_newroot`/`DROOT` the same way, gemwmlib.c:1685). A
        // negative index is defused downstream (`render_tree_*` bail on
        // `start < 0`); an out-of-range positive
        // index cannot be validated here without the tree's object count, which
        // WF_NEWDESK does not carry — left as the caller's contract.
        NEWDESK_ROOT.store(root, Ordering::Release);
        NEWDESK_TREE.store(tree, Ordering::Release);
    }
}

/// Build the AES-internal background **fallback root** — EmuTOS
/// `gl_wtree[ROOT]` (= `W_TREE[ROOT]`), the object `w_drawdesk` draws when no
/// desktop tree is registered ([`NEWDESK_TREE`] == 0, `gemwmlib.c:340-359`).
///
/// `wm_init` builds that root as a `G_BOX` with `NONE` flags and `NORMAL` state
/// and copies its `ob_spec` **from the AES's own built-in DESKTOP tree**
/// (`W_TREE[ROOT].ob_spec = rs_trees[DESKTOP]->ob_spec`,
/// `gemwmlib.c:1268-1271`), i.e. the resource constant `4419L` = `0x1143` —
/// pattern 4 in colour 3, the classic green dither (`gem_rsc.c:551-556`) — with
/// its low byte replaced by the `.INF` `#Q` background if there was one
/// (`set_aes_background`, `geminit.c:635-641`, which runs *before* `wm_start`,
/// `geminit.c:719-730`). [`super::init::desktop_root_obspec`] holds exactly that
/// word, so read it here rather than caching a copy.
///
/// It is **not** white: the white screen belongs to `sh_draw(cmd, TRUE)`, which
/// swaps the DESKTOP root's `ob_spec` for `CLEAR_SCREEN`, draws once and puts
/// the old spec straight back (`gemshlib.c:269-286`) — see
/// [`super::shell::sh_draw_clear`].
///
/// Its geometry is the desktop window's `WS_CURR` = `gl_rscreen`, the whole
/// screen (`gemwmlib.c:1290`); built fresh per call so it always spans the
/// current framebuffer.
fn fallback_root(w: i16, h: i16) -> Object {
    Object {
        ob_next: -1,
        ob_head: -1,
        ob_tail: -1,
        ob_type: 20, // G_BOX
        ob_flags: 0,
        ob_state: 0,
        ob_spec: super::init::desktop_root_obspec(),
        ob_x: 0,
        ob_y: 0,
        ob_width: w,
        ob_height: h,
    }
}

/// Screen handoff — **relinquish** the desktop's windows: snapshot the whole
/// shared window table into [`SAVE`] and clear the live table (every desktop
/// window is hidden, its z-order counter reset). The saved state is *preserved*,
/// not destroyed, so [`restore_windows`] can rebuild it exactly; the launched
/// GEM app then gets a fresh window list (all handles free), composites over no
/// desktop windows, and its own `wind_*` calls run against the empty table.
///
/// This mirrors EmuTOS tearing the desktop's windows (`wm_init`) when a child is
/// launched. It is called from the desktop's launch path (kernel) together with
/// the `wind_set(DESKWH, WF_NEWDESK, NULL, …)` un-registration and the menu-bar
/// removal; the caller then clears the screen with
/// [`super::shell::sh_draw_clear`] (EmuTOS `sh_draw(cmd, TRUE)`).
pub fn save_windows() {
    // Make sure the desktop window's slot exists before it is snapshotted.
    let live = slots();
    // SAFETY: single-CPU AES; `SAVE` and `TABLE` are disjoint statics and
    // `[Window; NUM_WIN]` is `Copy`, so these are plain array copies.
    unsafe {
        *SAVE.slots.get() = *live;
        *SAVE.next_z.get() = *TABLE.next_z.get();
        *SAVE.active.get() = true;
        // Hide every desktop window by resetting the live table (its state is
        // held in `SAVE`); reset the z counter for the child. Slot 0 — the
        // desktop *window* — is left alone: EmuTOS' `wm_new` likewise deletes
        // only handles 1.. (`gemwmlib.c:1886`); the desktop window is never
        // torn down.
        live[1..].fill(Window::EMPTY);
        *TABLE.next_z.get() = 1;
    }
}

/// Screen handoff — **resume** the desktop's windows: restore the set saved by
/// [`save_windows`] back into the shared window table, discarding whatever the
/// child left there. A no-op if no set is currently saved. Called from the
/// desktop's launch path on the child's `Pterm`, alongside re-registering the
/// desktop background tree (`wind_set(DESKWH, WF_NEWDESK, tree, …)`) and a
/// repaint.
pub fn restore_windows() {
    // SAFETY: single-CPU AES; disjoint `Copy` statics (see [`save_windows`]).
    unsafe {
        if !*SAVE.active.get() {
            return;
        }
        *TABLE.slots.get() = *SAVE.slots.get();
        *TABLE.next_z.get() = *SAVE.next_z.get();
        *SAVE.active.get() = false;
    }
}

/// Paint the desktop background within `clip` — NewTOS's `w_drawdesk`
/// (EmuTOS `gemwmlib.c:340-359`), which is a single
/// `do_walk(DESKWH, tree, root, depth, pc)` (`gemwmlib.c:286-312`):
///
/// ```c
/// if (pc) rc_intersect(&gl_rfull, pc); else pc = &gl_rfull;
/// for (po = D.w_win[wh].w_rlist; po; po = po->o_link) {
///     rc_copy(&po->o_gr, &t);
///     if (rc_intersect(pc, &t)) { gsx_sclip(&t); ob_draw(tree, obj, depth); }
/// }
/// ```
///
/// So the background is **not** painted over the damage rectangle. It is painted
/// over the damage rectangle intersected with `gl_rfull` (the screen minus the
/// menu row) and then with each piece of the **desktop window's owner rectangle
/// list** — its own rectangle minus every window above it, rebuilt by `newrect`
/// (`gemwrect.c:165-205`) at the top of `draw_change` (`gemwmlib.c:1129`), i.e.
/// before anything is drawn. An open window owns its rectangle; the desktop does
/// not, so `w_drawdesk` can never paint inside one. What a window shows is
/// restored by its owner in answer to the `WM_REDRAW` that `w_update`/`w_redraw`
/// post (`gemwmlib.c:1004-1048,887-907`) — the AES does not paint it.
///
/// Painting the whole clip instead put the desktop background — box *and* icons —
/// inside every window a damage rectangle reached across, on `wind_open`,
/// `wind_close`, `wind_set(WF_CURRXYWH)`, the mover drag and
/// `form_dial(FMD_FINISH)`, until the owner answered its `WM_REDRAW`.
///
/// Runs **inside** `compose`/`redraw_all` (the AES SVC handler), so it must not
/// re-enter the AES trap: [`draw_desk_tree`] draws straight through the backend.
fn paint_bg_clip(backend: &dyn RasterBackend, clip: ClipRect) {
    let mut pieces = [Grect::default(); RCAP];
    let n = walk_rects(DESKWH, Some(clip_grect(clip)), &mut pieces);
    for r in pieces.iter().take(n) {
        draw_desk_tree(backend, rect_clip(r.x, r.y, r.w, r.h));
    }
}

/// `ob_draw(tree, obj, depth)` for the desktop background tree, clipped to
/// `clip` — the body of `w_drawdesk`'s `do_walk` iteration
/// (`gemwmlib.c:340-359,306-308`).
///
/// If a desktop tree is registered ([`NEWDESK_TREE`] != 0, via
/// `wind_set(DESKWH, WF_NEWDESK, …)`), draw that tree from its root
/// ([`NEWDESK_ROOT`], the desktop's `DROOT`) at **`MAX_DEPTH`** — the `DROOT`
/// background box **and every icon child** — exactly as EmuTOS draws
/// `gl_newdesk` at `MAX_DEPTH`. Otherwise draw the AES-internal [`fallback_root`]
/// — the built-in DESKTOP root's own box — at depth 0, exactly as EmuTOS draws
/// `gl_wtree` at depth 0. Both go through the ordinary object
/// manager ([`render_tree_lp64`] = the non-trapping `ob_draw` engine), clipped
/// to `clip` — never a bespoke fill.
fn draw_desk_tree(backend: &dyn RasterBackend, clip: ClipRect) {
    let cw = (clip.xmx - clip.xmn + 1).max(0);
    let ch = (clip.ymx - clip.ymn + 1).max(0);
    if cw == 0 || ch == 0 {
        return;
    }
    let tree = NEWDESK_TREE.load(Ordering::Acquire);
    if tree != 0 {
        let root = NEWDESK_ROOT.load(Ordering::Acquire);
        if NEWDESK_APP.load(Ordering::Acquire) {
            // App-supplied background tree (EL0 GEM app, e.g. TeraDesk): read it
            // at the ABI captured at registration and treat it as NOT
            // kernel-built, so a `G_USERDEF`'s EL0 `ub_code` is deferred by
            // `ob_user` rather than called at EL1 (→ PXN instruction abort at
            // the app callback's VA, the WF_NEWDESK crash). The boxes/images of
            // the tree still draw; the app userdef simply is not invoked (an
            // EL0-upcall mechanism is out of scope — the goal is: no crash).
            let abi = Abi::from_u8(NEWDESK_ABI.load(Ordering::Acquire));
            // SAFETY: `tree` is the app's registered background-tree base — a
            // valid OBJECT array (at `abi` width) kept stable for its lifetime;
            // `root` in range (caller contract, see `set_newdesk`).
            unsafe {
                render_tree_app(tree as *const Object, root, MAX_DEPTH, clip.xmn, clip.ymn, cw, ch, abi, backend);
            }
        } else {
            // Kernel/desktop-built native `g_screen` (LP64 records); a
            // `G_USERDEF` here is a real EL1 callback (`kernel_built = true`).
            // SAFETY: `tree` is the desktop's `g_screen` base, registered via
            // WF_NEWDESK; a valid native OBJECT array kept stable for the
            // desktop's lifetime, `root` in range.
            unsafe {
                render_tree_lp64(tree as *const Object, root, MAX_DEPTH, clip.xmn, clip.ymn, cw, ch, backend);
            }
        }
    } else {
        let fb = fallback_root(backend.width() as i16, backend.height() as i16);
        // SAFETY: `fb` is a single valid native OBJECT (root index 0, no children).
        unsafe {
            render_tree_lp64(&fb as *const Object, 0, 0, clip.xmn, clip.ymn, cw, ch, backend);
        }
    }
}

/// A `ClipRect` for the rectangle `[x, x+w) × [y, y+h)`.
fn rect_clip(x: i16, y: i16, w: i16, h: i16) -> ClipRect {
    ClipRect { xmn: x, ymn: y, xmx: x + w - 1, ymx: y + h - 1 }
}

/// The inverse of [`rect_clip`] — a `ClipRect` (inclusive bounds, the VDI
/// convention) as a `GRECT` (origin + size, the AES convention), so the
/// rectangle algebra EmuTOS does on `GRECT`s can be done on a clip.
fn clip_grect(c: ClipRect) -> Grect {
    Grect {
        x: c.xmn,
        y: c.ymn,
        w: (c.xmx - c.xmn + 1).max(0),
        h: (c.ymx - c.ymn + 1).max(0),
    }
}

/// Repaint the desktop and every open window back-to-front (painter's
/// algorithm — front windows overwrite the ones they cover). The simple
/// stand-in for per-window rectangle-list clipping when redrawing frames;
/// the rectangle list (`WF_FIRSTXYWH`) still drives what the *app* redraws.
fn redraw_all(backend: &dyn RasterBackend) {
    compose(backend, ClipRect::full(backend.width(), backend.height()));
}

/// The desktop-background + window-frame composite, confined to `clip`. Paints
/// the desktop background (painter hook) then every open window's frame
/// back-to-front (painter's algorithm — front windows overwrite the ones they
/// cover), each clipped to `clip`. [`redraw_all`] passes the full-device clip;
/// [`redraw_clip`] passes a damage rectangle so a move/size settle repaints only
/// the affected area instead of the whole screen. The window **content** (item
/// objects) is re-laid by the owner afterwards, exactly as in the full path.
fn compose(backend: &dyn RasterBackend, clip: ClipRect) {
    // Bracket the whole window redraw with the **counting** cursor semaphore,
    // exactly as EmuTOS `w_update` wraps the frame walk in `gsx_moff()` /
    // `gsx_mon()` (gemwmlib.c:1011/1047). The cursor is a save-under sprite:
    // painting the desktop background + frames over the shown sprite would
    // leave its backing store stale, so the next cursor move restores old
    // pixels — the reported rectangular cursor "dropping". Hide it once here
    // (its backing is restored first), repaint, then show it on top of the
    // fresh pixels. The count makes any nested composite (an outer
    // `graf_mouse(M_OFF)` desktop bracket, or a re-entrant redraw) safe.
    crate::vdi::cursor_hide(backend);
    paint_bg_clip(backend, clip);
    let top = get_top();
    // Selection sort by ascending z, drawing each open window once.
    let mut drawn = [false; NUM_WIN];
    loop {
        let mut pick = usize::MAX;
        let mut pick_z = i16::MAX;
        for (i, w) in slots().iter().enumerate() {
            if w.in_use && w.open && !drawn[i] && w.z <= pick_z {
                // pick the lowest z not yet drawn; tie-break stable
                if pick == usize::MAX || w.z < pick_z {
                    pick = i;
                    pick_z = w.z;
                }
            }
        }
        if pick == usize::MAX {
            break;
        }
        drawn[pick] = true;
        let istop = pick as i16 == top;
        // `w_update`'s per-window step: `gsx_sclip(pt); w_cpwalk(top, 0,
        // MAX_DEPTH, FALSE)` (`gemwmlib.c:1029-1030`) — the frame is drawn once
        // per piece of *that window's* owner rectangle list inside the damage
        // rectangle, never over the whole damage rectangle. Without the list a
        // covered window repainted its frame on top of the window covering it.
        draw_frame_pieces(pick as i16, istop, backend, Some(clip_grect(clip)));
    }
    crate::vdi::cursor_show(backend);
}

/// Damage-scoped repaint of the desktop background + every window frame, clipped
/// to `clip` (a border-rect union). The public entry the desktop drives after a
/// solid window move/resize (WM_MOVED/WM_SIZED) and the programmatic WF_CXYWH
/// path use, in place of [`redraw`] (whole-screen `redraw_all`) — only the union
/// of the window's old and new border rects is repainted, which removes the
/// full-screen flicker. The window content is re-laid by the desktop
/// (`do_wredraw`) / by the `send_redraw` WM_REDRAW that follows.
pub fn redraw_clip(x: i16, y: i16, w: i16, h: i16) {
    if w <= 0 || h <= 0 {
        return;
    }
    if let Some(backend) = host().and_then(|h| h.raster()) {
        let cr = ClipRect { xmn: x, ymn: y, xmx: x + w - 1, ymx: y + h - 1 };
        compose(backend, cr);
    }
}

/// The bounding rectangle enclosing both `a` and `b` — the damage region of a
/// window move/resize (old border rect ∪ new border rect).
fn union_grect(a: Grect, b: Grect) -> Grect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = (a.x + a.w).max(b.x + b.w);
    let y1 = (a.y + a.h).max(b.y + b.h);
    Grect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
}

/// The intersection of `a` and `b`, or `None` when they don't overlap — the
/// per-window clip EmuTOS's `w_redraw` computes with `rc_intersect`
/// (`gemwmlib.c:897,903`).
fn intersect_grect(a: Grect, b: Grect) -> Option<Grect> {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.w).min(b.x + b.w);
    let y1 = (a.y + a.h).min(b.y + b.h);
    (x1 > x0 && y1 > y0).then_some(Grect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 })
}

// ── rectangle list (visible pieces) ──────────────────────────────────────

const RCAP: usize = 16;
const WM_REDRAW: i16 = 20;

/// Iterator state for `WF_FIRSTXYWH`/`WF_NEXTXYWH` — the visible pieces of
/// the window most recently queried. Single-CPU, like the window table.
struct RList {
    rects: UnsafeCell<[Grect; RCAP]>,
    count: UnsafeCell<usize>,
    cursor: UnsafeCell<usize>,
}
unsafe impl Sync for RList {}
static RLIST: RList = RList {
    rects: UnsafeCell::new([Grect { x: 0, y: 0, w: 0, h: 0 }; RCAP]),
    count: UnsafeCell::new(0),
    cursor: UnsafeCell::new(0),
};

fn put(out: &mut [Grect; RCAP], m: &mut usize, r: Grect) {
    if r.w > 0 && r.h > 0 && *m < RCAP {
        out[*m] = r;
        *m += 1;
    }
}

/// Subtract covering rect `c` from each rect in `list[0..n]`, producing the
/// non-overlapping remainder. Returns the new count.
fn subtract(list: &mut [Grect; RCAP], n: usize, c: Grect) -> usize {
    let mut out = [Grect::default(); RCAP];
    let mut m = 0usize;
    for r in list.iter().take(n).copied() {
        // No intersection → keep whole.
        if c.x >= r.x + r.w || c.x + c.w <= r.x || c.y >= r.y + r.h || c.y + c.h <= r.y {
            put(&mut out, &mut m, r);
            continue;
        }
        if r.y < c.y {
            put(&mut out, &mut m, Grect { x: r.x, y: r.y, w: r.w, h: c.y - r.y });
        }
        if r.y + r.h > c.y + c.h {
            put(&mut out, &mut m, Grect { x: r.x, y: c.y + c.h, w: r.w, h: (r.y + r.h) - (c.y + c.h) });
        }
        let oy = r.y.max(c.y);
        let oh = (r.y + r.h).min(c.y + c.h) - oy;
        if r.x < c.x {
            put(&mut out, &mut m, Grect { x: r.x, y: oy, w: c.x - r.x, h: oh });
        }
        if r.x + r.w > c.x + c.w {
            put(&mut out, &mut m, Grect { x: c.x + c.w, y: oy, w: (r.x + r.w) - (c.x + c.w), h: oh });
        }
    }
    *list = out;
    m
}

/// `handle`'s owner rectangle list ([`border_pieces`]) intersected with its work
/// area — what `wm_get(WF_FIRSTXYWH/WF_NEXTXYWH)` reports. EmuTOS walks the same
/// list and clips each piece to `WS_WORK` on the way out (`w_owns` against the
/// window's work rectangle, `gemwmlib.c:1556-1561`, `:1226-1239`).
///
/// Returns the piece count; `out[..n]` holds them. Pieces that are pure border
/// (no work-area overlap) drop out.
fn work_rects(handle: i16, out: &mut [Grect; RCAP]) -> usize {
    let Some(work) = win(handle).map(|w| w.work) else {
        return 0;
    };
    let mut list = [Grect::default(); RCAP];
    let n = border_pieces(handle, &mut list);
    let mut m = 0usize;
    for r in list.iter().take(n) {
        if let Some(t) = intersect_grect(*r, work) {
            out[m] = t;
            m += 1;
        }
    }
    m
}

/// Load `handle`'s [`work_rects`] into `RLIST`, cursor reset — the
/// `WF_FIRSTXYWH`/`WF_NEXTXYWH` iterator's backing store (EmuTOS keeps the list
/// itself and a `w_rnext` cursor per window, `gemwmlib.c:1556-1561`).
fn compute_rlist(handle: i16) {
    // SAFETY: single-CPU AES.
    let (rects, count, cursor) = unsafe {
        (&mut *RLIST.rects.get(), &mut *RLIST.count.get(), &mut *RLIST.cursor.get())
    };
    *cursor = 0;
    let mut list = [Grect::default(); RCAP];
    let n = work_rects(handle, &mut list);
    rects[..n].copy_from_slice(&list[..n]);
    *count = n;
}

/// Next piece from `RLIST` (empty `Grect` when exhausted).
fn rlist_next() -> Grect {
    // SAFETY: single-CPU AES.
    unsafe {
        let count = *RLIST.count.get();
        let cursor = &mut *RLIST.cursor.get();
        if *cursor < count {
            let r = (*RLIST.rects.get())[*cursor];
            *cursor += 1;
            r
        } else {
            Grect::default()
        }
    }
}

/// Post a `WM_REDRAW` for `rect` to `handle`'s owner process.
fn send_redraw(handle: i16, rect: Grect) {
    let owner = match win(handle) {
        Some(w) => w.owner,
        None => return,
    };
    let mut msg = [0i16; 8];
    msg[0] = WM_REDRAW;
    msg[1] = super::appl::current_pid();
    msg[3] = handle;
    msg[4] = rect.x;
    msg[5] = rect.y;
    msg[6] = rect.w;
    msg[7] = rect.h;
    // Enqueue into the owner's FIFO — EmuTOS `w_redraw`→`ap_sendmsg`→`ap_rdwr`
    // →`doq` (`gemwmlib.c:887-907,851-863`, `gemqueue.c`). Same-window redraws
    // coalesce (rect-union) instead of overwriting, so no content redraw is
    // lost when several are posted before the event loop drains them.
    // SAFETY: post_msg hands the message to the live process slot on the
    // single trap path.
    unsafe {
        super::pd::post_msg(owner, msg);
    }
}

/// Bounding rectangle of `handle`'s currently-visible work-area pieces — its
/// work rect minus every higher-z open window — i.e. EmuTOS `w_union` over the
/// owner rectangle list (`w_union`/`w_owns`/`w_rlist`, `gemwmlib.c:869-884`).
/// Empty when the window is closed or fully covered. Computed the same way as
/// [`compute_rlist`], but returning only the enclosing rectangle and **without**
/// disturbing the shared `WF_FIRSTXYWH`/`WF_NEXTXYWH` iterator state ([`RLIST`]).
fn visible_bound(handle: i16) -> Grect {
    let mut list = [Grect::default(); RCAP];
    let n = work_rects(handle, &mut list);
    let mut acc: Option<Grect> = None;
    for r in list.iter().take(n) {
        acc = Some(match acc {
            Some(a) => union_grect(a, *r),
            None => *r,
        });
    }
    acc.unwrap_or_default()
}

/// Post a `WM_REDRAW` to `handle`'s owner for the intersection of `damage` with
/// the window's now-visible area — EmuTOS `w_redraw`: `pt` ∩ work ∩
/// `w_union(w_rlist)` (`gemwmlib.c:887-907`). No message when the intersection
/// is empty (the window is not exposed inside `damage`).
fn send_redraw_in(handle: i16, damage: Grect) {
    if let Some(rect) = intersect_grect(damage, visible_bound(handle)) {
        send_redraw(handle, rect);
    }
}

/// EmuTOS `w_update`→`w_redraw` fan-out (`gemwmlib.c:1004-1048,887-907`): walk
/// every still-open window and post a `WM_REDRAW` for the part of its
/// now-visible area inside the exposed `damage` region. A window whose visible
/// region grew because a removed/lowered window no longer covers it thus
/// repaints its newly-exposed content; a window `damage` does not reach gets
/// nothing (empty intersection → no message).
///
/// Every posted `WM_REDRAW` is appended to the owner's per-app FIFO queue
/// (`Aespd::queue`), so when several **same-owner** windows are uncovered at
/// once each one's redraw is delivered (in FIFO order) — none is overwritten.
/// Redraws for the *same* window coalesce via the queue's rectangle-union
/// (EmuTOS `doq`, `gemqueue.c:45-56`).
fn broadcast_redraw(damage: Grect) {
    for handle in 1..NUM_WIN as i16 {
        if win(handle).map(|w| w.open).unwrap_or(false) {
            send_redraw_in(handle, damage);
        }
    }
}

/// Map a window handle to its table slot, if valid and in use — EmuTOS
/// `get_pwin` (`gemwmlib.c:1391-1404`): the handle indexes the table directly
/// and **0 is a legal handle** (the desktop window, `VF_INUSE` since
/// [`wm_init`]); only a negative handle, one past the table, or a slot without
/// `VF_INUSE` fails.
fn win(handle: i16) -> Option<&'static mut Window> {
    if handle < 0 || handle as usize >= NUM_WIN {
        return None;
    }
    let w = &mut slots()[handle as usize];
    w.in_use.then_some(w)
}

/// Move a window: store the new border rect and recompute the work rect from
/// it — EmuTOS `w_move` (`gemwmlib.c:1069-1077`), the *only* place besides
/// `wm_init`/`wm_delete` where `WS_WORK` is written.
fn set_curr(w: &mut Window, r: Grect) {
    w.curr = r;
    w.work = calc(WC_WORK, w.kind, r);
}

// ── geometry ────────────────────────────────────────────────────────────

/// Which scroll bars a `kind` gets after EmuTOS' `SIZER` conflict resolution —
/// `(havevbar, havehbar)`, `wm_calc`'s 3D branch (`gemwmlib.c:1823-1834`).
///
/// A `SIZER` on its own asks for *both* bars; when the window already has real
/// gadgets on an axis the `SIZER` is stripped from both requests and only the
/// axes with gadgets keep a bar. If stripping leaves nothing, the vertical bar
/// is forced back on — TOS4 draws the sizer in a vertical bar when the window
/// has no other elements. This is the rule the non-3D branch does not have, and
/// it is why `SIZER|HGADGETS` grows only the bottom border while `SIZER` alone
/// grows only the right one.
fn bars(kind: u16) -> (bool, bool) {
    let mut havevbar = kind & (VGADGETS | SIZER);
    let mut havehbar = kind & (HGADGETS | SIZER);
    if havevbar != 0 && havehbar != 0 {
        havevbar &= !SIZER;
        havehbar &= !SIZER;
        if havevbar == 0 && havehbar == 0 {
            havevbar |= SIZER;
        }
    }
    (havevbar != 0, havehbar != 0)
}

/// Border thickness contributed by `kind` on each side: `(left, top,
/// right, bottom)` — EmuTOS `wm_calc`, **3D branch** (`gemwmlib.c:1817-1840`),
/// the branch our Falcon030/TOS4 profile compiles (`CONF_WITH_3D_OBJECTS`,
/// `config.h:1259`).
///
/// Four things differ from the `#else` branch: the title band is
/// `adj_hbox - 1` (not `gl_hbox - 1`) thick, `MOVER` alone earns that band
/// (`TGADGETS`), an `INFO` line costs a full `gl_hbox` (not `gl_hbox - 1`), and
/// the scroll bars are decided by [`bars`]' strip-and-force rule instead of a
/// plain `SIZER` test on both axes.
fn borders(kind: u16) -> (i16, i16, i16, i16) {
    let lb = 1;
    let mut tb = 1;
    let mut rb = 1;
    let mut bb = 1;
    if kind & TGADGETS != 0 {
        tb += adj_hbox() - 1;
    }
    if kind & INFO != 0 {
        tb += GL_HBOX;
    }
    let (havevbar, havehbar) = bars(kind);
    if havevbar {
        rb += adj_wbox() - 1;
    }
    if havehbar {
        bb += adj_hbox() - 1;
    }
    (lb, tb, rb, bb)
}

/// Convert a rectangle between border and work coordinates (`wm_calc`).
fn calc(wtype: i16, kind: u16, r: Grect) -> Grect {
    let (mut lb, mut tb, mut rb, mut bb) = borders(kind);
    if wtype == WC_BORDER {
        lb = -lb;
        tb = -tb;
        rb = -rb;
        bb = -bb;
    }
    Grect {
        x: r.x + lb,
        y: r.y + tb,
        w: r.w - lb - rb,
        h: r.h - tb - bb,
    }
}

// ── frame drawing ───────────────────────────────────────────────────────

fn rect_outline(r: Grect, backend: &dyn RasterBackend, clip: ClipRect) {
    backend.fill_rect(r.x, r.y, r.w, 1, BLACK, clip); // top
    backend.fill_rect(r.x, r.y + r.h - 1, r.w, 1, BLACK, clip); // bottom
    backend.fill_rect(r.x, r.y, 1, r.h, BLACK, clip); // left
    backend.fill_rect(r.x + r.w - 1, r.y, 1, r.h, BLACK, clip); // right
}

/// Draw one bordered gadget cell with a centred glyph (closer/fuller/
/// arrows/sizer). `sel` inverts it (pressed feedback).
fn gadget_sel(r: Grect, ch: u16, sel: bool, backend: &dyn RasterBackend, clip: ClipRect) {
    // Modern 3D gadget: grey face + bevel (raised, recessed when pressed).
    backend.fill_rect(r.x, r.y, r.w, r.h, GRAY_LT, clip);
    bevel(r, !sel, backend, clip);
    let gx = r.x + (r.w - 8) / 2;
    let gy = r.y + (r.h - 16) / 2;
    backend.draw_glyph(gx, gy, crate::vdi::font::glyph(ch), BLACK, WriteMode::Transparent, clip);
}

/// Draw a 2-pixel 3D bevel: `raised` = white top/left + dark-grey
/// bottom/right; recessed swaps them.
fn bevel(r: Grect, raised: bool, backend: &dyn RasterBackend, clip: ClipRect) {
    if r.w < 2 || r.h < 2 {
        return;
    }
    let (tl, br) = if raised { (WHITE, GRAY_DK) } else { (GRAY_DK, WHITE) };
    for i in 0..2 {
        backend.fill_rect(r.x + i, r.y + i, r.w - 2 * i, 1, tl, clip);
        backend.fill_rect(r.x + i, r.y + i, 1, r.h - 2 * i, tl, clip);
        backend.fill_rect(r.x + i, r.y + r.h - 1 - i, r.w - 2 * i, 1, br, clip);
        backend.fill_rect(r.x + r.w - 1 - i, r.y + i, 1, r.h - 2 * i, br, clip);
    }
}

fn gadget(r: Grect, ch: u16, backend: &dyn RasterBackend, clip: ClipRect) {
    gadget_sel(r, ch, false, backend, clip);
}

/// Draw text left-aligned within a band, one 8×16 glyph per byte.
fn draw_label(x: i16, y: i16, max_w: i16, s: &[u8], backend: &dyn RasterBackend, clip: ClipRect) {
    let cy = y + 1;
    let mut cx = x;
    for &b in s {
        if b == 0 || cx + 8 > x + max_w {
            break;
        }
        backend.draw_glyph(cx, cy, crate::vdi::font::glyph(b as u16), BLACK, WriteMode::Transparent, clip);
        cx += 8;
    }
}

/// Draw the full window frame for `w` (border + gadgets + work area), confined
/// to `clip`. Callers repainting the whole screen pass the full-device clip;
/// the damage-scoped composite ([`redraw_clip`]) passes the union of the moved
/// window's old and new border rects, so a settle-overlap repaint never clears
/// a work area (or overdraws a frame) outside the damage region.
fn draw_frame(w: &Window, istop: bool, backend: &dyn RasterBackend, clip: ClipRect) {
    let b = w.curr;
    let kind = w.kind;

    // Outer frame.
    rect_outline(b, backend, clip);

    let ah = adj_hbox();
    let aw = adj_wbox();
    let mut top = b.y + 1; // running top of the next band

    // Title row (name area + closer + fuller). EmuTOS `w_bldactive`'s 3D branch
    // (gemwmlib.c:515-560): the title box is `adj_hbox` tall, each gadget is a
    // `gl_wbox` × `gl_hbox` cell inset by `ADJ3DSTD`, and the closer/fuller
    // advance the name area by a full `adj_wbox`. `MOVER` alone earns the band
    // (`TGADGETS`) — the same test [`borders`] uses, so band and work area agree.
    // Ours: white base, the name area carries a light dither on the *active*
    // window (the classic move-bar look), plain on the inactive one.
    if kind & TGADGETS != 0 {
        let bar = Grect { x: b.x + 1, y: top, w: b.w - 2, h: ah - 1 };
        backend.fill_rect(bar.x, bar.y, bar.w, bar.h, WHITE, clip);

        let gy = b.y + ADJ3DSTD;
        let mut name_x = b.x;
        let mut name_w = b.w;
        if kind & CLOSER != 0 {
            let g = Grect { x: b.x + ADJ3DSTD, y: gy, w: GL_WBOX, h: GL_HBOX };
            gadget(g, G_CLOSE, backend, clip);
            name_x += aw;
            name_w -= aw;
        }
        if kind & FULLER != 0 {
            name_w -= aw;
            let g = Grect { x: name_x + name_w + ADJ3DSTD, y: gy, w: GL_WBOX, h: GL_HBOX };
            gadget(g, G_FULL, backend, clip);
        }
        // W_NAME sits `ADJ3DSTD` inside what is left (gemwmlib.c:534-537).
        name_x += ADJ3DSTD;
        name_w -= 2 * ADJ3DSTD;
        // Active window: the classic move-bar dither behind the name.
        if istop {
            backend.fill_pattern(
                name_x,
                gy,
                name_w,
                GL_HBOX,
                PatternFill { fg: BLACK, bg: WHITE, pattern: &TITLE_STIPPLE, mode: WriteMode::Replace },
                clip,
            );
        }
        // Window name (centred when there is room), on a small white box so it
        // stays crisp over the dither (EmuTOS clears the name's glyph cells).
        let nlen = name_len(&w.name) as i16 * 8;
        let nx = name_x + ((name_w - nlen) / 2).max(2);
        if istop && nlen > 0 {
            backend.fill_rect((nx - 3).max(name_x), gy, nlen + 6, GL_HBOX, WHITE, clip);
        }
        draw_label(nx, gy, name_w, &w.name, backend, clip);
        // The title box's own bottom border row. `W_TITLE` is a `G_BOX` with
        // `ob_spec 0x00011101` — border thickness 1 (`gemwmlib.c:63,84`) — and
        // `w_bldactive` gives it `y = 0, h = adj_hbox` inside `W_BOX`
        // (`:514-517`), so that border's bottom row is `b.y + adj_hbox - 1`:
        // exactly one row **above** the first row `wm_calc` hands to the work
        // area (`tb = 1 + (adj_hbox - 1)`, `:1808-1810`). Drawing it a row lower
        // put frame ink inside the client region on every frame redraw.
        backend.fill_rect(b.x, b.y + ah - 1, b.w, 1, BLACK, clip);
        top += ah - 1;
    }

    // Info line — `gl_hbox` tall, *not* adjusted (gemwmlib.c:562-567), which is
    // why [`borders`] charges a full `gl_hbox` for it.
    if kind & INFO != 0 {
        let band = Grect { x: b.x + 1, y: top, w: b.w - 2, h: GL_HBOX };
        backend.fill_rect(band.x, band.y, band.w, band.h, GRAY_LT, clip);
        draw_label(band.x + 2, top, band.w - 2, &w.info, backend, clip);
        // Same rule as the title band: `W_INFO` is `gl_hbox` tall starting at
        // `b.y + adj_hbox` (`w_bldactive`, `gemwmlib.c:561-567`) and its
        // `gl_ainfo` TEDINFO has `te_thickness = 1` (`:153-157`), so its bottom
        // border row is the band's last row — `wm_calc`'s work area starts on
        // the row after it (`tb += gl_hbox`, `:1811-1812`).
        backend.fill_rect(b.x, top + GL_HBOX - 1, b.w, 1, BLACK, clip);
        top += GL_HBOX;
    }

    // The work area is deliberately **not** painted here. In EmuTOS the window
    // frame is an OBJECT tree (`gl_awind`) in which `W_BOX`, `W_DATA` and
    // `W_WORK` all have the type `G_IBOX` (`aes/gemwmlib.c:62-69`), and
    // `W_DATA`/`W_WORK` carry `ob_spec == 0x00001101` — border thickness 0
    // (`:87-88`). `ob_draw` skips the border of a zero-thickness object
    // (`if (th != 0) gr_box(...)`) and the interior fill of a `G_IBOX`
    // (`if (obtype != G_IBOX) gr_inside(...)`), `aes/gemoblib.c:663-670`. So a
    // frame redraw — a topping, a `WF_NAME`/`WF_INFO` change, a composite —
    // never touches one pixel of the client region: it belongs to the owner,
    // which repaints it on `WM_REDRAW`. Filling it white here erased the
    // content of every window whose info line was rewritten (TeraDesk's
    // `dir_info` does that on every selection change) or that was topped.

    // Right scroll bar (arrows + track + thumb). Fill the bar column first —
    // its gadgets are `gl_wbox` × `gl_hbox` cells inside an `adj_wbox`-wide
    // column, so without a background the `ADJ3DSTD` margins around them would
    // show the desktop through. Stop one column short of the window's right
    // edge (and one row short of its bottom) so the box border stays black.
    // Like the bottom band below, the fill lives here and not in
    // [`draw_vslider`], so the slider-only redraw path never erases the sizer
    // corner it does not repaint.
    if let Some(r) = vbar_rect(b, kind) {
        let h = (r.y + r.h).min(b.y + b.h - 1) - r.y;
        backend.fill_rect(r.x, r.y, r.w - 1, h, GRAY_LT, clip);
    }
    draw_vslider(w, backend, clip);

    // Bottom scroll bar (arrows + track + thumb). Fill the whole bottom band
    // first, so the corner cell next to the right bar is painted even when the
    // band carries no gadgets. The band fill lives here (not in
    // `draw_hslider`) so the slider-only redraw path never erases the sizer
    // corner it does not repaint. The band exists exactly when [`borders`]
    // charged for it — otherwise the work area reaches the bottom edge and a
    // band fill here would paint over the client's rectangle.
    let (havevbar, havehbar) = bars(kind);
    if havehbar {
        let row_y = b.y + b.h - ah;
        backend.fill_rect(b.x + 1, row_y, b.w - 2, ah - 1, GRAY_LT, clip);
    }
    draw_hslider(w, backend, clip);

    // Corner cell (EmuTOS `W_SIZER`, gemwmlib.c:612-627): drawn whenever the
    // window has both bars or a `SIZER`, `gl_wbox` × `gl_hbox` inset by
    // `ADJ3DSTD` from the bottom-right corner of the data area. The sizer
    // *glyph* is added only for a real `SIZER` (`ob_spec |= 0x06000000`).
    if (havevbar && havehbar) || kind & SIZER != 0 {
        let g = Grect {
            x: b.x + b.w - GL_WBOX - ADJ3DSTD,
            y: b.y + b.h - GL_HBOX - ADJ3DSTD,
            w: GL_WBOX,
            h: GL_HBOX,
        };
        if kind & SIZER != 0 {
            gadget(g, G_SIZE, backend, clip);
        } else {
            backend.fill_rect(g.x, g.y, g.w, g.h, GRAY_LT, clip);
            bevel(g, true, backend, clip);
        }
    }
}

/// Elevator (thumb) geometry within a scroll track `track_len` px long:
/// returns `(offset, len)` — the thumb starts `offset` px into the track and
/// spans `len` px. `slsize` = `WF_?SLSIZ` (per-mille of the track, or **−1** =
/// "use default size" = one gadget cell), `slpos` = `WF_?SLIDE` (per-mille of
/// the remaining travel). The single source of truth for the thumb, shared by
/// the slider draw ([`draw_vslider`]/[`draw_hslider`]) and the frame hit-test /
/// drag ([`gadget_at`]/[`drag_elevator`]) so the clicked thumb is exactly the
/// drawn one. Matches EmuTOS `w_bldvbar`/`w_bldhbar` (`gemwmlib.c:634-708`):
/// `size = gl_hbox` when `w_vslsiz == -1`, else per-mille of the track.
fn thumb_geom(track_len: i16, slsize: i16, slpos: i16) -> (i16, i16) {
    let min_thumb = GL_HBOX.min(track_len);
    let len = if slsize == -1 {
        min_thumb
    } else {
        ((track_len as i32 * slsize.clamp(1, 1000) as i32 / 1000) as i16).clamp(min_thumb, track_len)
    };
    let travel = track_len - len;
    let pos = slpos.clamp(0, 1000);
    let offset = (travel as i32 * pos as i32 / 1000) as i16;
    (offset, len)
}

/// First row of the window's work area — the running band top after the title
/// and info bands. Identical to `calc(WC_WORK, …).y` by construction (both add
/// [`borders`]' `tb`), which is what keeps the painted bands and the reported
/// work rectangle edge-to-edge.
fn band_top(b: Grect, kind: u16) -> i16 {
    let mut top = b.y + 1;
    if kind & TGADGETS != 0 {
        top += adj_hbox() - 1;
    }
    if kind & INFO != 0 {
        top += GL_HBOX;
    }
    top
}

/// Screen rectangle of the vertical scroll-bar column — EmuTOS `W_VBAR`
/// (`gemwmlib.c:408`, placed at `gemwmlib.c:606-610`): `adj_wbox` wide against
/// the window's right edge, from the first work row down to the top of the
/// horizontal band. `None` when [`bars`] gave the window no vertical bar.
fn vbar_rect(b: Grect, kind: u16) -> Option<Grect> {
    let (havevbar, havehbar) = bars(kind);
    if !havevbar {
        return None;
    }
    let aw = adj_wbox();
    let top = band_top(b, kind);
    let bot = b.y + b.h - 1 - if havehbar { adj_hbox() } else { 0 };
    (bot >= top).then_some(Grect { x: b.x + b.w - aw, y: top, w: aw, h: bot - top + 1 })
}

/// Screen rectangle of the horizontal scroll-bar band — EmuTOS `W_HBAR`
/// (`gemwmlib.c:446`, placed at `gemwmlib.c:612-616`): `adj_hbox` tall against
/// the window's bottom edge, from the left border to the vertical bar column.
/// `None` when [`bars`] gave the window no horizontal bar.
fn hbar_rect(b: Grect, kind: u16) -> Option<Grect> {
    let (havevbar, havehbar) = bars(kind);
    if !havehbar {
        return None;
    }
    let ah = adj_hbox();
    let left = b.x + 1;
    let right = b.x + b.w - 1 - if havevbar { adj_wbox() } else { 0 };
    (right >= left).then_some(Grect { x: left, y: b.y + b.h - ah, w: right - left + 1, h: ah })
}

/// The vertical scroll track span `(top, length)` — the region left for the
/// thumb + paging after the up/down arrows — or `None` if there is no VSLIDE
/// track. EmuTOS `w_bldvbar` (3D, `gemwmlib.c:401-436`): each arrow consumes a
/// full `adj_hbox` of the bar, and what remains is the slide box. Shared with
/// the hit-test / drag ([`gadget_at`]/[`drag_elevator`]) so they agree
/// pixel-for-pixel with the draw.
fn vtrack(w: &Window) -> Option<(i16, i16)> {
    let kind = w.kind;
    if kind & VSLIDE == 0 {
        return None;
    }
    let bar = vbar_rect(w.curr, kind)?;
    let ah = adj_hbox();
    let top = bar.y + if kind & UPARROW != 0 { ah } else { 0 };
    let len = bar.h
        - if kind & UPARROW != 0 { ah } else { 0 }
        - if kind & DNARROW != 0 { ah } else { 0 };
    (len > 0).then_some((top, len))
}

/// Screen rectangle of a scroll-bar elevator (thumb) — EmuTOS `W_VELEV` /
/// `W_HELEV` (`gemwmlib.c:436`, `:474`): inset `ADJ3DSTD` inside the slide box
/// on the cross axis at `gl_?box`, and `2*ADJ3DSTD + 1` shorter than the slide
/// length along the drag axis. `track` is the `(start, length)` from
/// [`vtrack`]/[`htrack`], `bar` the enclosing bar column/band.
fn elev_rect(bar: Grect, track: (i16, i16), off: i16, len: i16, vert: bool) -> Grect {
    let (start, _) = track;
    let thick = (len - 2 * ADJ3DSTD - 1).max(1);
    if vert {
        Grect { x: bar.x + ADJ3DSTD, y: start + off + ADJ3DSTD, w: GL_WBOX, h: thick }
    } else {
        Grect { x: start + off + ADJ3DSTD, y: bar.y + ADJ3DSTD, w: thick, h: GL_HBOX }
    }
}

/// Draw the right (vertical) scroll bar — up/down arrows, the track, and a
/// thumb positioned + sized from `w.vslide` / `w.vslsize` (both per-mille).
/// Shared by `draw_frame` and `wind_set(WF_VSLIDE/WF_VSLSIZ)`; drawing only
/// the slider column leaves the work area (content) untouched.
fn draw_vslider(w: &Window, backend: &dyn RasterBackend, clip: ClipRect) {
    let kind = w.kind;
    let Some(bar) = vbar_rect(w.curr, kind) else {
        return;
    };
    let ah = adj_hbox();
    // Arrows: a `gl_wbox` × `gl_hbox` cell inset `ADJ3DSTD` into the bar, each
    // consuming a full `adj_hbox` of it. The down arrow's extra −1 is EmuTOS'
    // (`y+h+ADJ3DSTD-1`, gemwmlib.c:424).
    if kind & UPARROW != 0 {
        let g = Grect { x: bar.x + ADJ3DSTD, y: bar.y + ADJ3DSTD, w: GL_WBOX, h: GL_HBOX };
        gadget(g, G_UP, backend, clip);
    }
    if kind & DNARROW != 0 {
        let g = Grect { x: bar.x + ADJ3DSTD, y: bar.y + bar.h - ah + 1, w: GL_WBOX, h: GL_HBOX };
        gadget(g, G_DN, backend, clip);
    }
    if let Some((track_y, track_h)) = vtrack(w) {
        let track = Grect { x: bar.x, y: track_y, w: bar.w, h: track_h };
        backend.fill_rect(track.x, track.y, track.w, track.h, GRAY_LT, clip);
        rect_outline(track, backend, clip);
        // Thumb geometry shared with `gadget_at`/`drag_elevator` (see
        // [`thumb_geom`]): a stored −1 size renders as one gadget cell, never a
        // zero/negative-height elevator; the stored/returned value stays −1.
        let (off, thumb_h) = thumb_geom(track_h, w.vslsize, w.vslide);
        let e = elev_rect(bar, (track_y, track_h), off, thumb_h, true);
        backend.fill_rect(e.x, e.y, e.w, e.h, GRAY_LT, clip);
        bevel(e, true, backend, clip);
    }
}

/// Redraw just window `handle`'s vertical slider (no work-area clear), for
/// `wind_set(WF_VSLIDE/WF_VSLSIZ)` — EmuTOS `wm_set`'s
/// `w_cpwalk(w_handle, W_VSLIDE, MAX_DEPTH, TRUE)`
/// (`gemwmlib.c:1704-1717,1739-1754`), so `do_walk` clips it to the window's
/// own owner rectangle list rather than letting it draw over a window on top.
fn redraw_vslider(handle: i16) {
    if let (Some(backend), Some(w)) = (host().and_then(|h| h.raster()), win(handle)) {
        if w.open {
            let mut pieces = [Grect::default(); RCAP];
            let n = walk_rects(handle, None, &mut pieces);
            // Bracket the slider repaint with the counting cursor semaphore
            // (EmuTOS `w_update` moff/mon) so a cursor over the scroll bar
            // leaves no dropping. See the note in [`compose`].
            crate::vdi::cursor_hide(backend);
            for r in pieces.iter().take(n) {
                draw_vslider(w, backend, rect_clip(r.x, r.y, r.w, r.h));
            }
            crate::vdi::cursor_show(backend);
        }
    }
}

/// Draw the bottom (horizontal) scroll bar — left/right arrows, the track, and
/// a thumb positioned + sized from `w.hslide` / `w.hslsize` (both per-mille).
/// The mirror image of [`draw_vslider`] with the axes swapped (x↔y, w↔h): the
/// track runs along the bottom row instead of the right column, offset only by
/// the left border and the vertical scroll bar / sizer column on the right (no
/// title/info bands consume the horizontal axis). Shared by `draw_frame` and
/// `wind_set(WF_HSLIDE/WF_HSLSIZ)`; drawing only the slider row leaves the work
/// area (content) untouched. The bottom-band fill is done by the caller.
/// The horizontal scroll track span `(left, length)` — the region left for
/// the thumb + paging after the left/right arrows — or `None` if there is no
/// HSLIDE track. Mirror of [`vtrack`] with the axes swapped (x↔y, w↔h).
fn htrack(w: &Window) -> Option<(i16, i16)> {
    let kind = w.kind;
    if kind & HSLIDE == 0 {
        return None;
    }
    let bar = hbar_rect(w.curr, kind)?;
    let aw = adj_wbox();
    let left = bar.x + if kind & LFARROW != 0 { aw } else { 0 };
    let len = bar.w
        - if kind & LFARROW != 0 { aw } else { 0 }
        - if kind & RTARROW != 0 { aw } else { 0 };
    (len > 0).then_some((left, len))
}

fn draw_hslider(w: &Window, backend: &dyn RasterBackend, clip: ClipRect) {
    let kind = w.kind;
    let Some(bar) = hbar_rect(w.curr, kind) else {
        return;
    };
    let aw = adj_wbox();
    // Arrows: mirror of [`draw_vslider`] — a `gl_wbox` × `gl_hbox` cell inset
    // `ADJ3DSTD` into the band, each consuming a full `adj_wbox`
    // (gemwmlib.c:448-462).
    if kind & LFARROW != 0 {
        let g = Grect { x: bar.x + ADJ3DSTD, y: bar.y + ADJ3DSTD, w: GL_WBOX, h: GL_HBOX };
        gadget(g, G_LF, backend, clip);
    }
    if kind & RTARROW != 0 {
        let g = Grect { x: bar.x + bar.w - aw + 1, y: bar.y + ADJ3DSTD, w: GL_WBOX, h: GL_HBOX };
        gadget(g, G_RT, backend, clip);
    }
    if let Some((track_x, track_w)) = htrack(w) {
        let track = Grect { x: track_x, y: bar.y, w: track_w, h: bar.h };
        backend.fill_rect(track.x, track.y, track.w, track.h, GRAY_LT, clip);
        rect_outline(track, backend, clip);
        // Thumb geometry shared with `gadget_at`/`drag_elevator` (see
        // [`thumb_geom`]): a stored −1 size renders as one gadget cell.
        let (off, thumb_w) = thumb_geom(track_w, w.hslsize, w.hslide);
        let e = elev_rect(bar, (track_x, track_w), off, thumb_w, false);
        backend.fill_rect(e.x, e.y, e.w, e.h, GRAY_LT, clip);
        bevel(e, true, backend, clip);
    }
}

/// Redraw just window `handle`'s horizontal slider (no work-area clear), for
/// `wind_set(WF_HSLIDE/WF_HSLSIZ)` — EmuTOS `wm_set`'s
/// `w_cpwalk(w_handle, W_HSLIDE, MAX_DEPTH, TRUE)`
/// (`gemwmlib.c:1704-1717,1739-1754`); see [`redraw_vslider`].
fn redraw_hslider(handle: i16) {
    if let (Some(backend), Some(w)) = (host().and_then(|h| h.raster()), win(handle)) {
        if w.open {
            let mut pieces = [Grect::default(); RCAP];
            let n = walk_rects(handle, None, &mut pieces);
            // Counting cursor bracket around the slider repaint — see
            // [`redraw_vslider`] / [`compose`].
            crate::vdi::cursor_hide(backend);
            for r in pieces.iter().take(n) {
                draw_hslider(w, backend, rect_clip(r.x, r.y, r.w, r.h));
            }
            crate::vdi::cursor_show(backend);
        }
    }
}

fn name_len(s: &[u8]) -> usize {
    s.iter().position(|&b| b == 0).unwrap_or(s.len())
}

fn copy_cstr(dst: &mut [u8], src: *const u8) {
    if src.is_null() {
        dst[0] = 0;
        return;
    }
    // SAFETY: trusted GEM pointer to a NUL-terminated string.
    unsafe {
        let mut i = 0;
        while i < dst.len() - 1 {
            let c = *src.add(i);
            dst[i] = c;
            if c == 0 {
                break;
            }
            i += 1;
        }
        dst[dst.len() - 1] = 0;
    }
}

/// Copy the NUL-terminated string in `src` (up to its terminator) out to the
/// caller's `dst` buffer, including the terminating NUL. `src` is one of the
/// window's own bounded name/info buffers (≤ `NAME_CAP - 1` chars + NUL), so
/// the caller's buffer — which the WF_NAME/WF_INFO contract requires to be
/// large enough — is never overrun by our stored length.
fn copy_out_cstr(dst: *mut u8, src: &[u8]) {
    let len = name_len(src);
    // SAFETY: trusted GEM caller buffer, large enough per the WF_NAME/
    // WF_INFO contract; we write `len + 1` bytes bounded by `NAME_CAP`.
    unsafe {
        for (i, &b) in src.iter().take(len).enumerate() {
            *dst.add(i) = b;
        }
        *dst.add(len) = 0;
    }
}

/// Resolve the `WF_NAME`/`WF_INFO` string/buffer pointer for both calling
/// conventions used at our altitude (F7 ABI-width audit):
///
///  * **Native** (the desktop's `desk::wind_set_ptr`, `selftest_objc`): the
///    pointer travels in `addr_in[0]` with `AIN_LEN == 1`.
///  * **Classic AES** (gemlib `wind_set_str` / `wind_get`, both ILP32 and
///    LP64): the pointer is packed at `&int_in[2]` with `AIN_LEN == 0`, at
///    the caller's pointer width — 4 bytes ILP32, 8 bytes LP64.
///
/// `AIN_LEN` is the declared discriminator, so an LP64 desktop and an ILP32
/// gemlib app (QED) both resolve to the right slot.
fn wf_str_ptr(pb: &Aespb) -> u64 {
    if pb.ain_len() >= 1 {
        pb.addrin(0)
    } else {
        pb.intin_ptr(2)
    }
}

// ── dispatch entry points ───────────────────────────────────────────────

/// `wind_create` (100) — allocate a window of border `kind`, max extent
/// `int_in[1..4]`. Returns a handle (≥1) or a negative error.
pub fn wind_create(pb: &Aespb) {
    let kind = pb.intin(0) as u16;
    let full = Grect { x: pb.intin(1), y: pb.intin(2), w: pb.intin(3), h: pb.intin(4) };
    // EmuTOS `wm_create` scans from slot 0 (`gemwmlib.c:1373-1374`); slot 0 is
    // the desktop window and permanently `VF_INUSE`, so the first free handle
    // is 1. The handle *is* the slot index.
    for (i, w) in slots().iter_mut().enumerate() {
        if !w.in_use {
            *w = Window::EMPTY;
            w.in_use = true;
            w.kind = kind;
            w.full = full;
            w.curr = full;
            w.work = calc(WC_WORK, kind, full);
            // Per-window gadget colours inherit the current global defaults
            // (EmuTOS `w_setup`, gemwmlib.c:222-226) so a prior WF_DCOLOR change
            // is reflected in windows created afterwards.
            w.tcolor = *gl_wtcolor();
            w.bcolor = *gl_wbcolor();
            pb.set_ret(i as i16);
            return;
        }
    }
    pb.set_ret(-1); // no free handle
}

/// `wind_open` (101) — display window `int_in[0]` at rect `int_in[1..4]`,
/// on top of the stack. Owned by the calling process.
pub fn wind_open(pb: &Aespb) {
    let handle = pb.intin(0);
    let r = Grect { x: pb.intin(1), y: pb.intin(2), w: pb.intin(3), h: pb.intin(4) };
    if let Some(w) = win(handle) {
        // Re-opening an already-open window is a no-op returning FALSE — no
        // re-top, no re-draw (EmuTOS wm_open, gemwmlib.c:1418-1419:
        // `if (pwin->w_flags & VF_ISOPEN) return FALSE`). The guard sits after
        // the handle-validity check so an invalid handle keeps its own path.
        if w.open {
            pb.set_ret(0);
            return;
        }
        w.prev = w.curr;
        set_curr(w, r);
        w.open = true;
        w.owner = super::appl::current_pid();
        w.z = bump_z();
        // The new window is topmost and covers exactly its own rect `r`; nothing
        // outside `r` changes. Repaint only `r` (background + frames, this window
        // on top) rather than the whole screen — the open/close full-screen
        // flicker fix, mirroring the move/size damage-scope. This window's content
        // follows via `send_redraw` (WM_REDRAW).
        redraw_clip(r.x, r.y, r.w, r.h);
        send_redraw(handle, w.work);
        pb.set_ret(1);
    } else {
        pb.set_ret(0);
    }
}

/// `wind_close` (102) — hide window `int_in[0]` (the handle stays valid), then
/// broadcast `WM_REDRAW` to every window newly uncovered by the removal.
///
/// EmuTOS `wm_close`→`draw_change`→`w_update` (`gemwmlib.c:1438-1459,1058-1223,
/// 1004-1048`): once the window leaves the tree, the AES repaints the desktop
/// background **directly** (`w_drawdesk`, `:340-360,1217-1219`) plus the
/// remaining window frames over the exposed region `c`, and `w_update` walks
/// every still-open window top-to-bottom posting a `WM_REDRAW` for the part of
/// its now-visible area inside `c` (`w_redraw`, `:887-907`). `c` is the closed
/// window's rect, unioned with the new top window's rect **when the closed
/// window was on top** (so the newly-activated top frame + content repaint —
/// `:1194-1211`). The exposed background gets no message; the AES paints it, as
/// here via [`compose`]/[`redraw_clip`] — matching EmuTOS.
pub fn wind_close(pb: &Aespb) {
    let handle = pb.intin(0);
    // `wm_close` returns FALSE for an invalid handle *and* for a valid but
    // not-open window (EmuTOS gemwmlib.c:1444-1447), TRUE only after an actual
    // uncover. [`close_window`] returns exactly that boolean.
    let ok = close_window(handle);
    pb.set_ret(i16::from(ok));
}

/// Close window `handle` (clear its open flag) and repaint the area it vacated —
/// the shared body of [`wind_close`] and the implicit close [`wind_delete`]
/// performs on a still-open window (EmuTOS `wm_close`, gemwmlib.c:1438-1459,
/// invoked from `wm_delete` :1473-1474). Returns `false` for an invalid handle
/// or a window that was not open (no damage, no message); `true` after the
/// uncover.
fn close_window(handle: i16) -> bool {
    let old_top = get_top();
    let Some((r, was_open)) = win(handle).map(|w| {
        let was = w.open;
        w.open = false;
        (w.curr, was)
    }) else {
        return false; // invalid handle
    };
    if !was_open {
        return false; // valid but not open — no action (gemwmlib.c:1446-1447)
    }
    // Damage region `c`: the vacated rect, plus the new top window's rect
    // when the closed window was the top one (activates the new top frame).
    let new_top = get_top();
    let damage = if handle == old_top {
        // `get_top` answers `DESKWH` when nothing is left to top (EmuTOS
        // `w_top`, gemwmlib.c:315-318); there is then no new top frame to
        // activate, so the damage stays the vacated rect.
        match (new_top != DESKWH).then(|| win(new_top).map(|w| w.curr)).flatten() {
            Some(nt) => union_grect(r, nt),
            None => r,
        }
    } else {
        r
    };
    // AES repaints background + remaining frames directly over the damage,
    redraw_clip(damage.x, damage.y, damage.w, damage.h);
    // then fans WM_REDRAW out to every window whose visible region grew.
    broadcast_redraw(damage);
    true
}

/// `wind_delete` (103) — free the handle `int_in[0]`.
///
/// EmuTOS `wm_delete` (gemwmlib.c:1466-1485) closes a still-open window first
/// (`if (pwin->w_flags & VF_ISOPEN) wm_close(w_handle)`, :1473-1474) so its
/// frame + content are uncovered — background and lower windows repaint and the
/// newly-exposed windows get `WM_REDRAW` — *before* the slot is freed; otherwise
/// the deleted window would linger on screen as a dead pixel block. An invalid
/// handle keeps the existing FALSE return.
pub fn wind_delete(pb: &Aespb) {
    let handle = pb.intin(0);
    let Some(was_open) = win(handle).map(|w| w.open) else {
        pb.set_ret(0);
        return;
    };
    if was_open {
        // Reuse the established close-uncover path (redraw_clip + broadcast_redraw).
        close_window(handle);
    }
    if let Some(w) = win(handle) {
        w.in_use = false;
        w.open = false;
    }
    pb.set_ret(1);
}

/// Write a single scalar to `int_out[1]` with `ret = 1` — the common
/// `wind_get` scalar-field return (WF_TOP / the slider fields).
fn scalar_out(pb: &Aespb, v: i16) {
    pb.set_intout(1, v);
    pb.set_ret(1);
}

/// `wind_get` (104) — read a `WF_*` field of window `int_in[0]`.
pub fn wind_get(pb: &Aespb) {
    let handle = pb.intin(0);
    let field = pb.intin(1);

    // Rectangle-list iteration is computed separately from the slot fields.
    if field == WF_FIRSTXYWH || field == WF_NEXTXYWH {
        if field == WF_FIRSTXYWH {
            compute_rlist(handle);
        }
        let r = rlist_next();
        pb.set_intout(1, r.x);
        pb.set_intout(2, r.y);
        pb.set_intout(3, r.w);
        pb.set_intout(4, r.h);
        pb.set_ret(1);
        return;
    }

    // WF_TOP is answered from the global stack, independent of `handle` —
    // in particular wind_get(0, WF_TOP) (a desktop-owner query) must return
    // the top window handle, not fall through to the handle-0 screen rect.
    if field == WF_TOP {
        pb.set_intout(1, get_top());
        pb.set_ret(1);
        return;
    }

    // WF_SCREEN (17): EmuTOS returns the *persistent* menu/alert screen-save
    // buffer address + length (`gsx_mret` → `gl_tmp.fd_addr`/`gl_mlen`, allocated
    // once at AES init, gemgsxif.c:125-129). Needs no valid handle (like WF_TOP,
    // gemwmlib.c:1517-1520).
    //
    // Assessed deliberate deviation (AUD-P5-012): NewTOS's AES has *no* such
    // persistent buffer. Our menu/alert save-under (`bb_save`, #51) allocates a
    // *transient* per-alert MFDB from the heap and frees it on close
    // (`tos_api::vdi::bb_save`), so there is no stable address/length that is
    // "the menu/alert buffer". Fabricating a screen-sized static region purely to
    // return an address would waste memory and — worse — misrepresent an unused
    // scratch region as the menu/alert buffer it is not. Report a null/zero-length
    // buffer truthfully (ret=1); this is a documented deviation, not a stub (it
    // reports the real state — no persistent buffer — rather than faking a value).
    // Revisit only if a program is found that relies on a persistent buffer here.
    if field == WF_SCREEN {
        for i in 1..=4 {
            pb.set_intout(i, 0); // address (hi/lo) + length (hi/lo)
        }
        pb.set_ret(1);
        return;
    }

    // WF_DCOLOR (19): the *global* default gadget colour words. Like WF_TOP /
    // WF_SCREEN it needs no valid handle (EmuTOS gemwmlib.c:1519,1572-1576).
    // gadget number = int_in[2] (pinwds[0]); colour words → int_out[2..3]
    // (poutwds[1..2], D-095), TRUE.
    if field == WF_DCOLOR {
        let g = gadget_index(pb.intin(2));
        pb.set_intout(2, gl_wtcolor()[g]);
        pb.set_intout(3, gl_wbcolor()[g]);
        pb.set_ret(1);
        return;
    }

    let r = match win(handle) {
        Some(w) => match field {
            // The four rectangle fields are plain reads of the stored
            // `WS_WORK`/`WS_CURR`/`WS_PREV`/`WS_FULL` (EmuTOS `wm_get` →
            // `w_getsize`, gemwmlib.c:1528-1541,1596-1597). `WF_WXYWH` is
            // *stored*, not recomputed: for the desktop window `wm_init` puts
            // `gl_rfull` there, which is not `wm_calc(WC_WORK, NONE, screen)`.
            WF_WXYWH => w.work,
            WF_CXYWH => w.curr,
            WF_PXYWH => w.prev,
            WF_FXYWH => w.full,
            // Slider position/size scalars → int_out[1] (EmuTOS wm_get,
            // gemwmlib.c:1542-1551). WF_HSLSIZ/WF_VSLSIZ round-trip the −1
            // "use default size" marker unchanged.
            WF_HSLIDE => return scalar_out(pb, w.hslide),
            WF_VSLIDE => return scalar_out(pb, w.vslide),
            WF_HSLSIZ => return scalar_out(pb, w.hslsize),
            WF_VSLSIZ => return scalar_out(pb, w.vslsize),
            // WF_COLOR (18): the per-window topped/untopped colour words for
            // gadget int_in[2] → int_out[2..3] (poutwds[1..2], D-095), TRUE
            // (EmuTOS wm_get, gemwmlib.c:1567-1571).
            WF_COLOR => {
                let g = gadget_index(pb.intin(2));
                pb.set_intout(2, w.tcolor[g]);
                pb.set_intout(3, w.bcolor[g]);
                pb.set_ret(1);
                return;
            }
            // WF_OWNER (20): owner pid, open flag, and the top/bottom window
            // handles (EmuTOS wm_get, AES_VERSION>=0x330 gemwmlib.c:1579-1588;
            // our profile is 0x340). poutwds[0..3] → int_out[1..4].
            WF_OWNER => {
                let (owner, open) = (w.owner, w.open);
                pb.set_intout(1, owner);
                pb.set_intout(2, i16::from(open));
                pb.set_intout(3, get_top());
                pb.set_intout(4, get_bottom());
                pb.set_ret(1);
                return;
            }
            // WF_NAME/WF_INFO get: copy the stored title/info out to the
            // caller's buffer (pointer at `&int_in[2]`, caller width —
            // gemlib `wind_get`, `a_wind_get.c:344`; the native `addr_in[0]`
            // convention is honoured too via `wf_str_ptr`). Per the gemlib
            // contract, an empty title returns FALSE and leaves the caller's
            // buffer untouched.
            WF_NAME | WF_INFO => {
                let src = if field == WF_NAME { w.name } else { w.info };
                if name_len(&src) == 0 {
                    pb.set_ret(0);
                    return;
                }
                let dst = wf_str_ptr(pb) as *mut u8;
                if dst.is_null() {
                    pb.set_ret(0);
                    return;
                }
                copy_out_cstr(dst, &src);
                pb.set_ret(1);
                return;
            }
            // Any other field — including `WF_KIND` (1), which `wm_get` does
            // **not** handle: it falls to `default: return FALSE` and writes
            // nothing (`gemwmlib.c:1591`). We used to answer it (kind + TRUE);
            // that was deliberate deviation D-053 / AUD-P5-014 and is now
            // dropped, because outside the hardware layer NewTOS does not
            // deviate from EmuTOS on purpose.
            _ => {
                pb.set_ret(0);
                return;
            }
        },
        None => {
            pb.set_ret(0);
            return;
        }
    };
    pb.set_intout(1, r.x);
    pb.set_intout(2, r.y);
    pb.set_intout(3, r.w);
    pb.set_intout(4, r.h);
    pb.set_ret(1);
}

/// `wind_set` (105) — set a `WF_*` field of window `int_in[0]`.
pub fn wind_set(pb: &Aespb) {
    let handle = pb.intin(0);
    let field = pb.intin(1);
    // WF_DCOLOR (19) writes the *global* default gadget colour words and needs
    // no valid handle (EmuTOS wm_set, gemwmlib.c:1632-1636,1727-1734): gadget =
    // int_in[2], tcolor = int_in[3], bcolor = int_in[4] (−1 = leave unchanged).
    // No window redraw (EmuTOS sets `gadget = -1` → no w_cpwalk).
    if field == WF_DCOLOR {
        let g = gadget_index(pb.intin(2));
        let (tc, bc) = (pb.intin(3), pb.intin(4));
        if tc != -1 {
            gl_wtcolor()[g] = tc;
        }
        if bc != -1 {
            gl_wbcolor()[g] = bc;
        }
        pb.set_ret(1);
        return;
    }
    if win(handle).is_none() {
        pb.set_ret(0);
        return;
    }
    match field {
        // WF_NEWDESK registers the desktop-background tree on the window named
        // by the handle — in practice the desktop window, `DESKWH` = 0, which
        // is a real slot since [`wm_init`]. EmuTOS also re-owns that window to
        // the caller (`pwin->w_owner = rlr`, `gemwmlib.c:1684-1687`).
        WF_NEWDESK => {
            if let Some(w) = win(handle) {
                w.owner = super::appl::current_pid();
            }
            set_newdesk(pb);
        }
        WF_NAME => {
            // The title pointer arrives two ways (see `wf_str_ptr`): the
            // desktop's native `addr_in[0]` (AIN_LEN=1), or the classic-AES
            // `&int_in[2]` packing at the caller's pointer width (gemlib
            // `wind_set_str`, AIN_LEN=0) — the latter is how ILP32 apps
            // (QED) pass it. Reading `addr_in[0]` unconditionally handed an
            // ILP32 caller an empty slot → garbage title (F7 audit).
            let title = wf_str_ptr(pb) as *const u8;
            if let Some(w) = win(handle) {
                copy_cstr(&mut w.name, title);
            }
            redraw_open(handle);
        }
        WF_INFO => {
            let info = wf_str_ptr(pb) as *const u8;
            if let Some(w) = win(handle) {
                copy_cstr(&mut w.info, info);
            }
            redraw_open(handle);
        }
        WF_CXYWH => {
            let r = Grect { x: pb.intin(2), y: pb.intin(3), w: pb.intin(4), h: pb.intin(5) };
            let (work, prev) = if let Some(w) = win(handle) {
                w.prev = w.curr;
                set_curr(w, r);
                (w.work, w.prev)
            } else {
                (r, r)
            };
            // Repaint only the union of the old and new border rects — the
            // programmatic move/size path (desktop WM_MOVED/WM_SIZED/WM_FULLED,
            // app `wind_set`) — not the whole screen. This is EmuTOS
            // `draw_change`'s tail (`gemwmlib.c:1216-1222`): `w_drawdesk(&c)`
            // over the damage rectangle, then `w_update(start, &c, stop, moved)`.
            let d = union_grect(prev, r);
            redraw_clip(d.x, d.y, d.w, d.h);
            // This window's content: `w_redraw` for the whole work area, since —
            // unlike EmuTOS, which blits the top window across in `w_move`
            // (`:944-987`) and then passes `moved = TRUE` to skip it — nothing
            // carried its pixels to the new place.
            send_redraw(handle, work);
            // Every *other* window the damage reaches: `w_update` walks the whole
            // window list and calls `w_redraw(top, pt)` on each (`:1004-1048`,
            // `:887-907`), so a window the moved one uncovers is asked for its
            // content back. Without this the vacated rectangle kept showing the
            // moved window's old pixels wherever it fell inside another window —
            // the AES may not paint there itself (`do_walk`, `:286-312`), so the
            // owner has to. Same-window redraws coalesce in the FIFO, so the
            // `send_redraw` above is not duplicated.
            broadcast_redraw(d);
        }
        WF_TOP => {
            // `if (w_handle != gl_wtop) wm_mktop(w_handle)` — EmuTOS `wm_set`
            // (`gemwmlib.c:1676-1680`). A `WF_TOP` on the window that is
            // *already* on top does nothing whatsoever: no repaint, no message.
            // (Topping it anyway used to run the whole composite over the
            // window's rectangle, which erased its content; TeraDesk answers
            // every `WM_TOPPED` with a `wind_set(WF_TOP)`, so this ran on every
            // click on a window.)
            //
            // [`top_window`] *is* `wm_mktop`→`draw_change`'s top branch,
            // including the conditional `w_redraw` — whether a `WM_REDRAW` is
            // due depends on the `VF_BROKEN` state only that function knows.
            // No `WM_UNTOPPED` here: EmuTOS sends it solely from the Control
            // Manager's `perform_untop` on the click (`aes/gemctrl.c:146-153,
            // 174,344`), which is [`intercept`]'s job.
            if handle != get_top() {
                top_window(handle);
            }
        }
        // Slider positions clamp to 1..1000; sizes clamp to 1..1000 but let −1
        // ("use default size") pass through (EmuTOS wm_set, gemwmlib.c:1643-1656).
        WF_VSLIDE => {
            if let Some(w) = win(handle) {
                w.vslide = pb.intin(2).clamp(1, 1000);
            }
            redraw_vslider(handle);
        }
        WF_VSLSIZ => {
            if let Some(w) = win(handle) {
                w.vslsize = slider_size(pb.intin(2));
            }
            redraw_vslider(handle);
        }
        WF_HSLIDE => {
            if let Some(w) = win(handle) {
                w.hslide = pb.intin(2).clamp(1, 1000);
            }
            redraw_hslider(handle);
        }
        WF_HSLSIZ => {
            if let Some(w) = win(handle) {
                w.hslsize = slider_size(pb.intin(2));
            }
            redraw_hslider(handle);
        }
        // WF_COLOR (18): store the per-window colour words for gadget int_in[2]
        // — tcolor = int_in[3], bcolor = int_in[4] (−1 = leave unchanged) —
        // EmuTOS wm_set gemwmlib.c:1719-1726. EmuTOS then redraws the gadget with
        // the new colours (`do_cpwalk`); our procedural [`draw_frame`] renders the
        // fixed TOS4 3D look and does not consume these per-gadget words, so the
        // frame re-emit (like a WF_NAME change) is byte-identical — the words are
        // pure round-trip state observed via WF_COLOR get.
        WF_COLOR => {
            let g = gadget_index(pb.intin(2));
            let (tc, bc) = (pb.intin(3), pb.intin(4));
            if let Some(w) = win(handle) {
                if tc != -1 {
                    w.tcolor[g] = tc;
                }
                if bc != -1 {
                    w.bcolor[g] = bc;
                }
            }
            redraw_open(handle);
        }
        // Unknown or unimplemented field → FALSE (EmuTOS wm_set default:
        // gemwmlib.c:1736). WF_NEWDESK/WF_DCOLOR are handled above; WF_OWNER is
        // get-only (EmuTOS has no wm_set WF_OWNER case) → FALSE here.
        _ => {
            pb.set_ret(0);
            return;
        }
    }
    pb.set_ret(1);
}

/// Validate a slider *size* (`WF_HSLSIZ`/`WF_VSLSIZ`): clamp to 1..1000, but
/// pass −1 ("use default size") through unclamped (EmuTOS gemwmlib.c:1643-1648).
fn slider_size(v: i16) -> i16 {
    if v == -1 {
        -1
    } else {
        v.clamp(1, 1000)
    }
}

/// Redraw just `handle`'s frame if it is open (for name/info changes) — EmuTOS
/// `wm_set`'s `w_cpwalk(w_handle, W_NAME/W_INFO, MAX_DEPTH, TRUE)`
/// (`gemwmlib.c:1660-1675,1753-1754`), i.e. `do_walk` over the window's own
/// owner rectangle list: a window that another one covers repaints its title
/// only where it is actually visible.
fn redraw_open(handle: i16) {
    if let (Some(backend), Some(w)) = (host().and_then(|h| h.raster()), win(handle)) {
        if w.open {
            // Counting cursor bracket around the single-frame repaint (EmuTOS
            // `w_update` moff/mon) — see [`compose`].
            crate::vdi::cursor_hide(backend);
            draw_frame_pieces(handle, handle == get_top(), backend, None);
            crate::vdi::cursor_show(backend);
        }
    }
}

/// `wind_find` (106) — handle of the window under `(int_in[0], int_in[1])`;
/// 0 = the desktop window, −1 when the point is on no window at all (off the
/// screen) — see [`find_handle`].
pub fn wind_find(pb: &Aespb) {
    pb.set_ret(find_handle(pb.intin(0), pb.intin(1)));
}

/// `wind_update` (107) — window-tree screen lock (BEG/END_UPDATE) and mouse-
/// control-ownership transfer (BEG/END_MCTRL).
///
/// EmuTOS `wm_update` (gemwmlib.c:1778-1797): mode `< 2` takes/releases the
/// counting window-tree lock `wind_spb`; mode `>= 2` calls `fm_own(mode - 2)`,
/// the mouse-control-ownership transfer. `fm_own` (gemfmlib.c:82-107) brackets
/// with BEG/END_UPDATE and keeps the `ml_ocnt` counter. While a `BEG_MCTRL` is
/// in effect (`ml_ocnt > 0`) the app has taken over the mouse, so the AES frame
/// interception ([`intercept`]) switches off. This structure is the foundation
/// P4-011's full `fm_own` (menu-tree hold + control-region transfer via
/// `ct_chgown`, the `ml_ocnt == 0` edge body) builds on top of.
pub fn wind_update(pb: &Aespb) {
    // EmuTOS `wm_update` (gemwmlib.c:1778-1797): mode `< 2` is the window-tree
    // lock (0 = END_UPDATE, any non-zero = BEG_UPDATE); mode `>= 2` is
    // `fm_own(mode - 2)` (non-zero → begin the mouse-control transfer). The two
    // guard arms reproduce that for out-of-range modes.
    match pb.intin(0) {
        END_UPDATE => update_end(),
        BEG_UPDATE => update_beg(),
        END_MCTRL => mctrl_own(false),
        BEG_MCTRL => mctrl_own(true),
        m if m < 2 => update_beg(), // negative: non-zero, `< 2` → BEG_UPDATE
        m => mctrl_own(m - 2 != 0), // `> 3`: fm_own(m-2) begins
    }
    pb.set_ret(1);
}

/// `tak_flag(&wind_spb)` — take the counting window-tree lock. In single-tasking
/// the take always succeeds (one AES process, so `sy_owner == rlr`), so this is
/// a plain increment of `sy_tas` (EmuTOS gemflag.c:71-86).
fn update_beg() {
    UPDATE_LOCK.fetch_add(1, Ordering::Relaxed);
}

/// `unsync(&wind_spb)` — release the counting window-tree lock: decrement, but
/// never below zero (EmuTOS gemflag.c:105-115: a stray `END_UPDATE` with the
/// count already 0 is ignored).
fn update_end() {
    let v = UPDATE_LOCK.load(Ordering::Relaxed);
    if v > 0 {
        UPDATE_LOCK.store(v - 1, Ordering::Relaxed);
    }
}

/// Public `fm_own` entry for the Form Manager (EmuTOS gemfmlib.c:82-107): a
/// modal dialog brackets its wait loop with `fm_own(TRUE)` / `fm_own(FALSE)`
/// (`fm_do`, gemfmlib.c:287,349; `fm_alert` reaches the loop via `fm_do`).
/// Delegates to [`mctrl_own`], the shared mouse-control-ownership + menu-hold
/// transition that `wind_update(BEG/END_MCTRL)` also drives.
pub fn fm_own(begin: bool) {
    mctrl_own(begin);
}

/// `fm_own(begin)` — mouse-control-ownership transfer (EmuTOS gemfmlib.c:82-107).
/// `begin`: `BEG_UPDATE`, then on the outermost 0 → 1 edge park the menu tree,
/// then `ml_ocnt++`. `!begin`: `ml_ocnt--`, then on the 1 → 0 edge restore the
/// menu tree, then `END_UPDATE`. The menu-hold (`gl_mntree = NULL`) is the
/// observable part of the `ml_ocnt == 0` edge body: a menu-bar click is inert
/// while the dialog is up. The control-region / mouse-owner handover
/// (`get_ctrl`/`get_mown`/`ct_chgown`, gemfmlib.c:91-93,102) is the cooperative
/// multi-process ownership transfer and is a no-op at single-tasking (one AES
/// process owns everything) — deferred to the cooperative kernel (Phase 2b).
fn mctrl_own(begin: bool) {
    if begin {
        update_beg(); // fm_own: wm_update(BEG_UPDATE) (gemfmlib.c:86)
        // 0 → 1 edge, evaluated BEFORE the increment (EmuTOS tests `ml_ocnt`
        // while still 0, gemfmlib.c:87): park the active menu tree so the
        // Control Manager stops tracking the bar for the duration of the dialog.
        if MCTRL_CNT.load(Ordering::Relaxed) == 0 {
            super::menu::hold_menu();
        }
        MCTRL_CNT.fetch_add(1, Ordering::Relaxed);
    } else {
        // ml_ocnt-- then, on the 1 → 0 edge, restore the parked menu tree
        // (EmuTOS `ml_ocnt--; if (ml_ocnt == 0) { ...; gl_mntree = ml_mnhold; }`,
        // gemfmlib.c:99-104), then wm_update(END_UPDATE). `fetch_sub` returns the
        // PRE-decrement value, so the outermost edge is `prev == 1`. EmuTOS
        // decrements unconditionally; the `intercept` gate (`> 0`) treats any
        // non-positive count as "app does not own the mouse", so an unbalanced
        // END_MCTRL cannot wedge interception off.
        if MCTRL_CNT.fetch_sub(1, Ordering::Relaxed) == 1 {
            super::menu::release_menu();
        }
        update_end();
    }
}

/// `wind_calc` (108) — convert work↔border rect for `kind`.
pub fn wind_calc(pb: &Aespb) {
    let wtype = pb.intin(0);
    let kind = pb.intin(1) as u16;
    let r = Grect { x: pb.intin(2), y: pb.intin(3), w: pb.intin(4), h: pb.intin(5) };
    let o = calc(wtype, kind, r);
    pb.set_intout(1, o.x);
    pb.set_intout(2, o.y);
    pb.set_intout(3, o.w);
    pb.set_intout(4, o.h);
    pb.set_ret(1);
}

/// `wind_new` (109) — close + delete every window and clear every lock taken
/// with `wind_update` (EmuTOS `wm_new`, gemwmlib.c:1875-1893: it drains
/// `ml_ocnt` via `END_MCTRL` and `wind_spb.sy_tas` via `END_UPDATE` before
/// deleting the windows). Releasing the locks here is the observable equivalent
/// of those drain loops (both counters end at 0).
pub fn wind_new(pb: &Aespb) {
    wm_new();
    if let Some(backend) = host().and_then(|h| h.raster()) {
        redraw_all(backend);
    }
    pb.set_ret(1);
}

/// `wm_new` (`gemwmlib.c:1875-1893`) — the internal window sweep behind
/// [`wind_new`]: drain the outstanding `wind_update` mouse-control and
/// screen-sync locks, then close and delete every window still in use.
///
/// The shell loop calls it after a launched program terminates
/// (`gemshlib.c:677`) and after an abnormal desktop exit (`:609`), so the
/// windows the previous occupant left open are swept before the next program
/// starts. It **draws nothing** — EmuTOS's `wm_new` only closes/deletes; the
/// repaint belongs to the `wind_new` *opcode* (which an application calls
/// expecting its screen back).
pub fn wm_new() {
    // `for (wh = 1; wh < NUM_WIN; wh++)` (gemwmlib.c:1886) — handle 0, the
    // desktop window, is not one of the windows `wm_new` deletes.
    slots()[1..].fill(Window::EMPTY);
    // SAFETY: single-CPU AES.
    unsafe { *TABLE.next_z.get() = 1 };
    // Release all update / mouse-control locks (gemwmlib.c:1879-1883).
    MCTRL_CNT.store(0, Ordering::Relaxed);
    UPDATE_LOCK.store(0, Ordering::Relaxed);
}

/// `wm_init` (`gemwmlib.c:1247-1310`) — re-initialise the Window Manager
/// before each program the shell loop starts (`gemshlib.c:717`). EmuTOS
/// rebuilds the whole window-extent/element object arena; the observable
/// part for us is that **every window is gone**, the desktop window (handle
/// 0) spans the screen again, and `gl_newdesk` is reset to NULL
/// (`gemwmlib.c:1300`) so a tree the previous program registered with
/// `wind_set(WF_NEWDESK)` cannot leak into the next one.
///
/// Resetting `gl_newdesk` is what makes it safe for the shell loop to run
/// after an EL0 application: the tree pointer that application registered
/// lived in its own (now unmapped) address space.
///
/// Like `wm_new` it draws nothing; `sh_draw(clear)` does the painting
/// immediately afterwards (`gemshlib.c:719`).
pub fn wm_init() {
    wm_new();
    // `w_setup(ppd, DESKWH, NONE)` + the four `w_setsize` calls
    // (`gemwmlib.c:1290-1294`).
    init_desk_window();
    // `gl_newdesk = NULL;` (gemwmlib.c:1300)
    NEWDESK_TREE.store(0, Ordering::Release);
    NEWDESK_ROOT.store(0, Ordering::Release);
    NEWDESK_APP.store(false, Ordering::Release);
    // Any saved desktop window set belongs to a desktop instance that is
    // over; `wm_init` leaves no window state behind (`:1263-1268`).
    // SAFETY: single-CPU AES.
    unsafe { *SAVE.active.get() = false };
}

fn inside(r: Grect, x: i16, y: i16) -> bool {
    x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h
}

/// Repaint the desktop background + every open window (whole screen). The
/// desktop composite (`desk::redraw_desktop`) drives this after a full-screen
/// program returns; the Form Manager's scoped `form_dial(FMD_FINISH)` uses
/// [`redraw_dialog_area`] instead so only the dialog's rectangle is repainted.
pub fn redraw() {
    if let Some(backend) = host().and_then(|h| h.raster()) {
        redraw_all(backend);
    }
}

/// Repaint the desktop background + remaining window frames within the dialog
/// target rectangle `pt`, then fan a `WM_REDRAW` out to every window overlapping
/// it — the Form Manager's `form_dial(FMD_FINISH)` restore, confined to `pt`
/// (not the whole desktop).
///
/// EmuTOS `fm_dial` FMD_FINISH (gemfmlib.c:375-379): `w_drawdesk(pt)` +
/// `w_update(DESKWH, pt, DESKWH, FALSE)` — the same background-repaint +
/// window-redraw fan-out pair used on window close ([`wind_close`]),
/// scoped to `pt` (`int_in[5..8]`, the big rect) rather than the full screen.
pub fn redraw_dialog_area(x: i16, y: i16, w: i16, h: i16) {
    if w <= 0 || h <= 0 {
        return;
    }
    // w_drawdesk(pt) + remaining window frames over pt.
    redraw_clip(x, y, w, h);
    // w_update(DESKWH, pt, DESKWH, FALSE): WM_REDRAW for each overlapping window.
    broadcast_redraw(Grect { x, y, w, h });
}

fn screen_rect() -> Grect {
    if let Some(backend) = host().and_then(|h| h.raster()) {
        Grect { x: 0, y: 0, w: backend.width() as i16, h: backend.height() as i16 }
    } else {
        Grect { x: 0, y: 0, w: 640, h: 480 }
    }
}

/// EmuTOS `gl_rfull` — the full screen minus the menu row at the top
/// (`gsx_start`, `gemgraf.c:423-424`, set **unconditionally**, whether or not a
/// menu bar is installed). [`wm_init`] stores it as the desktop window's `FULL`
/// and `WORK` rectangles, which is where every later reader takes it from.
fn desktop_work() -> Grect {
    let full = screen_rect();
    let top = super::menu::desk_top();
    Grect { x: full.x, y: full.y + top, w: full.w, h: full.h - top }
}

// ── Batch 2: interactive frame + WM_* messages ──────────────────────────

// WM_* message ids (aesdefs.h:229-239).
const WM_TOPPED: i16 = 21;
const WM_CLOSED: i16 = 22;
const WM_FULLED: i16 = 23;
const WM_ARROWED: i16 = 24;
const WM_HSLID: i16 = 25;
const WM_VSLID: i16 = 26;
const WM_SIZED: i16 = 27;
const WM_MOVED: i16 = 28;
const WM_UNTOPPED: i16 = 30;

// WM_ARROWED actions (aesdefs.h:245-253, gemctrl.c:63-74). Line = arrow, page
// = track click either side of the elevator.
const WA_UPPAGE: i16 = 0;
const WA_DNPAGE: i16 = 1;
const WA_UPLINE: i16 = 2;
const WA_DNLINE: i16 = 3;
const WA_LFPAGE: i16 = 4;
const WA_RTPAGE: i16 = 5;
const WA_LFLINE: i16 = 6;
const WA_RTLINE: i16 = 7;

/// Which frame component a point falls in. Beyond the title/sizer/arrow
/// gadgets, the scroll bars distinguish the elevator **thumb** (drag →
/// `WM_HSLID`/`WM_VSLID`) from a **page** click in the track either side of it
/// (→ `WM_ARROWED` with `WA_*PAGE`), mirroring EmuTOS `hctl_window`'s split of
/// `W_?ELEV` vs `W_?SLIDE` (gemctrl.c:262-306).
#[derive(PartialEq, Clone, Copy)]
enum Gadget {
    None,
    Closer,
    Fuller,
    Mover,
    Sizer,
    Up,
    Dn,
    Lf,
    Rt,
    VElev,  // vertical elevator thumb (drag)
    HElev,  // horizontal elevator thumb (drag)
    VUpPage, // v-track above the thumb (WA_UPPAGE)
    VDnPage, // v-track below the thumb (WA_DNPAGE)
    HLfPage, // h-track left of the thumb (WA_LFPAGE)
    HRtPage, // h-track right of the thumb (WA_RTPAGE)
}

/// Hit-test a point against `w`'s frame gadgets (mirrors `draw_frame`'s
/// layout).
fn gadget_at(w: &Window, mx: i16, my: i16) -> Gadget {
    let b = w.curr;
    let kind = w.kind;
    if !inside(b, mx, my) {
        return Gadget::None;
    }
    let ah = adj_hbox();
    let aw = adj_wbox();

    // Title row — one `adj_hbox` band, `adj_wbox` per gadget cell (the layout
    // [`draw_frame`] paints).
    if kind & TGADGETS != 0 && my < b.y + ah {
        if kind & CLOSER != 0 && mx < b.x + aw {
            return Gadget::Closer;
        }
        if kind & FULLER != 0 && mx >= b.x + b.w - aw {
            return Gadget::Fuller;
        }
        if kind & MOVER != 0 {
            return Gadget::Mover;
        }
        return Gadget::None;
    }
    // Sizer corner.
    if kind & SIZER != 0 && mx >= b.x + b.w - aw && my >= b.y + b.h - ah {
        return Gadget::Sizer;
    }
    // Right scroll-bar arrows.
    if let Some(bar) = vbar_rect(b, kind).filter(|_| mx >= b.x + b.w - aw) {
        if kind & UPARROW != 0 && my < bar.y + ah {
            return Gadget::Up;
        }
        if kind & DNARROW != 0 && my >= bar.y + bar.h - ah {
            return Gadget::Dn;
        }
        // Elevator thumb (drag) vs. a page click in the track either side of
        // it (EmuTOS `hctl_window` W_VELEV vs W_VSLIDE split, gemctrl.c:262-306:
        // `my < elev_y` → up-page, else down-page).
        if let Some((track_y, track_h)) = vtrack(w) {
            if my >= track_y && my < track_y + track_h {
                let (off, len) = thumb_geom(track_h, w.vslsize, w.vslide);
                let e = elev_rect(bar, (track_y, track_h), off, len, true);
                if my >= e.y && my < e.y + e.h {
                    return Gadget::VElev;
                }
                return if my < e.y { Gadget::VUpPage } else { Gadget::VDnPage };
            }
        }
    }
    // Bottom scroll-bar arrows.
    if let Some(bar) = hbar_rect(b, kind).filter(|_| my >= b.y + b.h - ah) {
        if kind & LFARROW != 0 && mx < bar.x + aw {
            return Gadget::Lf;
        }
        if kind & RTARROW != 0 && mx >= bar.x + bar.w - aw {
            return Gadget::Rt;
        }
        // Elevator thumb (drag) vs. a page click in the track either side of it
        // (EmuTOS W_HELEV vs W_HSLIDE split, `mx < elev_x` → left-page).
        if let Some((track_x, track_w)) = htrack(w) {
            if mx >= track_x && mx < track_x + track_w {
                let (off, len) = thumb_geom(track_w, w.hslsize, w.hslide);
                let e = elev_rect(bar, (track_x, track_w), off, len, false);
                if mx >= e.x && mx < e.x + e.w {
                    return Gadget::HElev;
                }
                return if mx < e.x { Gadget::HLfPage } else { Gadget::HRtPage };
            }
        }
    }
    Gadget::None
}

/// The screen rectangle of a title-row gadget (closer/fuller), for the
/// `watchbox` feedback — the `gl_wbox` × `gl_hbox` cell [`draw_frame`] paints,
/// inset `ADJ3DSTD` inside the `adj_wbox`-wide title cell.
fn gad_rect(w: &Window, g: Gadget) -> Grect {
    let b = w.curr;
    let y = b.y + ADJ3DSTD;
    match g {
        Gadget::Closer => Grect { x: b.x + ADJ3DSTD, y, w: GL_WBOX, h: GL_HBOX },
        Gadget::Fuller => {
            Grect { x: b.x + b.w - adj_wbox() + ADJ3DSTD, y, w: GL_WBOX, h: GL_HBOX }
        }
        _ => Grect::default(),
    }
}

/// Track the button over a closer/fuller gadget: invert it while the mouse
/// is inside, restore on leave; return whether it was released inside
/// (EmuTOS `gr_watchbox`).
fn watchbox(r: Grect, ch: u16, backend: &dyn RasterBackend) -> bool {
    let Some(h) = host() else { return false };
    let clip = ClipRect::full(backend.width(), backend.height());
    let mut was_in = true;
    gadget_sel(r, ch, true, backend, clip);
    loop {
        let (mx, my) = crate::vdi::mouse_position();
        let now_in = inside(r, mx, my);
        if now_in != was_in {
            gadget_sel(r, ch, now_in, backend, clip);
            was_in = now_in;
        }
        if crate::vdi::mouse_buttons() & 0x01 == 0 {
            break;
        }
        h.pump_input();
    }
    gadget_sel(r, ch, false, backend, clip); // restore normal
    was_in
}

/// EmuTOS `gr_stilldn` (`aes/gemgrlib.c:47-73`) as the sizer and mover drags use
/// it: block until the pointer leaves the 1×1 rectangle at `(x, y)` — i.e. until
/// it moves at all — or the button comes up. Returns `false` for the button-up
/// exit (`which & MU_BUTTON`, `:69-70`) and `true` for the `MU_M1` exit.
///
/// `gr_wait` passes `gr_stilldn(TRUE, xrat, yrat, 1, 1)` (`:182`), so the
/// `MOBLK` fires as soon as the pointer is *outside* that one-pixel rectangle;
/// the button condition is `0x0001ff00` = "all buttons up, one click"
/// (`:62-66`), which here is simply the release. A key press (`MU_KEYBD`) is
/// also armed but does not end the drag — it returns without `MU_BUTTON`.
fn gr_stilldn(x: i16, y: i16, h: &'static dyn Host) -> bool {
    loop {
        if crate::vdi::mouse_buttons() & 0x01 == 0 {
            return false;
        }
        let (mx, my) = crate::vdi::mouse_position();
        if mx != x || my != y {
            return true;
        }
        h.pump_input();
    }
}

/// EmuTOS `gr_draw` (`aes/gemgrlib.c:155-166`): the rubber box `o`, plus — when
/// `poff` is not the zero rectangle (`have2box`) — a second box offset from it
/// per component. XOR, so the same call draws and erases.
fn gr_draw(o: Grect, poff: Grect, backend: &dyn RasterBackend, clip: ClipRect) {
    super::graf::gsx_xbox(o.x, o.y, o.w, o.h, backend, clip);
    if poff != Grect::default() {
        super::graf::gsx_xbox(
            o.x + poff.x,
            o.y + poff.y,
            o.w + poff.w,
            o.h + poff.h,
            backend,
            clip,
        );
    }
}

/// EmuTOS `gr_rubwind` (`aes/gemgrlib.c:199-220`) — stretch the free corner of
/// an XOR box pinned at `(xorigin, yorigin)` until the button comes up, drawing
/// a second box offset by `poff` alongside it; returns the final `(w, h)`.
///
/// The window is **not** resized here and nothing on the screen is repainted:
/// the only pixels touched are the two XOR outlines, which each `gr_wait`
/// draws, waits on, and complements away again (`:169-191`). The Control
/// Manager then posts `WM_SIZED` and the *owner* confirms the new geometry with
/// `wind_set(WF_CURRXYWH)`, which is what repaints the window and the region it
/// uncovered (`hctl_window` → `ct_msgup`, `gemctrl.c:258-259,314`).
///
/// This replaces the former solid live-resize, which repainted the desktop
/// background (`w_drawdesk` — background box *and every icon*) over the whole
/// window rectangle and redrew the frame on top of it, once per drag frame. The
/// window's area therefore held the background-behind-the-window, icons
/// included, for the duration of each frame's redraw: the square "holes" that
/// travel with the drag on a real Pi 4. The mover drag never showed it because
/// it blits the window to its new place instead of exposing what is under it.
fn gr_rubwind(
    h: &'static dyn Host,
    xorigin: i16,
    yorigin: i16,
    wmin: i16,
    hmin: i16,
    poff: Grect,
    backend: &dyn RasterBackend,
) -> (i16, i16) {
    update_beg(); // wm_update(BEG_UPDATE)
    // `gr_setup(BLACK)`: clip to the whole screen, XOR write mode
    // (`gemgrlib.c:76-80`); the XOR mode lives in `gsx_xbox`.
    let clip = ClipRect::full(backend.width(), backend.height());
    let mut o = Grect { x: xorigin, y: yorigin, w: 0, h: 0 };
    loop {
        // `gr_clamp` (`gemgrlib.c:83-88`): the free corner follows the pointer,
        // never closer than (wmin, hmin) — a *signed* max, no upper clamp.
        let (mx, my) = crate::vdi::mouse_position();
        o.w = (mx - o.x + 1).max(wmin);
        o.h = (my - o.y + 1).max(hmin);
        // `gr_wait` (`gemgrlib.c:169-191`): draw, wait for a change, erase.
        // The `gsx_moff`/`gsx_mon` brackets are our counted cursor hide/show.
        crate::vdi::cursor_hide(backend);
        gr_draw(o, poff, backend, clip);
        crate::vdi::cursor_show(backend);
        let down = gr_stilldn(mx, my, h);
        crate::vdi::cursor_hide(backend);
        gr_draw(o, poff, backend, clip);
        crate::vdi::cursor_show(backend);
        if !down {
            break;
        }
    }
    update_end(); // wm_update(END_UPDATE)
    (o.w, o.h)
}

/// The `W_SIZER` arm of EmuTOS `hctl_window` (`aes/gemctrl.c:241-260`): work out
/// the second box's offset and the minimum size from the window, run
/// [`gr_rubwind`], and report the rectangle that goes into the `WM_SIZED`
/// message (`ct_msgup(message, owner, wh, x, y, w, h)`, `gemctrl.c:314`) — the
/// **unchanged** border origin with the dragged size.
fn drag_size(w: &Window, backend: &dyn RasterBackend) -> Grect {
    let border = w.curr;
    // No host means no input pump, so there is no drag to run: report the
    // geometry unchanged rather than inventing one.
    let Some(h) = host() else { return border };
    // `t = WS_WORK - WS_CURR`, per component (`gemctrl.c:247-251`) — the second
    // XOR box is the work area the new size would have.
    let poff = Grect {
        x: w.work.x - border.x,
        y: w.work.y - border.y,
        w: w.work.w - border.w,
        h: w.work.h - border.h,
    };
    // Minimum size (`gemctrl.c:252-257`): one character cell, widened to four
    // gadget boxes when the window has top/horizontal gadgets and heightened to
    // six when it has vertical ones.
    let wmin = if w.kind & (TGADGETS | HGADGETS) != 0 { GL_WBOX * 4 } else { GL_WCHAR };
    let hmin = if w.kind & VGADGETS != 0 { GL_HBOX * 6 } else { GL_HCHAR };
    let (nw, nh) = gr_rubwind(h, border.x, border.y, wmin, hmin, poff, backend);
    Grect { x: border.x, y: border.y, w: nw, h: nh }
}

/// EmuTOS `rc_constrain` (`util/rectfunc.c:32-42`) — pull `pt` back inside `pc`
/// without changing its size: each edge that sticks out moves the origin, the
/// left/top checks first so a `pt` larger than `pc` ends up pinned at `pc`'s
/// far edge.
fn rc_constrain(pc: Grect, pt: &mut Grect) {
    if pt.x < pc.x {
        pt.x = pc.x;
    }
    if pt.y < pc.y {
        pt.y = pc.y;
    }
    if pt.x + pt.w > pc.x + pc.w {
        pt.x = (pc.x + pc.w) - pt.w;
    }
    if pt.y + pt.h > pc.y + pc.h {
        pt.y = (pc.y + pc.h) - pt.h;
    }
}

/// EmuTOS `gr_dragbox` (`aes/gemgrlib.c:243-267`) — drag a fixed-size `w`×`h`
/// XOR box that keeps its distance to the pointer, constrained to `pc`, until
/// the button comes up; returns the final origin.
///
/// The grab offset is taken **once**, before the loop, with
/// `gr_clamp(sx+1, sy+1, 0, 0, &offx, &offy)` (`:253`) — which for
/// `gr_clamp(cursor, origin, min) = max(cursor - origin + 1, min)` is
/// `max(mouse - box_origin, 0)`, i.e. where inside the box the pointer took
/// hold; the box then tracks as `origin = mouse - offset` (`:259-260`) so it
/// does not snap a corner to the pointer.
///
/// Like [`gr_rubwind`] this draws **nothing but the outline**: `gr_wait`
/// (`:169-191`) complements the box, blocks in `gr_stilldn` until the pointer
/// moves or the button comes up, and complements it away again. The window is
/// not moved and no pixel underneath it is repainted — the Control Manager
/// posts `WM_MOVED` and the *owner* applies the new position with
/// `wind_set(WF_CURRXYWH)` (`hctl_window` → `ct_msgup`, `gemctrl.c:237-238,314`).
///
/// This replaces the former solid live-move, which blitted the whole window to
/// its new place once per drag frame (`copy_rect`) and repainted only the
/// vacated L-region with the desktop background. Inside a window underneath,
/// that L is not the desktop's to paint (`do_walk`, `gemwmlib.c:286-312`) and
/// no `WM_REDRAW` asked its owner for it either, so every drag step left the
/// moving window's leading edge behind: the comb of one-pixel vertical lines,
/// one per pointer step, reported from a real Pi 4.
fn gr_dragbox(
    h: &'static dyn Host,
    w: i16,
    hgt: i16,
    sx: i16,
    sy: i16,
    pc: Grect,
    backend: &dyn RasterBackend,
) -> (i16, i16) {
    update_beg(); // wm_update(BEG_UPDATE)
    // `gr_setup(BLACK)`: clip to the whole screen, XOR write mode
    // (`gemgrlib.c:76-80`); the XOR mode lives in `gsx_xbox`.
    let clip = ClipRect::full(backend.width(), backend.height());
    let (mx0, my0) = crate::vdi::mouse_position();
    let offx = (mx0 - sx).max(0);
    let offy = (my0 - sy).max(0);
    let mut o = Grect { x: sx, y: sy, w, h: hgt };
    loop {
        let (mx, my) = crate::vdi::mouse_position();
        o.x = mx - offx;
        o.y = my - offy;
        rc_constrain(pc, &mut o);
        // `gr_wait` (`gemgrlib.c:169-191`) with `poff = gl_rzero`, so
        // `have2box` is FALSE and `gr_draw` emits the single box (`:171,155-166`).
        // The `gsx_moff`/`gsx_mon` brackets are our counted cursor hide/show.
        crate::vdi::cursor_hide(backend);
        gr_draw(o, Grect::default(), backend, clip);
        crate::vdi::cursor_show(backend);
        let down = gr_stilldn(mx, my, h);
        crate::vdi::cursor_hide(backend);
        gr_draw(o, Grect::default(), backend, clip);
        crate::vdi::cursor_show(backend);
        if !down {
            break;
        }
    }
    update_end(); // wm_update(END_UPDATE)
    (o.x, o.y)
}

/// The `W_NAME` arm of EmuTOS `hctl_window` (`aes/gemctrl.c:225-239`): build the
/// constraining rectangle, run [`gr_dragbox`] over the window's **border** rect,
/// and report the dragged origin with the *unchanged* size — the rectangle that
/// goes into the `WM_MOVED` message (`ct_msgup(message, owner, wh, x, y, w, h)`,
/// `gemctrl.c:314`).
///
/// The constraining rectangle is the 3D branch's
/// `r_set(&f, 0, gl_hbox, gl_rscreen.g_w + w - gl_wbox - 6 - 2*ADJ3DSTD,
/// MAX_COORDINATE)` (`:230-232`), i.e. "prevent the mover gadget from being
/// moved completely offscreen": the window may hang off the right edge until
/// only `gl_wbox + 6 + 2*ADJ3DSTD` pixels of the title band are left, may not
/// rise above the menu row, and is effectively unbounded downward
/// (`MAX_COORDINATE` = 10000, `include/sysconf.h:63`). So only two of the four
/// sides are held: `f.g_x` keeps the left edge at 0 and `f.g_y` the top at the
/// menu row, while the window may be pushed off the right edge down to
/// `gl_wbox + 6 + 2*ADJ3DSTD` = 29 pixels of title band, and off the bottom
/// entirely. The old solid drag pinned all four sides to the screen rectangle;
/// EmuTOS does not.
fn drag_move(w: &Window, backend: &dyn RasterBackend) -> Grect {
    let border = w.curr;
    // No host means no input pump, so there is no drag to run: report the
    // geometry unchanged rather than inventing one.
    let Some(h) = host() else { return border };
    let screen = screen_rect();
    let f = Grect {
        x: 0,
        y: GL_HBOX,
        w: screen.w + border.w - GL_WBOX - 6 - 2 * ADJ3DSTD,
        h: MAX_COORDINATE,
    };
    let (nx, ny) = gr_dragbox(h, border.w, border.h, border.x, border.y, f, backend);
    Grect { x: nx, y: ny, w: border.w, h: border.h }
}

/// Live-drag a scroll-bar elevator (thumb) following the mouse until release;
/// returns the final position 0..1000, which the caller posts as `WM_VSLID` /
/// `WM_HSLID` `msg[4]`. Models EmuTOS `gr_slidebox` (`gemgrlib.c:358-410`): the
/// thumb is dragged within its track, held at a constant grip offset from the
/// cursor, and the reported position is `mul_div_round(travel_used, 1000,
/// travel)` (the D-098 slider-creep rounding). Feedback is **thumb-follow** (the
/// slider is repainted at the new spot each frame, bracketed with the counting
/// cursor semaphore); EmuTOS instead drags an XOR outline of the elevator inside
/// its slide box (`gr_slidebox` → `gr_dragbox`, `gemgrlib.c:388`). That is a
/// remaining deviation, tracked separately — unlike the mover and the sizer it
/// repaints only the slider gadget, never the window or what is under it.
/// The window's stored slider position is advanced so the thumb tracks the
/// cursor; the owner then normally confirms it via `wind_set(WF_?SLIDE, pos)`.
fn drag_elevator(w: &mut Window, vert: bool, backend: &dyn RasterBackend) -> i16 {
    let (slsize, slpos) = if vert { (w.vslsize, w.vslide) } else { (w.hslsize, w.hslide) };
    let Some(h) = host() else { return slpos };
    let Some((track_start, track_len)) = (if vert { vtrack(w) } else { htrack(w) }) else {
        return slpos;
    };
    let (thumb_off, thumb_len) = thumb_geom(track_len, slsize, slpos);
    let travel = track_len - thumb_len;
    // Grip: how far the cursor sits from the thumb's leading edge at grab, held
    // constant so the thumb does not snap its edge to the cursor (EmuTOS
    // `gr_dragbox` `gr_clamp(sx+1, sy+1, 0, 0)` → `max(mouse - origin, 0)`).
    let (mx0, my0) = crate::vdi::mouse_position();
    let thumb_start = track_start + thumb_off;
    let grip = ((if vert { my0 } else { mx0 }) - thumb_start).max(0);
    let clip = ClipRect::full(backend.width(), backend.height());
    // The `loop` always runs at least once, assigning `pos` before it can break,
    // so a seed value would be a dead store — declare it deferred instead.
    let mut pos;
    loop {
        let (mx, my) = crate::vdi::mouse_position();
        let m = if vert { my } else { mx };
        // New thumb leading edge, constrained to the track [start, start+travel].
        let new_start = (m - grip).clamp(track_start, track_start + travel);
        let rel = new_start - track_start; // travel used, 0..travel
        pos = if travel > 0 {
            super::graf::mul_div_round(rel as i32, 1000, travel as i32) as i16
        } else {
            0
        };
        if vert {
            w.vslide = pos;
        } else {
            w.hslide = pos;
        }
        crate::vdi::cursor_hide(backend);
        if vert {
            draw_vslider(w, backend, clip);
        } else {
            draw_hslider(w, backend, clip);
        }
        crate::vdi::cursor_show(backend);
        if crate::vdi::mouse_buttons() & 0x01 == 0 {
            break;
        }
        h.pump_input();
    }
    pos
}

/// Block until the left button is released (used after edge-triggered
/// gadgets like topped/arrowed so one press fires once).
fn wait_release() {
    if let Some(h) = host() {
        while crate::vdi::mouse_buttons() & 0x01 != 0 {
            h.pump_input();
        }
    }
}

/// Handle of the open window under `(mx,my)`, **top-most first** (the highest
/// z among the windows whose border rect contains the point); 0 = the desktop
/// window, **−1 = nobody**.
///
/// Mirrors EmuTOS `wm_find` = `ob_find(gl_wtree, 0, 2, x, y)`
/// (`gemsuper.c:348-350`, `gemwmlib.c:1767-1770`), which walks the window tree
/// strictly top-to-bottom from the ROOT. `ob_find` starts with
/// `lastfound = NIL` (`gemoblib.c:1044`, `NIL = -1`, `obdefs.h:21`) and only
/// assigns a handle once the point is found **inside** an object's rectangle —
/// and the ROOT of the window tree carries the desktop window's `WS_CURR`, i.e.
/// `gl_rscreen`. So a point off the screen belongs to no window at all: the
/// desktop is *not* an unconditional fallback.
/// EmuTOS `chk_ctrl` (`aes/geminput.c:86-104`) — who owns the part of the
/// screen the point falls in: `+1` the active process (inside the control
/// rectangle), `-1` the Control Manager (menu bar, or any window), `0` the
/// desktop. `chkown` turns that into an `AESPD *` on the button-down edge
/// (`:310-322`); at single-app `+1` and `0` are both the running application.
///
/// The control rectangle is EmuTOS's `ctrl`, which has exactly two sources:
/// `fm_own` sets it to the whole screen for as long as `ml_ocnt > 0`
/// (`ct_chgown(rlr, &gl_rscreen)`, `aes/gemfmlib.c:93`), and otherwise the
/// Control Manager's `w_setactive` keeps it at the **work** area of the top
/// window (`aes/gemwmlib.c:326-333`). Both are read live here instead of being
/// mirrored in a `set_ctrl` variable — [`MCTRL_CNT`] is `ml_ocnt` and
/// [`get_top`] is `w_top`, so the value is the same one `set_ctrl` would hold.
///
/// The menu-bar test is `inside(mx, my, &gl_rmenu)`, and `gl_rmenu` is the top
/// character row of the screen whether or not a menu is installed
/// (`vdi`-independent, `aes/gemgraf.c:427`) — [`super::menu::desk_top`] is that
/// row's height, and no window's work area reaches into it because `gl_rfull`
/// excludes it. `wm_find` is [`find_handle`]; it answers `DESKWH` (0) for the
/// desktop, which is EmuTOS's `wh == 0` "not a window" case.
pub(super) fn chk_ctrl(mx: i16, my: i16) -> i16 {
    let ctrl = if MCTRL_CNT.load(Ordering::Relaxed) > 0 {
        screen_rect()
    } else {
        match win(get_top()) {
            Some(w) => w.work,
            None => screen_rect(),
        }
    };
    if inside(ctrl, mx, my) {
        return 1;
    }
    if my < super::menu::desk_top() {
        return -1;
    }
    if find_handle(mx, my) > DESKWH {
        -1
    } else {
        0
    }
}

fn find_handle(mx: i16, my: i16) -> i16 {
    let desk = slots()[DESKWH as usize].curr;
    let mut best = if inside(desk, mx, my) { DESKWH } else { -1 };
    let mut best_z = i16::MIN;
    for (i, w) in slots().iter().enumerate() {
        if w.in_use && w.open && inside(w.curr, mx, my) && w.z >= best_z {
            best_z = w.z;
            best = i as i16;
        }
    }
    best
}

/// AES window-frame interception (the `evnt_multi` hook, mirroring the EmuTOS
/// Control Manager's `hctl_window`, `gemctrl.c:158`). On a button press in a
/// window frame, handle the gadget and post the resulting `WM_*` message into
/// the window **owner's** per-app FIFO — the EmuTOS `ct_msgup`→`ap_sendmsg`→
/// `aqueue`/`doq` path (`gemctrl.c:81-102`, `gemwmlib.c:851-863`,
/// `gemqueue.c:85-125`), exactly the routing `send_redraw` already uses. The
/// caller (`evnt_multi`) then delivers it through the regular `MU_MESAG`
/// dequeue, so frame messages obey FIFO order (and same-window redraw / arrow
/// coalescing) instead of clobbering a message the app has not yet read
/// (AUD-P2-001). A click in the work area (or on the desktop) is a no-op.
pub fn intercept() {
    // While an app holds mouse control via `wind_update(BEG_MCTRL)`
    // (`ml_ocnt > 0`), the app has taken over the mouse and the AES frame
    // interception switches off — EmuTOS `fm_own` hands the control region to the
    // app so `ctlmgr` no longer processes frame clicks (gemfmlib.c:82-107). See
    // [`wind_update`]; this is the hook P4-011's full `fm_own` extends.
    if MCTRL_CNT.load(Ordering::Relaxed) > 0 {
        return;
    }
    if crate::vdi::mouse_buttons() & 0x01 == 0 {
        return;
    }
    let (mx, my) = crate::vdi::mouse_position();
    let handle = find_handle(mx, my);
    // 0 = the desktop window, −1 = no window at all: neither has a frame to
    // intercept.
    if handle <= DESKWH {
        return;
    }
    let Some(backend) = host().and_then(|h| h.raster()) else {
        return;
    };
    // The FIFO target is the window owner (EmuTOS `ct_msgup`'s `owner` =
    // `pwin->w_owner`, `gemctrl.c:314`); at single-app this is the running app.
    let owner = match win(handle) {
        Some(w) => w.owner,
        None => return,
    };

    let mut msg = [0i16; 8];
    // `ap_sendmsg` sets `msg[1] = rlr->p_pid`, the sender pid (`gemwmlib.c:855`);
    // for ctlmgr messages that is the control process, at single-app the app.
    msg[1] = super::appl::current_pid();
    msg[3] = handle;

    // Click on a non-top window → tell the old top's owner it lost the top spot
    // and the clicked window's owner to bring itself up, then **return without
    // topping anything**. EmuTOS `hctl_window` (`aes/gemctrl.c:170-178`):
    //
    //     if (w_handle != gl_wtop) {
    //         perform_untop(gl_wtop);
    //         ct_msgup(WM_TOPPED, pwin->w_owner, w_handle, 0, 0, 0, 0);
    //         return;
    //     }
    //
    // The topping is the *owner's* to request with `wind_set(WF_TOP)` (TeraDesk
    // `wd_type_topped`, `sdk/apps/teradesk/window.c:2375-2378`; our desktop's
    // `top_msg`, `kernel/src/desk/main.rs:1396-1406`). Doing it here as well
    // made the owner's `WF_TOP` a no-op on an already-topped window — and the
    // repaint it triggered took the window's content with it.
    if handle != get_top() {
        let old_top = get_top();
        msg[0] = WM_TOPPED;
        wait_release();
        post_untopped(old_top, handle);
        post_to_owner(owner, &msg);
        return;
    }

    let w = match win(handle) {
        Some(w) => w,
        None => return,
    };
    match gadget_at(w, mx, my) {
        Gadget::Closer => {
            if !watchbox(gad_rect(w, Gadget::Closer), G_CLOSE, backend) {
                return;
            }
            msg[0] = WM_CLOSED;
        }
        Gadget::Fuller => {
            if !watchbox(gad_rect(w, Gadget::Fuller), G_FULL, backend) {
                return;
            }
            msg[0] = WM_FULLED;
        }
        Gadget::Mover => {
            let r = drag_move(w, backend);
            msg[0] = WM_MOVED;
            msg[4] = r.x;
            msg[5] = r.y;
            msg[6] = r.w;
            msg[7] = r.h;
        }
        Gadget::Sizer => {
            let r = drag_size(w, backend);
            msg[0] = WM_SIZED;
            msg[4] = r.x;
            msg[5] = r.y;
            msg[6] = r.w;
            msg[7] = r.h;
        }
        // Scroll arrows and track-page clicks both post WM_ARROWED with a
        // WA_* sub-action (EmuTOS `handle_arrow_msg`, gemctrl.c:111-139). Like
        // the existing arrow arms these fire once per press (our single-tasking
        // model does not run the cooperative auto-repeat loop; the page click
        // is edge-triggered, same as the line arrows already are here).
        Gadget::Up
        | Gadget::Dn
        | Gadget::Lf
        | Gadget::Rt
        | Gadget::VUpPage
        | Gadget::VDnPage
        | Gadget::HLfPage
        | Gadget::HRtPage => {
            msg[0] = WM_ARROWED;
            msg[4] = match gadget_at(w, mx, my) {
                Gadget::Up => WA_UPLINE,
                Gadget::Dn => WA_DNLINE,
                Gadget::Lf => WA_LFLINE,
                Gadget::Rt => WA_RTLINE,
                Gadget::VUpPage => WA_UPPAGE,
                Gadget::VDnPage => WA_DNPAGE,
                Gadget::HLfPage => WA_LFPAGE,
                _ => WA_RTPAGE,
            };
            wait_release();
        }
        // Elevator-thumb drag → post the final 0..1000 position as WM_VSLID /
        // WM_HSLID (EmuTOS `hctl_window` W_?ELEV, gemctrl.c:295-306).
        Gadget::VElev => {
            msg[0] = WM_VSLID;
            msg[4] = drag_elevator(w, true, backend);
        }
        Gadget::HElev => {
            msg[0] = WM_HSLID;
            msg[4] = drag_elevator(w, false, backend);
        }
        Gadget::None => return,
    }
    post_to_owner(owner, &msg);
}

/// Post a Control-Manager `WM_*` message into `owner`'s per-app FIFO — the
/// EmuTOS `ap_sendmsg`→`aqueue`/`doq` enqueue path (`gemwmlib.c:851-863`,
/// `gemqueue.c:85-125`), the same routing [`send_redraw`] uses. This replaces
/// the former direct-`pbuff` write so frame messages queue in FIFO order (and
/// `WM_ARROWED` coalesces) instead of overwriting a message still unread by
/// the app (AUD-P2-001).
fn post_to_owner(owner: i16, msg: &[i16; 8]) {
    // SAFETY: single trap path; `post_msg` hands the message to the live slot.
    unsafe {
        super::pd::post_msg(owner, *msg);
    }
}

/// Post `WM_UNTOPPED` (30) to the previously-topped window's owner when the top
/// changes — EmuTOS `perform_untop` (gemctrl.c:146-155): the old top's owner is
/// told its window is no longer active (`msg[3]` = old top handle). No message
/// when `old_top` is the same window being (re-)topped or is not a real window
/// (0 = desktop / none). Routed through the owner FIFO like every other WM_*
/// (no `pbuff` bypass, AUD-P2-001).
fn post_untopped(old_top: i16, new_top: i16) {
    if old_top == new_top || old_top == DESKWH {
        return;
    }
    if let Some(owner) = win(old_top).map(|w| w.owner) {
        let mut msg = [0i16; 8];
        msg[0] = WM_UNTOPPED;
        // `ap_sendmsg` sets msg[1] = sender pid (gemwmlib.c:855); at single-app
        // this is the running app / control process.
        msg[1] = super::appl::current_pid();
        msg[3] = old_top;
        post_to_owner(owner, &msg);
    }
}

/// Test hook (not wired to boot): the message at the **front** of process
/// `pid`'s FIFO queue (without removing it), or `None` if the queue is empty.
/// Lets the P5 redraw-broadcast selftest observe the `WM_REDRAW` posted on
/// `wind_close` / `wind_set(WF_TOP)`.
pub fn test_pending_msg(pid: i16) -> Option<[i16; 8]> {
    // SAFETY: single-CPU AES; test scaffolding on the boot trap path.
    unsafe { super::pd::by_pid(pid).and_then(|p| p.queue.front()) }
}

/// Test hook (not wired to boot): number of messages queued for `pid` (0 when
/// empty). Lets a selftest assert the WM_REDRAW rectangle-union coalesces two
/// same-window redraws into a single queue entry.
pub fn test_msg_count(pid: i16) -> usize {
    // SAFETY: single-CPU AES; test scaffolding on the boot trap path.
    unsafe { super::pd::by_pid(pid).map(|p| p.queue.pending()).unwrap_or(0) }
}

/// Test hook (not wired to boot): drop every pending message for `pid`, so the
/// P5 selftest can isolate the `WM_REDRAW` a subsequent window op posts.
pub fn test_clear_msg(pid: i16) {
    // SAFETY: single-CPU AES; test scaffolding on the boot trap path.
    unsafe {
        if let Some(p) = super::pd::by_pid(pid) {
            p.queue.clear();
        }
    }
}

/// Test hook: name of the gadget under a point (for the headless logic
/// check in the selftest).
pub fn gadget_name_at(handle: i16, mx: i16, my: i16) -> &'static str {
    match win(handle) {
        Some(w) => match gadget_at(w, mx, my) {
            Gadget::None => "none",
            Gadget::Closer => "closer",
            Gadget::Fuller => "fuller",
            Gadget::Mover => "mover",
            Gadget::Sizer => "sizer",
            Gadget::Up => "up",
            Gadget::Dn => "dn",
            Gadget::Lf => "lf",
            Gadget::Rt => "rt",
            Gadget::VElev => "velev",
            Gadget::HElev => "helev",
            Gadget::VUpPage => "vuppage",
            Gadget::VDnPage => "vdnpage",
            Gadget::HLfPage => "hlfpage",
            Gadget::HRtPage => "hrtpage",
        },
        None => "no-window",
    }
}
