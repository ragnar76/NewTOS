//! `desk::redraw` — the damage-rectangle redraw (`do_wredraw`) and the
//! redraw-routing primitives that supersede the #6 full-screen-clip path.
//!
//! Reimplements EmuTOS `desksupp.c:202-232` `do_wredraw()` per
//! [`docs/api/desktop/desksupp-redraw.md`] and `_rebuild-plan.md` §6, the
//! `win_shwall`-style "one redraw per affected window" fan-out
//! (`deskwin.c:805`), and the full-desktop composite that boot and every window
//! operation drive. Fixes **D2** (occlusion-aware window-content redraw) and
//! **D8** (`WM_REDRAW` uses the message's own rectangle).
//!
//! ## How occlusion is realised in NewTOS
//!
//! The AES window manager in `tos-api` owns the desktop-background painter hook
//! and the window **frames**; its `redraw_all` composites the desktop surface
//! (background + icons, via the painter hook) and every window frame
//! back-to-front, so *frames* and the *desktop* are occluded by draw order in
//! the full composite ([`full_redraw`]). The desktop **application** owns the
//! window **content** — the item `OBJECT`s — and the desktop **surface**, and
//! draws both here. [`do_wredraw`] clips whatever it draws to the target's AES
//! owner-rectangle list (`WF_FIRSTXYWH`/`WF_NEXTXYWH`: the work/desk area minus
//! every higher window) intersected with the caller's damage rectangle, and
//! draws once per surviving piece — so a covered window never paints over the
//! window on top of it, and the desktop background never paints over a window,
//! **regardless of draw order** (the D2 / Task-#18 fix). The desktop
//! pseudo-window (handle 0 / `DESKWH`) is the bottom of the z-order, so its
//! rectangle list is the desk area minus every open window. The AES rectangle
//! list is used as-is (`tos_api::aes::window::compute_rlist`); this module
//! builds no rectangle list of its own.

use super::global::{self, Grect, DROOT};
use super::win::DESKWH;
use super::{
    graf_mouse, objc_draw_tree, paint_desktop_region, wind_get_rect, wind_get_work, M_OFF, M_ON,
    WF_FIRSTXYWH, WF_NEXTXYWH,
};

/// Resolve the object-tree root for the redraw and apply the empty-clip guard.
/// Returns `None` — "nothing to paint" — when the clip is degenerate **or** the
/// handle no longer maps to a live window (closed between the redraw being
/// queued and processed, EmuTOS `desksupp.c:214-215`). [`DESKWH`] (0) always
/// resolves to [`DROOT`]; any other handle to its `w_root`. Pure lookup: no
/// drawing, no cursor toggle — so [`do_wredraw`] can consult it *before*
/// touching the cursor and thus reproduce EmuTOS's "a stale/empty redraw toggles
/// nothing" exactly.
fn wredraw_root(handle: i16, clip: Grect) -> Option<i16> {
    if clip.w <= 0 || clip.h <= 0 {
        return None;
    }
    if handle == DESKWH {
        return Some(DROOT);
    }
    global::with(|g| g.win_find(handle).map(|w| w.w_root)).flatten()
}

/// `do_wredraw_nocursor(handle, clip)` — the **pure clip-and-draw** half of
/// [`do_wredraw`]: every step *except* the `graf_mouse(M_OFF/M_ON)` bracket. The
/// caller owns the cursor, which lets a multi-window composite hide it **once**
/// around the whole fan-out instead of blinking it back on between windows —
/// AES `graf_mouse` is not ref-counted, so a nested `M_ON` shows the cursor
/// unconditionally mid-paint (the **F3** flicker). [`win_shwall`] and
/// [`full_redraw`] therefore call *this* inside their single outer bracket,
/// while the public [`do_wredraw`] (the `WM_REDRAW` path) keeps its own.
///
/// Steps 1 + 3 of [`do_wredraw`]'s contract: resolve the root via
/// [`wredraw_root`] (a stale handle or empty clip is a silent no-op), then walk
/// the AES owner-rectangle list — `t = wind_get(handle, WF_FIRSTXYWH)`, then
/// `while t.w != 0 && t.h != 0 { if rc_intersect(clip, t) { objc_draw(…, root,
/// …, t) } t = wind_get(handle, WF_NEXTXYWH) }` — draining the entire list once.
fn do_wredraw_nocursor(handle: i16, clip: Grect) {
    let Some(root) = wredraw_root(handle, clip) else {
        return;
    };
    // Uniform owner-rectangle-list walk for BOTH the desktop pseudo-window and
    // real windows (EmuTOS `do_wredraw`, `desksupp.c:223-229`): `wind_get(…,
    // WF_FIRSTXYWH/NEXTXYWH)` yields each visible piece (the work/desk area
    // minus every higher window), intersected here with the caller's damage
    // rectangle `clip`. The desktop is the *bottom* of the z-order, so its list
    // excludes every open window — the background is drawn only on the exposed
    // desk area and never paints over a window (the Task-#18 Z-order fix). Real
    // windows draw their item subtree via `objc_draw`; the desktop draws its
    // background pattern + icons via `paint_desktop_region`.
    let tree = if handle == DESKWH { None } else { global::screen_addr() };
    let mut t = wind_get_rect(handle, WF_FIRSTXYWH);
    while t.w != 0 && t.h != 0 {
        if let Some(piece) = rc_intersect(clip, t) {
            if handle == DESKWH {
                paint_desktop_region(piece);
            } else if let Some(tree) = tree {
                objc_draw_tree(tree, root, piece);
            }
        }
        t = wind_get_rect(handle, WF_NEXTXYWH);
    }
}

/// `do_wredraw(handle, clip)` — repaint whatever part of the object tree rooted
/// at window `handle` (or the desktop) is both (a) currently visible on screen
/// and (b) within the caller-supplied damage rectangle `clip`
/// (`desksupp.c:202-232`, `desksupp-redraw.md`). The public `WM_REDRAW` entry
/// point; the actual drawing lives in [`do_wredraw_nocursor`].
///
/// Steps, exactly per §6:
/// 1. Resolve the object-tree root: [`DESKWH`] (0) → [`DROOT`]; otherwise the
///    window's `w_root`. A stale/closed handle (`win_find` → `None`) or an empty
///    clip returns immediately — no drawing, **no cursor toggle** (EmuTOS
///    `desksupp.c:214-215`); the [`wredraw_root`] pre-check preserves that.
/// 2. `graf_mouse(M_OFF)` — hide the cursor for the paint.
/// 3. Draw via [`do_wredraw_nocursor`] (the owner-rectangle-list walk).
/// 4. `graf_mouse(M_ON)` — restore the cursor.
///
/// The [`DESKWH`] arm draws the desktop surface (background pattern + icons over
/// `DROOT`) via `super::paint_desktop_region`, but — like the window arm — only
/// on the pieces the AES owner-rectangle list yields for the desktop
/// pseudo-window: the desk area (screen minus the menu bar) minus every open
/// window (the desktop is the bottom of the z-order). Walking that list is what
/// keeps a standalone desktop redraw from painting over the windows above it
/// (the Task-#18 Z-order fix); it mirrors EmuTOS routing the desktop through the
/// very same `wind_get_grect(DESKWH, WF_FIRSTXYWH/NEXTXYWH)` loop as a window.
pub fn do_wredraw(handle: i16, clip: Grect) {
    // Preserve EmuTOS's "a stale/empty redraw toggles nothing" (desksupp.c:214-215):
    // resolve first, and only bracket the cursor around a real paint.
    if wredraw_root(handle, clip).is_none() {
        return;
    }
    graf_mouse(M_OFF);
    do_wredraw_nocursor(handle, clip);
    graf_mouse(M_ON);
}

/// `win_shwall` (EmuTOS `deskwin.c:805`) — repaint every open window's content,
/// each occlusion-clipped, by routing it through [`do_wredraw`] with the
/// window's current work-area rectangle. EmuTOS queues one `WM_REDRAW` per
/// window and drains them in the event loop; NewTOS calls `do_wredraw` directly
/// (the `tos-api` message pipe holds a single pending message, not a queue), for
/// the same observable result. The desktop pseudo-window is **not** included —
/// its surface is repainted by the AES `redraw_all` painter hook, exactly as
/// EmuTOS's `win_shwall` skips `DESKWH`.
///
/// Drives the post-op redraw for window open/close/move/size/top and the
/// view/sort rebuild (`win_bldview` → redraw): those operations run `redraw_all`
/// (which erases every window's work area to draw the frames), so all open
/// windows' content must be re-laid on top — this is that fan-out.
pub fn win_shwall() {
    // One outer cursor bracket around the whole fan-out — the per-window paints
    // go through `do_wredraw_nocursor`, so the cursor is never blinked back on
    // between windows (the F3 flicker fix).
    graf_mouse(M_OFF);
    win_shwall_nocursor();
    graf_mouse(M_ON);
}

/// The cursor-unbracketed body of [`win_shwall`]: snapshot the open handles and
/// paint each window's content occlusion-clipped via [`do_wredraw_nocursor`].
/// Split out so [`full_redraw`] can fold the fan-out into its *own* single outer
/// `graf_mouse` bracket — without the mid-fan-out `M_ON` that (as `graf_mouse`
/// is not ref-counted) would flash the cursor over a half-drawn desktop (F3).
fn win_shwall_nocursor() {
    // Snapshot the open handles under the lock; draw with the lock released.
    let mut handles = [0i16; global::NUM_WNODES];
    let n = global::with(|g| {
        let mut n = 0;
        for &slot in g.windows.zorder.iter() {
            let wn = &g.windows.slots[slot];
            if wn.w_id != DESKWH && n < handles.len() {
                handles[n] = wn.w_id;
                n += 1;
            }
        }
        n
    })
    .unwrap_or(0);

    // Order is irrelevant to correctness — each piece is occlusion-clipped by
    // the owner-rectangle list, so nothing a higher window covers is drawn.
    for &h in &handles[..n] {
        let work = wind_get_work(h);
        do_wredraw_nocursor(h, work);
    }
}

/// Full desktop composite: the desktop surface + every window frame (the AES
/// `redraw_all`, which runs the `super::paint_desktop_bg` painter hook), then
/// every open window's content occlusion-clipped ([`win_shwall`]). Boot's
/// initial paint and any "repaint everything" path. The menu bar is installed
/// once at boot and never covered (windows live below it), so it is not
/// reinstalled here.
///
/// The cursor is hidden **once** for the whole composite (`redraw_all` does not
/// manage it) and restored at the end; the window-content fan-out runs through
/// [`win_shwall_nocursor`] so no inner `graf_mouse(M_ON)` flashes the cursor
/// over a half-drawn desktop between windows (the F3 flicker fix).
pub fn full_redraw() {
    graf_mouse(M_OFF);
    // Desktop background + icons + every window frame, occluded by draw order.
    tos_api::aes::window::redraw();
    // Each open window's content, occlusion-clipped — no inner cursor toggle.
    win_shwall_nocursor();
    graf_mouse(M_ON);
}

/// `rc_intersect` (EmuTOS `aes/gemrect.c`) — the overlap of GRECTs `a` and `b`,
/// or `None` when they are disjoint. Pure rectangle math (desktop-internal, not
/// an AES call); the width/height are exclusive-bound differences, so a shared
/// edge alone (`x1 == x0`) yields no rectangle.
fn rc_intersect(a: Grect, b: Grect) -> Option<Grect> {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.w).min(b.x + b.w);
    let y1 = (a.y + a.h).min(b.y + b.h);
    if x1 > x0 && y1 > y0 {
        Some(Grect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 })
    } else {
        None
    }
}
