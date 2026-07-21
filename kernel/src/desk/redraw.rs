//! `desk::redraw` — the damage-rectangle redraw (`do_wredraw`) and the
//! per-window redraw fan-out (`win_shwall`): the desktop's 1:1 correspondence
//! with EmuTOS `desksupp.c` / `deskwin.c`.
//!
//! Reimplements EmuTOS `desksupp.c:202-232` `do_wredraw()` per
//! [`docs/api/desktop/desksupp-redraw.md`] and the `win_shwall` "one redraw per
//! open window" fan-out (`deskwin.c:805`). Every desktop repaint of *window
//! content* (and, on demand, the desktop surface) goes through [`do_wredraw`],
//! over the AES owner-rectangle list — exactly like EmuTOS.
//!
//! Window open/close/top/move/size no longer drive a desktop-side whole-screen
//! composite (the deleted `damage_redraw` / `full_redraw` shortcut). The AES
//! repaints the background + frames (its `redraw_clip`) and — for open/move/size
//! — `send_redraw`s a `WM_REDRAW` for the affected window's content, which the
//! event loop drains back into [`do_wredraw`]. This is the app-facing redraw
//! path a real GEM application exercises; driving it (rather than repainting the
//! whole desktop ourselves) is the point of the Module-1 rebuild.
//!
//! The only surviving whole-desktop composite, [`redraw_desktop`], is used for
//! genuine whole-screen recovery — boot, a full-screen app's `Pterm`, or a
//! self-drawn modal dialog / file operation (Module 2/4) that clobbered the
//! screen. It is **not** a per-event shortcut and is never called from the
//! window-manager message handlers.
//!
//! ## How occlusion is realised in NewTOS
//!
//! The AES window manager in `tos-api` owns the desktop-background painter hook
//! and the window **frames**; its `redraw`/`redraw_clip` composites the desktop
//! surface (background + icons, via the painter hook) and every window frame
//! back-to-front, so *frames* and the *desktop* are occluded by draw order. The
//! desktop **application** owns the window **content** — the item `OBJECT`s — and
//! draws it here. [`do_wredraw`] clips whatever it draws to the target's AES
//! owner-rectangle list (`WF_FIRSTXYWH`/`WF_NEXTXYWH`: the work/desk area minus
//! every higher window) intersected with the caller's damage rectangle, and
//! draws once per surviving piece — so a covered window never paints over the
//! window on top of it, and the desktop background never paints over a window,
//! **regardless of draw order**. The desktop pseudo-window (handle 0 / `DESKWH`)
//! is the bottom of the z-order, so its rectangle list is the desk area minus
//! every open window. The AES rectangle list is used as-is
//! (`tos_api::aes::window::compute_rlist`); this module builds no list of its own.

use super::global::{self, Grect, DROOT};
use super::win::DESKWH;
use super::{
    graf_mouse, objc_draw_tree, wind_get_rect, wind_get_work, M_OFF, M_ON,
    WF_FIRSTXYWH, WF_NEXTXYWH,
};

/// Resolve the object-tree root for the redraw and apply the stale-handle guard.
/// Returns `None` — "nothing to paint" — when the handle no longer maps to a
/// live window (closed between the redraw being queued and processed, EmuTOS
/// `desksupp.c:214-215`). [`DESKWH`] (0) always resolves to [`DROOT`]; any other
/// handle to its `w_root`. Pure lookup: no drawing, no cursor toggle — so
/// [`do_wredraw`] can consult it *before* touching the cursor and thus reproduce
/// EmuTOS's "a stale redraw toggles nothing" exactly.
fn wredraw_root(handle: i16) -> Option<i16> {
    if handle == DESKWH {
        return Some(DROOT);
    }
    global::with(|g| g.win_find(handle).map(|w| w.w_root)).flatten()
}

/// `do_wredraw_nocursor(handle, clip)` — the **pure clip-and-draw** half of
/// [`do_wredraw`]: every step *except* the `graf_mouse(M_OFF/M_ON)` bracket. The
/// caller owns the cursor, which lets a multi-window fan-out ([`win_shwall`])
/// hide it **once** around the whole walk instead of blinking it back on between
/// windows — AES `graf_mouse` is not ref-counted, so a nested `M_ON` shows the
/// cursor unconditionally mid-paint.
///
/// Steps 1 + 3 of [`do_wredraw`]'s contract: resolve the root via
/// [`wredraw_root`] (a stale handle is a silent no-op), then walk the AES
/// owner-rectangle list — `t = wind_get(handle, WF_FIRSTXYWH)`, then
/// `while t.w != 0 && t.h != 0 { if rc_intersect(clip, t) { objc_draw(…, root,
/// …, t) } t = wind_get(handle, WF_NEXTXYWH) }` — draining the entire list once
/// (EmuTOS `desksupp.c:223-229`).
fn do_wredraw_nocursor(handle: i16, clip: Grect) {
    let Some(root) = wredraw_root(handle) else {
        return;
    };
    let Some(tree) = global::screen_addr() else {
        return;
    };
    let mut t = wind_get_rect(handle, WF_FIRSTXYWH);
    while t.w != 0 && t.h != 0 {
        if let Some(piece) = rc_intersect(clip, t) {
            objc_draw_tree(tree, root, piece);
        }
        t = wind_get_rect(handle, WF_NEXTXYWH);
    }
}

/// `do_wredraw(handle, clip)` — repaint whatever part of the object tree rooted
/// at window `handle` (or the desktop) is both (a) currently visible on screen
/// and (b) within the caller-supplied damage rectangle `clip` (EmuTOS
/// `desksupp.c:202-232`, `desksupp-redraw.md`). The `WM_REDRAW` message handler's
/// terminal step (and the direct caller in `win_blt`/`refresh_window`/
/// `app_blddesk` — EmuTOS `desksupp.c:216`/`deskwin.c:635`/`deskapp.c:1403`).
///
/// Steps, exactly per EmuTOS:
/// 1. Resolve the object-tree root: [`DESKWH`] (0) → [`DROOT`]; otherwise the
///    window's `w_root`. A stale/closed handle (`win_find` → `None`) returns
///    immediately — no drawing, **no cursor toggle** (`desksupp.c:214-215`); the
///    [`wredraw_root`] pre-check preserves that.
/// 2. `graf_mouse(M_OFF)` — hide the cursor for the paint (`desksupp.c:221`).
/// 3. Walk the owner-rectangle list, `objc_draw` per non-empty
///    `rc_intersect(clip, piece)` (`desksupp.c:223-229`).
/// 4. `graf_mouse(M_ON)` — restore the cursor (`desksupp.c:231`).
///
/// The [`DESKWH`] arm draws the desktop surface (the `DROOT` background box + its
/// icon children) via `objc_draw`, exactly like the window arm draws a window's
/// item tree — only on the pieces the AES owner-rectangle list yields for the
/// desktop pseudo-window (the desk area minus every open window). Direct callers
/// use this to repaint the desktop surface (`do_wredraw(DESKWH, &G.g_desk)`, as
/// EmuTOS `app_blddesk` does); the **`WM_REDRAW` message** arm, by contrast,
/// skips `handle == DESKWH` (the AES redraws the desktop surface itself via
/// `WF_NEWDESK`).
pub fn do_wredraw(handle: i16, clip: Grect) {
    // Preserve EmuTOS's "a stale redraw toggles nothing" (desksupp.c:214-215):
    // resolve first, and only bracket the cursor around a real paint.
    if wredraw_root(handle).is_none() {
        return;
    }
    graf_mouse(M_OFF);
    do_wredraw_nocursor(handle, clip);
    graf_mouse(M_ON);
}

/// `win_shwall` (EmuTOS `deskwin.c:805`) — repaint every open window's content,
/// each occlusion-clipped, by routing it through [`do_wredraw`] with the
/// window's current work-area rectangle. EmuTOS queues one `WM_REDRAW` per window
/// (`fun_msg(WM_REDRAW, wh, …)`) and drains them in the event loop; NewTOS calls
/// `do_wredraw` directly (the `tos-api` message pipe holds a single pending
/// message, not a queue), for the same observable result. The desktop
/// pseudo-window is **not** included — its surface is repainted by the AES
/// `WF_NEWDESK` painter hook, exactly as EmuTOS's `win_shwall` skips `DESKWH`.
///
/// Drives the per-window repaint for the View/Sort rebuild (`desk_all`) and the
/// selection-clear fan-out (`desk_clear_all`): those change every window's
/// content, so each open window's items must be re-laid.
pub fn win_shwall() {
    // One outer cursor bracket around the whole fan-out — the per-window paints
    // go through `do_wredraw_nocursor`, so the cursor is never blinked back on
    // between windows.
    graf_mouse(M_OFF);
    win_shwall_nocursor();
    graf_mouse(M_ON);
}

/// The cursor-unbracketed body of [`win_shwall`]: snapshot the open handles and
/// paint each window's content occlusion-clipped via [`do_wredraw_nocursor`].
/// Split out so [`redraw_desktop`] can fold the fan-out into its own single outer
/// `graf_mouse` bracket.
fn win_shwall_nocursor() {
    // Snapshot the open handles under the lock; draw with the lock released.
    let mut handles = [0i16; global::NUM_WNODES];
    let n = snapshot_window_handles(&mut handles);

    // Order is irrelevant to correctness — each piece is occlusion-clipped by the
    // owner-rectangle list, so nothing a higher window covers is drawn.
    for &h in &handles[..n] {
        let work = wind_get_work(h);
        do_wredraw_nocursor(h, work);
    }
}

/// Snapshot the open (non-desktop) window handles in z-order into `out`,
/// returning the count. Taken under the global lock so the drawing that follows
/// runs with the lock released (each `do_wredraw_nocursor` re-acquires it for the
/// owner-rectangle-list walk).
fn snapshot_window_handles(out: &mut [i16; global::NUM_WNODES]) -> usize {
    global::with(|g| {
        let mut n = 0;
        for &slot in g.windows.zorder.iter() {
            let wn = &g.windows.slots[slot];
            if wn.w_id != DESKWH && n < out.len() {
                out[n] = wn.w_id;
                n += 1;
            }
        }
        n
    })
    .unwrap_or(0)
}

/// Repaint the whole desktop for genuine whole-screen recovery — boot, a
/// full-screen app's `Pterm` (app-return), or a self-drawn modal dialog / file
/// operation (Module 2/4) that covered the screen and did not save/restore under
/// itself. **Not** a per-event shortcut: window open/close/top/move/size and menu
/// picks route their repaint through the per-window [`do_wredraw`] / AES
/// `send_redraw` path, exactly like EmuTOS — this function is never called from a
/// window-manager message handler.
///
/// Composites the AES background + every window frame (`window::redraw`, the AES
/// compositor — restoring frames a self-drawn dialog / launched app clobbered),
/// re-lays every window's content occlusion-clipped via [`do_wredraw`]
/// ([`win_shwall_nocursor`]), and repaints the menu-bar strip a full-screen
/// program may have overwritten. The cursor is hidden once for the whole
/// composite (`window::redraw` does not manage it) and the window-content fan-out
/// runs cursor-unbracketed so no inner `graf_mouse(M_ON)` flashes the cursor over
/// a half-drawn desktop.
///
/// The Module-2 self-drawn dialogs and Module-4 file ops that still call this
/// should, when those modules are rebuilt, move their restore onto the AES
/// `form_dial(FMD_START/FMD_FINISH)` save/restore + `WM_REDRAW` path so they no
/// longer need a whole-desktop composite either — tracked as a follow-up.
pub fn redraw_desktop() {
    graf_mouse(M_OFF);
    // AES background + every window frame, occluded by draw order.
    tos_api::aes::window::redraw();
    // Each open window's content, occlusion-clipped — no inner cursor toggle.
    win_shwall_nocursor();
    // The menu bar (top strip), which a full-screen program may have clobbered.
    super::menu::install_menu_bar();
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
