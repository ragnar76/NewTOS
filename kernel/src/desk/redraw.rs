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
//! back-to-front, so *frames* and the *desktop* are occluded by draw order. The
//! desktop **application** owns the window **content** — the item `OBJECT`s —
//! and draws it here. [`do_wredraw`] clips that content to the window's AES
//! owner-rectangle list (`WF_FIRSTXYWH`/`WF_NEXTXYWH`: the work area minus every
//! higher window) intersected with the caller's damage rectangle, and calls
//! `objc_draw` once per surviving piece — so a covered window never paints over
//! the window on top of it, **regardless of draw order** (the D2 fix). The AES
//! rectangle list is used as-is (`tos_api::aes::window::compute_rlist`); this
//! module builds no rectangle list of its own.

use super::global::{self, Grect, DROOT};
use super::win::DESKWH;
use super::{
    graf_mouse, objc_draw_tree, paint_desktop_region, wind_get_rect, wind_get_work, M_OFF, M_ON,
    WF_FIRSTXYWH, WF_NEXTXYWH,
};

/// `do_wredraw(handle, clip)` — repaint whatever part of the object tree rooted
/// at window `handle` (or the desktop) is both (a) currently visible on screen
/// and (b) within the caller-supplied damage rectangle `clip`
/// (`desksupp.c:202-232`, `desksupp-redraw.md`).
///
/// Steps, exactly per §6:
/// 1. Resolve the object-tree root: [`DESKWH`] (0) → [`DROOT`]; otherwise the
///    window's `w_root`. A stale/closed handle (`win_find` → `None`) returns
///    immediately — no drawing, no cursor toggle (EmuTOS `desksupp.c:214-215`).
/// 2. `graf_mouse(M_OFF)` — hide the cursor for the paint.
/// 3. Walk the AES owner-rectangle list: `t = wind_get(handle, WF_FIRSTXYWH)`,
///    then `while t.w != 0 && t.h != 0 { if rc_intersect(clip, t) { objc_draw(…,
///    root, …, t) } t = wind_get(handle, WF_NEXTXYWH) }` — the sentinel is
///    `w == 0 || h == 0`; the loop always re-fetches the next piece, even after
///    a hit, so the entire list is drained once.
/// 4. `graf_mouse(M_ON)` — restore the cursor.
///
/// The [`DESKWH`] arm draws the desktop surface (background + icons over
/// `DROOT`) within `clip` via `super::paint_desktop_region` instead of walking
/// the owner-rectangle list: the desktop pseudo-window has no such list in
/// `tos-api` (nor does EmuTOS route the desktop through `do_wredraw`'s
/// `WM_REDRAW` path — handle 0 is skipped, the AES redraws the desktop). Its
/// under-window occlusion comes from the `redraw_all` compositing order.
pub fn do_wredraw(handle: i16, clip: Grect) {
    // 1. Resolve the object-tree root (silent no-op on a stale handle).
    let root = if handle == DESKWH {
        DROOT
    } else {
        match global::with(|g| g.win_find(handle).map(|w| w.w_root)).flatten() {
            Some(r) => r,
            None => return, // window closed between redraw queued and processed
        }
    };
    if clip.w <= 0 || clip.h <= 0 {
        return;
    }

    // 2. Hide the cursor around the draw.
    graf_mouse(M_OFF);

    if handle == DESKWH {
        // Desktop surface (root == DROOT): background pattern + desktop icons
        // (the DROOT subtree) within `clip`. Occlusion by the redraw_all
        // compositing order, not an owner-rectangle list (see the doc-comment).
        paint_desktop_region(clip);
    } else if let Some(tree) = global::screen_addr() {
        // 3. Window content: one clip-and-draw per visible owner rectangle.
        let mut t = wind_get_rect(handle, WF_FIRSTXYWH);
        while t.w != 0 && t.h != 0 {
            if let Some(piece) = rc_intersect(clip, t) {
                objc_draw_tree(tree, root, piece);
            }
            t = wind_get_rect(handle, WF_NEXTXYWH);
        }
    }

    // 4. Restore the cursor.
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
        do_wredraw(h, work);
    }
}

/// Full desktop composite: the desktop surface + every window frame (the AES
/// `redraw_all`, which runs the `super::paint_desktop_bg` painter hook), then
/// every open window's content occlusion-clipped ([`win_shwall`]). Boot's
/// initial paint and any "repaint everything" path. The menu bar is installed
/// once at boot and never covered (windows live below it), so it is not
/// reinstalled here.
///
/// The cursor is hidden for the frame paint (`redraw_all` does not manage it)
/// and restored at the end; [`do_wredraw`]'s own `graf_mouse` brackets nest
/// harmlessly inside.
pub fn full_redraw() {
    graf_mouse(M_OFF);
    // Desktop background + icons + every window frame, occluded by draw order.
    tos_api::aes::window::redraw();
    // Each open window's content, occlusion-clipped via `do_wredraw`.
    win_shwall();
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
