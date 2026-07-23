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

use super::object::{render_tree_lp64, Object};
use super::Aespb;
use crate::host::host;
use crate::vdi::backend::{ClipRect, Pel, PatternFill, RasterBackend, Rect, WriteMode};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicI16, AtomicU64, Ordering};

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

// ── WF_* fields, aesdefs.h:534-552 ──
const WF_KIND: i16 = 1;
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

/// Gadget cell size (`gl_hbox` = char height + 3; square gadgets). This is
/// also EmuTOS `gl_wbox`/`gl_hbox`: for the 8×16 system font both are 19
/// (`gl_hbox = gl_hchar+3 = 19`, `gl_wbox = max(gl_wchar+4, gl_hbox·aspect) =
/// 19`, `gemgraf.c:412-415`), so `graf::graf_handle` returns this same unit as
/// its box metric instead of a second magic 19.
pub(super) const GAD: i16 = 19;

const WHITE: Pel = 0;
const BLACK: Pel = 1;
const GRAY_LT: Pel = 8; // 3D face (EmuTOS LWHITE)
const GRAY_DK: Pel = 9; // 3D shadow (EmuTOS LBLACK)

/// The classic GEM "move-bar" dither on an **active** window's name area
/// (EmuTOS `TOPPED_COLOR` = `IP_2PATT`, a light 25 % black stipple). The
/// inactive window's name bar is left plain — the presence of the stipple is
/// the active/inactive cue, matching EmuTOS.
const TITLE_STIPPLE: [u16; 4] = [0x0000, 0x5555, 0x0000, 0x5555];

const MAX_WINDOWS: usize = 8;
const NAME_CAP: usize = 80;

/// The desktop pseudo-window's AES handle (EmuTOS `DESKWH`) — the bottom of the
/// window z-order. It has no table slot; its rectangle list is the desk area
/// (the screen minus the menu bar) minus every open window's border rect.
const DESKWH: i16 = 0;

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
    slots: UnsafeCell<[Window; MAX_WINDOWS]>,
    next_z: UnsafeCell<i16>, // monotonically increasing stacking counter
}
// SAFETY: only the single boot CPU touches the AES; no concurrent access.
unsafe impl Sync for WinTable {}

static TABLE: WinTable = WinTable {
    slots: UnsafeCell::new([Window::EMPTY; MAX_WINDOWS]),
    next_z: UnsafeCell::new(1),
};

/// Saved copy of the desktop's window set for the single-tasking **screen
/// handoff** ([`save_windows`] / [`restore_windows`]). Disjoint from [`TABLE`]:
/// [`save_windows`] snapshots the live table here and clears the live one so a
/// launched GEM app gets a clean window list; [`restore_windows`] copies it back
/// on the child's `Pterm`. `active` guards a restore with no prior save.
struct WinSave {
    slots: UnsafeCell<[Window; MAX_WINDOWS]>,
    next_z: UnsafeCell<i16>,
    active: UnsafeCell<bool>,
}
// SAFETY: only the single boot CPU touches the AES; no concurrent access.
unsafe impl Sync for WinSave {}

static SAVE: WinSave = WinSave {
    slots: UnsafeCell::new([Window::EMPTY; MAX_WINDOWS]),
    next_z: UnsafeCell::new(1),
    active: UnsafeCell::new(false),
};

#[allow(clippy::mut_from_ref)]
fn slots() -> &'static mut [Window; MAX_WINDOWS] {
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
            best = (i + 1) as i16;
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
            best = (i + 1) as i16;
        }
    }
    best
}

/// Raise `handle` to the top of the stack and repaint only the union of the
/// newly-topped window and the previously-top window (which loses its
/// active-title highlight) — not the whole screen (the topping-flicker fix,
/// same damage-scope as move/open/close).
fn top_window(handle: i16) {
    let old_top = get_top(); // current top before the bump (may equal handle)
    let old_rect = win(old_top).map(|w| w.curr);
    if let Some(w) = win(handle) {
        w.z = bump_z();
    }
    let new_rect = win(handle).map(|w| w.curr);
    let damage = match (new_rect, old_rect) {
        (Some(n), Some(o)) => Some(union_grect(n, o)),
        (Some(n), None) => Some(n),
        _ => None,
    };
    if let Some(d) = damage {
        redraw_clip(d.x, d.y, d.w, d.h);
    }
}

/// The registered desktop-background object tree (EmuTOS `gl_newdesk`,
/// `gemwmlib.c:54`): the base VA of the desktop application's own `OBJECT`
/// array, set via `wind_set(DESKWH, WF_NEWDESK, tree, 1, root)`. `0` = none →
/// [`paint_bg_clip`] draws the AES-internal fallback root instead.
///
/// This is a **native (LP64) `Object` array** — the desktop's `g_screen`, a
/// kernel-built tree — so [`paint_bg_clip`] reads it via `render_tree_lp64`
/// (forced 32-byte records), valid even while an ILP32 app is the current
/// process. It stays stable for the desktop's lifetime (the arena is never
/// reallocated), so holding the raw pointer across draws is sound.
static NEWDESK_TREE: AtomicU64 = AtomicU64::new(0);

/// The object index within [`NEWDESK_TREE`] that the background composite draws
/// from (EmuTOS `gl_newroot`, `gemwmlib.c:55`; the desktop passes its `DROOT`).
static NEWDESK_ROOT: AtomicI16 = AtomicI16::new(0);

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
        NEWDESK_ROOT.store(root, Ordering::Release);
        NEWDESK_TREE.store(tree, Ordering::Release);
    }
}

/// The AES-internal background fallback root's `ob_spec` word (gr_crack:
/// border/text WHITE, pattern `IP_SOLID`(7), fill colour WHITE(0), border
/// thickness 0 → an opaque white rectangle, no frame). This is the AES's
/// desktop-background box when no desktop tree is registered — the
/// `CLEAR_SCREEN` white-out (`sh_draw(cmd, TRUE)`, `gemshlib.c:280`).
const FALLBACK_ROOT_OBSPEC: u64 = 0x0070;

/// Build the AES-internal background **fallback root** (EmuTOS `gl_wtree[ROOT]`,
/// object *B*): a full-screen drawable `G_BOX` drawn when no desktop tree is
/// registered ([`NEWDESK_TREE`] == 0). Its `ob_spec` is a solid-white fill
/// ([`FALLBACK_ROOT_OBSPEC`]) so drawing it erases the clip to white — the
/// `CLEAR_SCREEN` white-out EmuTOS shows between the desktop and a launching GEM
/// app (`sh_draw(cmd, TRUE)`, `gemshlib.c:280`). Built fresh per call so it
/// always spans the current framebuffer.
fn fallback_root(w: i16, h: i16) -> Object {
    Object {
        ob_next: -1,
        ob_head: -1,
        ob_tail: -1,
        ob_type: 20, // G_BOX
        ob_flags: 0,
        ob_state: 0,
        ob_spec: FALLBACK_ROOT_OBSPEC,
        ob_x: 0,
        ob_y: 0,
        ob_width: w,
        ob_height: h,
    }
}

/// The `ob_spec` word of the object the AES currently draws as the desktop
/// background **ROOT** — NewTOS's equivalent of EmuTOS `rs_trees[DESKTOP][ROOT]
/// .ob_spec` (`gemsuper.c:96`), the desktop background pattern/colour word that
/// `appl_init` returns in `ap_private`.
///
/// When the desktop has registered its background tree ([`NEWDESK_TREE`] != 0,
/// via `wind_set(DESKWH, WF_NEWDESK, tree, root)`), this is that tree's
/// registered **root object's** `ob_spec` (the desktop's `g_screen[DROOT]
/// .ob_spec = g_patcol.desktop | BORDER_TEXT_COLOURS`). Real GEM apps (QED) call
/// `appl_init` after the desktop is up, so they read this live value — exactly
/// as an EmuTOS app reads the AES's pre-set DESKTOP-root `ob_spec` (which
/// `set_aes_background`, `geminit.c:635-644`, keeps in sync with the background).
///
/// Before the desktop registers WF_NEWDESK — only the desktop's **own** early
/// `appl_init` (run before it registers its tree, `deskmain.c:1953` vs `:2049`)
/// reaches this — no tree is registered, so the AES background genuinely *is*
/// the internal white [`fallback_root`] (`CLEAR_SCREEN`); its
/// [`FALLBACK_ROOT_OBSPEC`] is returned. That is the truthful current background
/// word, not a fabricated value: EmuTOS returns its pre-set DESKTOP pattern
/// there only because its AES `gem.rsc` DESKTOP root is fixed during `geminit`
/// (before the desktop process runs); NewTOS's AES owns no static DESKTOP
/// resource, so at that instant the background is the white fallback. (The
/// desktop's own `appl_init` discards `ap_private` anyway — `desk/mod.rs:170`
/// returns only the id — so this pre-registration value is unobservable to any
/// real consumer.)
pub(crate) fn desktop_bg_obspec() -> u64 {
    let tree = NEWDESK_TREE.load(Ordering::Acquire);
    if tree == 0 {
        return FALLBACK_ROOT_OBSPEC;
    }
    let root = NEWDESK_ROOT.load(Ordering::Acquire).max(0) as usize;
    // SAFETY: `tree` is the desktop's `g_screen` base, registered via
    // WF_NEWDESK — a valid native (LP64) `Object` array kept stable for the
    // desktop's lifetime (see [`NEWDESK_TREE`]); `root` is a valid object index
    // in it (the registered `DROOT`). We read the single 32-byte record's
    // `ob_spec` (offset 16) only. Read directly at the native LP64 layout,
    // like [`paint_bg_clip`]'s `render_tree_lp64`, regardless of the current
    // app's ABI.
    unsafe { (*(tree as *const Object).add(root)).ob_spec }
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
/// removal; the caller then clears the screen (a full composite over the empty
/// table with no registered desktop tree draws the white `CLEAR_SCREEN`
/// fallback root).
pub fn save_windows() {
    // SAFETY: single-CPU AES; `SAVE` and `TABLE` are disjoint statics and
    // `[Window; MAX_WINDOWS]` is `Copy`, so these are plain array copies.
    unsafe {
        *SAVE.slots.get() = *TABLE.slots.get();
        *SAVE.next_z.get() = *TABLE.next_z.get();
        *SAVE.active.get() = true;
        // Hide every desktop window by resetting the live table (its state is
        // held in `SAVE`); reset the z counter for the child.
        *TABLE.slots.get() = [Window::EMPTY; MAX_WINDOWS];
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
/// (EmuTOS `gemwmlib.c:340-360`).
///
/// If a desktop tree is registered ([`NEWDESK_TREE`] != 0, via
/// `wind_set(DESKWH, WF_NEWDESK, …)`), draw that tree from its root
/// ([`NEWDESK_ROOT`], the desktop's `DROOT`) at **`MAX_DEPTH`** — the `DROOT`
/// background box **and every icon child** — exactly as EmuTOS draws
/// `gl_newdesk` at `MAX_DEPTH`. Otherwise draw the AES-internal [`fallback_root`]
/// (a white `CLEAR_SCREEN` box) at depth 0. Both go through the ordinary object
/// manager ([`render_tree_lp64`] = the non-trapping `ob_draw` engine), clipped
/// to `clip` — never a bespoke fill.
///
/// Runs **inside** `compose`/`redraw_all` (the AES SVC handler), so it must not
/// re-enter the AES trap: `render_tree_lp64` draws straight through the backend.
fn paint_bg_clip(backend: &dyn RasterBackend, clip: ClipRect) {
    let cw = (clip.xmx - clip.xmn + 1).max(0);
    let ch = (clip.ymx - clip.ymn + 1).max(0);
    if cw == 0 || ch == 0 {
        return;
    }
    let tree = NEWDESK_TREE.load(Ordering::Acquire);
    if tree != 0 {
        let root = NEWDESK_ROOT.load(Ordering::Acquire);
        // SAFETY: `tree` is the desktop's `g_screen` base, registered via
        // WF_NEWDESK; it is a valid native OBJECT array kept stable for the
        // desktop's lifetime, `root` is in range.
        unsafe {
            render_tree_lp64(tree as *const Object, root, MAX_DEPTH, clip.xmn, clip.ymn, cw, ch, backend);
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
    let mut drawn = [false; MAX_WINDOWS];
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
        let istop = (pick as i16 + 1) == top;
        draw_frame(&slots()[pick], istop, backend, clip);
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

/// Compute `handle`'s visible work-area pieces (work rect minus every
/// higher-z open window) into `RLIST`, cursor reset.
///
/// [`DESKWH`] (the desktop pseudo-window) is treated as the **bottom** of the
/// z-order: its work area is the desk rectangle ([`desktop_work`], the screen
/// minus the menu bar) and *every* open window is above it, so the subtraction
/// loop removes each open window's border rect. This is the occlusion-aware
/// rectangle list EmuTOS's `do_wredraw(DESKWH, …)` walks uniformly with real
/// windows (`desksupp.c:223`), so the desktop background never reports — and
/// therefore never repaints over — a piece a window covers.
fn compute_rlist(handle: i16) {
    // SAFETY: single-CPU AES.
    let (rects, count, cursor) = unsafe {
        (&mut *RLIST.rects.get(), &mut *RLIST.count.get(), &mut *RLIST.cursor.get())
    };
    *count = 0;
    *cursor = 0;
    let (work, myz) = if handle == DESKWH {
        // Desktop: desk area, z below every window (i16::MIN → all subtracted).
        (desktop_work(), i16::MIN)
    } else {
        match win(handle) {
            Some(w) if w.open => (calc(WC_WORK, w.kind, w.curr), w.z),
            _ => return,
        }
    };
    let mut list = [Grect::default(); RCAP];
    list[0] = work;
    let mut n = 1;
    for ow in slots().iter() {
        if ow.in_use && ow.open && ow.z > myz {
            n = subtract(&mut list, n, ow.curr);
        }
    }
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
    let (work, myz) = match win(handle) {
        Some(w) if w.open => (calc(WC_WORK, w.kind, w.curr), w.z),
        _ => return Grect::default(),
    };
    let mut list = [Grect::default(); RCAP];
    list[0] = work;
    let mut n = 1;
    for ow in slots().iter() {
        if ow.in_use && ow.open && ow.z > myz {
            n = subtract(&mut list, n, ow.curr);
        }
    }
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
    for i in 0..MAX_WINDOWS {
        let handle = (i + 1) as i16;
        if win(handle).map(|w| w.open).unwrap_or(false) {
            send_redraw_in(handle, damage);
        }
    }
}

/// Map a window handle (1-based) to a table slot, if valid and in use.
fn win(handle: i16) -> Option<&'static mut Window> {
    if handle < 1 || handle as usize > MAX_WINDOWS {
        return None;
    }
    let w = &mut slots()[(handle - 1) as usize];
    w.in_use.then_some(w)
}

// ── geometry ────────────────────────────────────────────────────────────

/// Border thickness contributed by `kind` on each side: `(left, top,
/// right, bottom)` (EmuTOS `wm_calc`, non-3D path).
fn borders(kind: u16) -> (i16, i16, i16, i16) {
    let lb = 1;
    let mut tb = 1;
    let mut rb = 1;
    let mut bb = 1;
    if kind & (NAME | CLOSER | FULLER) != 0 {
        tb += GAD - 1;
    }
    if kind & INFO != 0 {
        tb += GAD - 1;
    }
    if kind & (VGADGETS | SIZER) != 0 {
        rb += GAD - 1;
    }
    if kind & (HGADGETS | SIZER) != 0 {
        bb += GAD - 1;
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

    let mut top = b.y + 1; // running top of the next band

    // Title row (name area + closer + fuller). EmuTOS: white base, the name
    // area carries a light dither on the *active* window (the classic
    // move-bar look), plain on the inactive one.
    if kind & (NAME | CLOSER | FULLER) != 0 {
        let bar = Grect { x: b.x + 1, y: top, w: b.w - 2, h: GAD - 1 };
        backend.fill_rect(bar.x, bar.y, bar.w, bar.h, WHITE, clip);

        let mut name_x = bar.x;
        let mut name_w = bar.w;
        if kind & CLOSER != 0 {
            let g = Grect { x: b.x, y: top - 1, w: GAD, h: GAD };
            gadget(g, G_CLOSE, backend, clip);
            name_x += GAD - 1;
            name_w -= GAD - 1;
        }
        if kind & FULLER != 0 {
            let g = Grect { x: b.x + b.w - GAD, y: top - 1, w: GAD, h: GAD };
            gadget(g, G_FULL, backend, clip);
            name_w -= GAD - 1;
        }
        // Active window: the classic move-bar dither behind the name.
        if istop {
            backend.fill_pattern(
                name_x,
                top,
                name_w,
                GAD - 1,
                PatternFill { fg: BLACK, bg: WHITE, pattern: &TITLE_STIPPLE, mode: WriteMode::Replace },
                clip,
            );
        }
        // Window name (centred when there is room), on a small white box so it
        // stays crisp over the dither (EmuTOS clears the name's glyph cells).
        let nlen = name_len(&w.name) as i16 * 8;
        let nx = name_x + ((name_w - nlen) / 2).max(2);
        if istop && nlen > 0 {
            backend.fill_rect((nx - 3).max(name_x), top, nlen + 6, GAD - 1, WHITE, clip);
        }
        draw_label(nx, top, name_w, &w.name, backend, clip);
        // separator under the title row
        backend.fill_rect(b.x, top + GAD - 1, b.w, 1, BLACK, clip);
        top += GAD - 1;
    }

    // Info line.
    if kind & INFO != 0 {
        let band = Grect { x: b.x + 1, y: top, w: b.w - 2, h: GAD - 1 };
        backend.fill_rect(band.x, band.y, band.w, band.h, GRAY_LT, clip);
        draw_label(band.x + 2, top, band.w - 2, &w.info, backend, clip);
        backend.fill_rect(b.x, top + GAD - 1, b.w, 1, BLACK, clip);
        top += GAD - 1;
    }

    // Work area (white client region). Its 1 px black frame is drawn by the
    // content owner *after* the content (the desktop's `draw_dir_listing`),
    // because the app clears the work area on every content redraw and would
    // otherwise overdraw a border painted here.
    let work = calc(WC_WORK, kind, b);
    backend.fill_rect(work.x, work.y, work.w, work.h, WHITE, clip);

    // Right scroll bar (arrows + track + thumb).
    draw_vslider(w, backend, clip);

    // Bottom scroll bar (arrows + track + thumb). Fill the whole bottom band
    // first: with a sizer but no horizontal scrollbar the band would otherwise
    // be left unpainted, showing the desktop through the bottom of the window.
    // The band fill lives here (not in `draw_hslider`) so the slider-only
    // redraw path never erases the sizer corner it does not repaint.
    if kind & (HGADGETS | SIZER) != 0 {
        let row_y = b.y + b.h - GAD;
        backend.fill_rect(b.x + 1, row_y, b.w - 2, GAD - 1, GRAY_LT, clip);
    }
    draw_hslider(w, backend, clip);

    // Sizer (bottom-right corner).
    if kind & SIZER != 0 {
        gadget(
            Grect { x: b.x + b.w - GAD, y: b.y + b.h - GAD, w: GAD, h: GAD },
            G_SIZE,
            backend,
            clip,
        );
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
    let min_thumb = GAD.min(track_len);
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

/// The vertical scroll track span `(top, length)` — the region left for the
/// thumb + paging after the up/down arrows — or `None` if there is no VSLIDE
/// track. Mirrors [`draw_vslider`]'s arrow-band offsets so the hit-test /
/// drag ([`gadget_at`]/[`drag_elevator`]) agree pixel-for-pixel with the draw.
fn vtrack(w: &Window) -> Option<(i16, i16)> {
    let b = w.curr;
    let kind = w.kind;
    if kind & VSLIDE == 0 {
        return None;
    }
    let mut top = b.y + 1;
    if kind & (NAME | CLOSER | FULLER) != 0 {
        top += GAD - 1;
    }
    if kind & INFO != 0 {
        top += GAD - 1;
    }
    let mut bot = b.y + b.h - 1;
    if kind & (HGADGETS | SIZER) != 0 {
        bot -= GAD - 1;
    }
    if kind & UPARROW != 0 {
        top += GAD - 1;
    }
    if kind & DNARROW != 0 {
        bot -= GAD - 1;
    }
    (bot > top).then_some((top, bot - top))
}

/// Draw the right (vertical) scroll bar — up/down arrows, the track, and a
/// thumb positioned + sized from `w.vslide` / `w.vslsize` (both per-mille).
/// Shared by `draw_frame` and `wind_set(WF_VSLIDE/WF_VSLSIZ)`; drawing only
/// the slider column leaves the work area (content) untouched.
fn draw_vslider(w: &Window, backend: &dyn RasterBackend, clip: ClipRect) {
    let b = w.curr;
    let kind = w.kind;
    if kind & (VGADGETS | SIZER) == 0 {
        return;
    }
    let col_x = b.x + b.w - GAD;
    // Arrow positions: top below the title + info bands, bottom above the
    // sizer/H-bar band (same offsets `vtrack` uses for the track between them).
    let mut sy = b.y + 1;
    if kind & (NAME | CLOSER | FULLER) != 0 {
        sy += GAD - 1;
    }
    if kind & INFO != 0 {
        sy += GAD - 1;
    }
    let mut send = b.y + b.h - 1;
    if kind & (HGADGETS | SIZER) != 0 {
        send -= GAD - 1;
    }
    if kind & UPARROW != 0 {
        gadget(Grect { x: col_x, y: sy, w: GAD, h: GAD }, G_UP, backend, clip);
    }
    if kind & DNARROW != 0 {
        gadget(Grect { x: col_x, y: send - GAD + 1, w: GAD, h: GAD }, G_DN, backend, clip);
    }
    if let Some((track_y, track_h)) = vtrack(w) {
        backend.fill_rect(col_x, track_y, GAD, track_h, GRAY_LT, clip);
        rect_outline(Grect { x: col_x, y: track_y, w: GAD, h: track_h }, backend, clip);
        // Thumb geometry shared with `gadget_at`/`drag_elevator` (see
        // [`thumb_geom`]): a stored −1 size renders as one gadget cell, never a
        // zero/negative-height elevator; the stored/returned value stays −1.
        let (off, thumb_h) = thumb_geom(track_h, w.vslsize, w.vslide);
        let thumb_y = track_y + off;
        backend.fill_rect(col_x, thumb_y, GAD, thumb_h, GRAY_LT, clip);
        bevel(Grect { x: col_x, y: thumb_y, w: GAD, h: thumb_h }, true, backend, clip);
    }
}

/// Redraw just window `handle`'s vertical slider (no work-area clear), for
/// `wind_set(WF_VSLIDE/WF_VSLSIZ)`.
fn redraw_vslider(handle: i16) {
    if let (Some(backend), Some(w)) = (host().and_then(|h| h.raster()), win(handle)) {
        if w.open {
            // Bracket the slider repaint with the counting cursor semaphore
            // (EmuTOS `w_update` moff/mon) so a cursor over the scroll bar
            // leaves no dropping. See the note in [`compose`].
            crate::vdi::cursor_hide(backend);
            draw_vslider(w, backend, ClipRect::full(backend.width(), backend.height()));
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
    let b = w.curr;
    let kind = w.kind;
    if kind & HSLIDE == 0 {
        return None;
    }
    let mut left = b.x + 1;
    let mut right = b.x + b.w - 1;
    if kind & (VGADGETS | SIZER) != 0 {
        right -= GAD - 1;
    }
    if kind & LFARROW != 0 {
        left += GAD - 1;
    }
    if kind & RTARROW != 0 {
        right -= GAD - 1;
    }
    (right > left).then_some((left, right - left))
}

fn draw_hslider(w: &Window, backend: &dyn RasterBackend, clip: ClipRect) {
    let b = w.curr;
    let kind = w.kind;
    if kind & (HGADGETS | SIZER) == 0 {
        return;
    }
    let row_y = b.y + b.h - GAD;
    // Arrow positions: left just past the left border, right before the
    // V-bar/sizer column (same offsets `htrack` uses for the track between).
    let sx = b.x + 1;
    let mut send = b.x + b.w - 1;
    if kind & (VGADGETS | SIZER) != 0 {
        send -= GAD - 1;
    }
    if kind & LFARROW != 0 {
        gadget(Grect { x: sx, y: row_y, w: GAD, h: GAD }, G_LF, backend, clip);
    }
    if kind & RTARROW != 0 {
        gadget(Grect { x: send - GAD + 1, y: row_y, w: GAD, h: GAD }, G_RT, backend, clip);
    }
    if let Some((track_x, track_w)) = htrack(w) {
        backend.fill_rect(track_x, row_y, track_w, GAD, GRAY_LT, clip);
        rect_outline(Grect { x: track_x, y: row_y, w: track_w, h: GAD }, backend, clip);
        // Thumb geometry shared with `gadget_at`/`drag_elevator` (see
        // [`thumb_geom`]): a stored −1 size renders as one gadget cell.
        let (off, thumb_w) = thumb_geom(track_w, w.hslsize, w.hslide);
        let thumb_x = track_x + off;
        backend.fill_rect(thumb_x, row_y, thumb_w, GAD, GRAY_LT, clip);
        bevel(Grect { x: thumb_x, y: row_y, w: thumb_w, h: GAD }, true, backend, clip);
    }
}

/// Redraw just window `handle`'s horizontal slider (no work-area clear), for
/// `wind_set(WF_HSLIDE/WF_HSLSIZ)`.
fn redraw_hslider(handle: i16) {
    if let (Some(backend), Some(w)) = (host().and_then(|h| h.raster()), win(handle)) {
        if w.open {
            // Counting cursor bracket around the slider repaint — see
            // [`redraw_vslider`] / [`compose`].
            crate::vdi::cursor_hide(backend);
            draw_hslider(w, backend, ClipRect::full(backend.width(), backend.height()));
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
    for (i, w) in slots().iter_mut().enumerate() {
        if !w.in_use {
            *w = Window::EMPTY;
            w.in_use = true;
            w.kind = kind;
            w.full = full;
            w.curr = full;
            // Per-window gadget colours inherit the current global defaults
            // (EmuTOS `w_setup`, gemwmlib.c:222-226) so a prior WF_DCOLOR change
            // is reflected in windows created afterwards.
            w.tcolor = *gl_wtcolor();
            w.bcolor = *gl_wbcolor();
            pb.set_ret((i + 1) as i16);
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
        w.curr = r;
        w.open = true;
        w.owner = super::appl::current_pid();
        w.z = bump_z();
        // The new window is topmost and covers exactly its own rect `r`; nothing
        // outside `r` changes. Repaint only `r` (background + frames, this window
        // on top) rather than the whole screen — the open/close full-screen
        // flicker fix, mirroring the move/size damage-scope. This window's content
        // follows via `send_redraw` (WM_REDRAW).
        redraw_clip(r.x, r.y, r.w, r.h);
        send_redraw(handle, calc(WC_WORK, w.kind, r));
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
        match win(new_top).map(|w| w.curr) {
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
/// `wind_get` scalar-field return (WF_KIND / WF_TOP / the slider fields).
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
            WF_WXYWH => calc(WC_WORK, w.kind, w.curr),
            WF_CXYWH => w.curr,
            WF_PXYWH => w.prev,
            WF_FXYWH => w.full,
            WF_KIND => {
                pb.set_intout(1, w.kind as i16);
                pb.set_ret(1);
                return;
            }
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
            // Any other field on a valid window (COLOR/DCOLOR/OWNER/… — not
            // implemented at this altitude) → FALSE (EmuTOS default).
            _ => {
                pb.set_ret(0);
                return;
            }
        },
        // Desktop (handle 0): the WORK area sits *under* the menu bar (EmuTOS
        // gl_rfull); CURR/PREV/FULL are the whole screen (gl_rscreen).
        None if handle == 0 => match field {
            WF_WXYWH => desktop_work(),
            WF_CXYWH | WF_PXYWH | WF_FXYWH => screen_rect(),
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
    // WF_NEWDESK registers the desktop-background tree on the desktop
    // pseudo-window (`DESKWH` = 0), which has no window-table slot — handle it
    // before the `win()` guard (EmuTOS `wm_set` operates on the always-present
    // `D.w_win[0]` slot, `gemwmlib.c:1685`). Never fails.
    if field == WF_NEWDESK {
        set_newdesk(pb);
        pb.set_ret(1);
        return;
    }
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
            let (kind, prev) = if let Some(w) = win(handle) {
                w.prev = w.curr;
                w.curr = r;
                (w.kind, w.prev)
            } else {
                (0, r)
            };
            // Repaint only the union of the old and new border rects — the
            // programmatic move/size path (desktop WM_FULLED, app `wind_set`) —
            // not the whole screen. The frame composite (background + frames) is
            // damage-scoped here; this window's content follows via `send_redraw`
            // (WM_REDRAW). Lower windows' content is re-laid by the owner's
            // damage repaint when it drives this path (`desk::redraw`).
            let d = union_grect(prev, r);
            redraw_clip(d.x, d.y, d.w, d.h);
            send_redraw(handle, calc(WC_WORK, kind, r));
        }
        WF_TOP => {
            // EmuTOS `wm_set(WF_TOP)` tops the window only when it isn't already
            // on top (`if (w_handle != gl_wtop) wm_mktop(...)`,
            // `gemwmlib.c:1679-1683`), and the resulting `draw_change`→`w_update`
            // posts a `WM_REDRAW` to just the newly-topped window for its
            // now-fully-visible work area (`:1004-1048` walks bottom=top=w_handle;
            // `w_redraw` at `:887-907`). Topping only *exposes* the topped window,
            // so no other window's visible region grows — none else is redrawn.
            // The send lives here (not in [`top_window`], which the interactive
            // `intercept` path shares) so the click-to-top WM_TOPPED message it
            // writes to `pbuff` is not clobbered by a queued WM_REDRAW.
            let old_top = get_top();
            top_window(handle);
            if old_top != handle {
                // The old top loses activation → WM_UNTOPPED to its owner
                // (EmuTOS `perform_untop`, gemctrl.c:151), then WM_REDRAW to the
                // newly-topped window for its now-fully-visible work area.
                post_untopped(old_top, handle);
                if let Some((kind, curr)) = win(handle).map(|w| (w.kind, w.curr)) {
                    send_redraw(handle, calc(WC_WORK, kind, curr));
                }
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

/// Redraw just `handle`'s frame if it is open (for name/info changes).
fn redraw_open(handle: i16) {
    if let (Some(backend), Some(w)) = (host().and_then(|h| h.raster()), win(handle)) {
        if w.open {
            // Counting cursor bracket around the single-frame repaint (EmuTOS
            // `w_update` moff/mon) — see [`compose`].
            crate::vdi::cursor_hide(backend);
            draw_frame(w, handle == get_top(), backend, ClipRect::full(backend.width(), backend.height()));
            crate::vdi::cursor_show(backend);
        }
    }
}

/// `wind_find` (106) — handle of the window under `(int_in[0], int_in[1])`;
/// 0 = desktop.
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

/// `fm_own(begin)` — mouse-control-ownership transfer (EmuTOS gemfmlib.c:82-107).
/// `begin` brackets with `BEG_UPDATE` then `ml_ocnt++`; `!begin` does `ml_ocnt--`
/// then `END_UPDATE`. The `ml_ocnt == 0` edge body (park the menu tree, hand the
/// control region to the app via `ct_chgown`) is P4-011 and builds on this
/// counter; here we maintain the counter + the update-lock bracketing so
/// [`intercept`] can already honour the app's mouse takeover.
fn mctrl_own(begin: bool) {
    if begin {
        update_beg(); // fm_own: wm_update(BEG_UPDATE) (gemfmlib.c:86)
        MCTRL_CNT.fetch_add(1, Ordering::Relaxed);
    } else {
        // ml_ocnt-- then wm_update(END_UPDATE) (gemfmlib.c:99,105). EmuTOS
        // decrements unconditionally; the `intercept` gate (`> 0`) treats any
        // non-positive count as "app does not own the mouse", so an unbalanced
        // END_MCTRL cannot wedge interception off.
        MCTRL_CNT.fetch_sub(1, Ordering::Relaxed);
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
    for w in slots().iter_mut() {
        *w = Window::EMPTY;
    }
    // SAFETY: single-CPU AES.
    unsafe { *TABLE.next_z.get() = 1 };
    // Release all update / mouse-control locks (gemwmlib.c:1879-1883).
    MCTRL_CNT.store(0, Ordering::Relaxed);
    UPDATE_LOCK.store(0, Ordering::Relaxed);
    if let Some(backend) = host().and_then(|h| h.raster()) {
        redraw_all(backend);
    }
    pb.set_ret(1);
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

/// The desktop pseudo-window's work rectangle: the full screen minus the menu
/// bar at the top (EmuTOS `gl_rscreen` minus the menu-bar owner rect). When no
/// menu bar is installed it is the whole screen. Seeds [`compute_rlist`]'s
/// [`DESKWH`] rectangle list.
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
    let has_title = kind & (NAME | CLOSER | FULLER) != 0;

    // Title row.
    if has_title && my < b.y + GAD {
        if kind & CLOSER != 0 && mx < b.x + GAD {
            return Gadget::Closer;
        }
        if kind & FULLER != 0 && mx >= b.x + b.w - GAD {
            return Gadget::Fuller;
        }
        if kind & MOVER != 0 {
            return Gadget::Mover;
        }
        return Gadget::None;
    }
    // Sizer corner.
    if kind & SIZER != 0 && mx >= b.x + b.w - GAD && my >= b.y + b.h - GAD {
        return Gadget::Sizer;
    }
    // Right scroll-bar arrows.
    if kind & (VGADGETS | SIZER) != 0 && mx >= b.x + b.w - GAD {
        let mut top = b.y + 1;
        if has_title {
            top += GAD - 1;
        }
        if kind & INFO != 0 {
            top += GAD - 1;
        }
        let mut bot = b.y + b.h - 1;
        if kind & (HGADGETS | SIZER) != 0 {
            bot -= GAD - 1;
        }
        if kind & UPARROW != 0 && my < top + GAD {
            return Gadget::Up;
        }
        if kind & DNARROW != 0 && my >= bot - GAD + 1 {
            return Gadget::Dn;
        }
        // Elevator thumb (drag) vs. a page click in the track either side of
        // it (EmuTOS `hctl_window` W_VELEV vs W_VSLIDE split, gemctrl.c:262-306:
        // `my < elev_y` → up-page, else down-page).
        if let Some((track_y, track_h)) = vtrack(w) {
            if my >= track_y && my < track_y + track_h {
                let (off, len) = thumb_geom(track_h, w.vslsize, w.vslide);
                let thumb_y = track_y + off;
                if my >= thumb_y && my < thumb_y + len {
                    return Gadget::VElev;
                }
                return if my < thumb_y { Gadget::VUpPage } else { Gadget::VDnPage };
            }
        }
    }
    // Bottom scroll-bar arrows.
    if kind & (HGADGETS | SIZER) != 0 && my >= b.y + b.h - GAD {
        let left = b.x + 1;
        let mut right = b.x + b.w - 1;
        if kind & (VGADGETS | SIZER) != 0 {
            right -= GAD - 1;
        }
        if kind & LFARROW != 0 && mx < left + GAD {
            return Gadget::Lf;
        }
        if kind & RTARROW != 0 && mx >= right - GAD {
            return Gadget::Rt;
        }
        // Elevator thumb (drag) vs. a page click in the track either side of it
        // (EmuTOS W_HELEV vs W_HSLIDE split, `mx < elev_x` → left-page).
        if let Some((track_x, track_w)) = htrack(w) {
            if mx >= track_x && mx < track_x + track_w {
                let (off, len) = thumb_geom(track_w, w.hslsize, w.hslide);
                let thumb_x = track_x + off;
                if mx >= thumb_x && mx < thumb_x + len {
                    return Gadget::HElev;
                }
                return if mx < thumb_x { Gadget::HLfPage } else { Gadget::HRtPage };
            }
        }
    }
    Gadget::None
}

/// The screen rectangle of a title-row gadget (closer/fuller), for the
/// `watchbox` feedback.
fn gad_rect(w: &Window, g: Gadget) -> Grect {
    let b = w.curr;
    match g {
        Gadget::Closer => Grect { x: b.x, y: b.y, w: GAD, h: GAD },
        Gadget::Fuller => Grect { x: b.x + b.w - GAD, y: b.y, w: GAD, h: GAD },
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

/// Live-drag the whole frame following the mouse until release; returns
/// the final border rect (the frame is left drawn there). Stands in for
/// EmuTOS `gr_dragbox` (outline drag) at our altitude.
fn drag_move(w: &mut Window, backend: &dyn RasterBackend) -> Grect {
    let Some(h) = host() else { return w.curr };
    let full = ClipRect::full(backend.width(), backend.height());
    let (sx, sy) = crate::vdi::mouse_position();
    let orig = w.curr;
    let sw = backend.width() as i16;
    let sh = backend.height() as i16;
    let mut last = orig;
    loop {
        let (mx, my) = crate::vdi::mouse_position();
        // Bound the window's top to the desktop work area — below the menu bar
        // (`menu::desk_top`, 0 when none is installed). A drag must never push
        // the window over the menu bar, which the desktop paints once and never
        // restores (EmuTOS bounds window moves to `gl_rfull` = screen minus the
        // menu bar). The bottom/left/right still reach the screen edges.
        // `.max(..)` guards the clamp: a window wider/taller than the available
        // area would make `max < min` and panic `i16::clamp` (fatal in no_std).
        let top = super::menu::desk_top();
        let nx = (orig.x + mx - sx).clamp(0, (sw - orig.w).max(0));
        let ny = (orig.y + my - sy).clamp(top, (sh - orig.h).max(top));
        let nr = Grect { x: nx, y: ny, w: orig.w, h: orig.h };
        if nr != last {
            // Solid drag (modern HW — no rubber-band outline): blit the
            // rendered window (frame + content) to the new position via a
            // fast screen-to-screen copy, then repaint the vacated L-region
            // with the desktop background. The whole window follows the mouse
            // over the intact background — no white trail, no re-render. Hide
            // the cursor around the blit so its sprite isn't duplicated into
            // the moved region.
            crate::vdi::cursor_hide(backend);
            backend.copy_rect(
                Rect { x: last.x, y: last.y, w: last.w, h: last.h },
                nr.x,
                nr.y,
                3, // BitBlt replace
                full,
            );
            repaint_vacated(backend, last, nr);
            crate::vdi::cursor_show(backend);
            w.curr = nr;
            last = nr;
        }
        if crate::vdi::mouse_buttons() & 0x01 == 0 {
            break;
        }
        h.pump_input();
    }
    // Record the pre-move border rect as the window's previous rect so the owner
    // (the desktop) can compute the damage region (old ∪ new) for the
    // settle-overlap repaint on WM_MOVED. The interactive drag mutated `w.curr`
    // directly, bypassing the `wind_set(WF_CXYWH)` path that maintains `prev`.
    w.prev = orig;
    last
}

/// Repaint the part of `old` not covered by `new` (same size — a window
/// move) with the desktop background: the vacated L-region of a solid drag.
/// Up to two rectangles — a full-width horizontal strip for the vertical
/// motion, plus a vertical strip limited to the overlapping rows (so the
/// corner is not painted twice, and the still-covered part is never erased).
fn repaint_vacated(backend: &dyn RasterBackend, old: Grect, new: Grect) {
    let dx = new.x - old.x;
    let dy = new.y - old.y;
    if dy > 0 {
        paint_bg_clip(backend, rect_clip(old.x, old.y, old.w, dy));
    } else if dy < 0 {
        paint_bg_clip(backend, rect_clip(old.x, new.y + old.h, old.w, -dy));
    }
    let oy = old.y.max(new.y);
    let oh = (old.y + old.h).min(new.y + old.h) - oy;
    if oh > 0 {
        if dx > 0 {
            paint_bg_clip(backend, rect_clip(old.x, oy, dx, oh));
        } else if dx < 0 {
            paint_bg_clip(backend, rect_clip(new.x + old.w, oy, -dx, oh));
        }
    }
}

/// Live-resize the frame (bottom-right corner follows the mouse) until
/// release; returns the final border rect. Stands in for `gr_rubwind`.
fn drag_size(w: &mut Window, backend: &dyn RasterBackend) -> Grect {
    let Some(h) = host() else { return w.curr };
    let orig = w.curr;
    let min_w = 3 * GAD;
    let min_h = 3 * GAD;
    let sw = backend.width() as i16;
    let sh = backend.height() as i16;
    let mut last = orig;
    loop {
        let (mx, my) = crate::vdi::mouse_position();
        // `.max(min_*)` guards the clamp bounds (origin near the screen edge
        // could make `max < min`, panicking `i16::clamp` — fatal in no_std).
        let nw = (mx - orig.x + 1).clamp(min_w, (sw - orig.x).max(min_w));
        let nh = (my - orig.y + 1).clamp(min_h, (sh - orig.y).max(min_h));
        let nr = Grect { x: orig.x, y: orig.y, w: nw, h: nh };
        if nr != last {
            // Resize reflows the content, so (unlike a move) it cannot blit —
            // repaint the desktop background over the previous window rect,
            // then redraw the frame at the new size. No white trail; the work
            // area's content returns on release via the desktop's redraw.
            crate::vdi::cursor_hide(backend);
            paint_bg_clip(backend, rect_clip(orig.x, orig.y, last.w, last.h));
            w.curr = nr;
            draw_frame(w, true, backend, ClipRect::full(backend.width(), backend.height()));
            crate::vdi::cursor_show(backend);
            last = nr;
        }
        if crate::vdi::mouse_buttons() & 0x01 == 0 {
            break;
        }
        h.pump_input();
    }
    // Record the pre-resize border rect as the window's previous rect so the
    // owner (the desktop) can compute the damage region (old ∪ new) for the
    // settle-overlap repaint on WM_SIZED, exactly as a programmatic WF_CXYWH
    // set would. The interactive drag mutated `w.curr` directly, bypassing the
    // `wind_set(WF_CXYWH)` path that normally maintains `prev`.
    w.prev = orig;
    last
}

/// Live-drag a scroll-bar elevator (thumb) following the mouse until release;
/// returns the final position 0..1000, which the caller posts as `WM_VSLID` /
/// `WM_HSLID` `msg[4]`. Models EmuTOS `gr_slidebox` (`gemgrlib.c:358-410`): the
/// thumb is dragged within its track, held at a constant grip offset from the
/// cursor, and the reported position is `mul_div_round(travel_used, 1000,
/// travel)` (the D-098 slider-creep rounding). Feedback is **thumb-follow**
/// (the slider is repainted at the new spot each frame, bracketed with the
/// counting cursor semaphore), matching NewTOS' solid [`drag_move`] rather than
/// EmuTOS' XOR outline — the same modern-HW choice already made for the mover.
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
/// z among the windows whose border rect contains the point); 0 = desktop.
/// Mirrors EmuTOS `wm_find` = `ob_find(gl_wtree, 0, 2, x, y)`, which walks the
/// window tree strictly top-to-bottom (gemsuper.c:348-350, gemwmlib.c:1767).
fn find_handle(mx: i16, my: i16) -> i16 {
    let mut best = 0i16;
    let mut best_z = i16::MIN;
    for (i, w) in slots().iter().enumerate() {
        if w.in_use && w.open && inside(w.curr, mx, my) && w.z >= best_z {
            best_z = w.z;
            best = (i + 1) as i16;
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
    if handle == 0 {
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

    // Click on a non-top window → untop the old top, bring this one up, and tell
    // each owner. EmuTOS `hctl_window` (gemctrl.c:172-180): `perform_untop(gl_wtop)`
    // sends WM_UNTOPPED to the *old* top's owner, then WM_TOPPED to the new one.
    if handle != get_top() {
        let old_top = get_top();
        msg[0] = WM_TOPPED;
        wait_release();
        post_untopped(old_top, handle);
        top_window(handle);
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
    if old_top == new_top {
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
