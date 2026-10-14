//! `desk::main` — the desktop event loop and interaction dispatch (EmuTOS
//! `deskmain.c` main loop + handlers), per `docs/api/desktop/deskmain.md`.
//!
//! Reimplements the `evnt_multi` outer loop + message-drain sub-loop, the mouse
//! button handler (`hndl_button` — resolves the click through `wind_find`, the
//! **D3** fix), the keyboard handler (`hndl_kbd` — CR/ESC/arrows/function keys/
//! Alt-letter/Ctrl-shortcut, the **D4** fix), the menu dispatch (`hndl_menu` →
//! the four `do_*menu`), the `desk_all` view/sort pipeline, and the AES window
//! messages (`WM_TOPPED`/`WM_MOVED`/`WM_SIZED`/`WM_FULLED`/`WM_ARROWED`/
//! `WM_*SLID`/`WM_CLOSED`). `men_update` (the **D6** fix) lives in
//! [`super::menu`] and runs after every dispatched event except a bare redraw.

use alloc::string::String;
use alloc::vec::Vec;

use super::dosbind::{dos_gdrv, dos_sdrv};
use super::global::{self, Point, DROOT, MAX_DEPTH, NUM_WNODES, WINDOW_STYLE};
use tos_types::aes::{
    Grect, DESKWH, G_ICON, MN_SELECTED, MU_BUTTON, MU_KEYBD, MU_MESAG, MU_TIMER, M_OFF, M_ON, NIL,
    SELECTED, WA_DNLINE, WA_DNPAGE, WA_LFLINE, WA_RTLINE, WA_UPLINE, WA_UPPAGE, WF_CXYWH, WF_FXYWH,
    WF_HSLIDE, WF_HSLSIZ, WF_INFO, WF_NAME, WF_PXYWH, WF_TOP, WF_VSLIDE, WF_VSLSIZ, WM_ARROWED,
    WM_CLOSED, WM_FULLED, WM_HSLID, WM_MOVED, WM_REDRAW, WM_SIZED, WM_TOPPED, WM_VSLID,
};
use super::menu::{self, Item, Title};
use super::model::{AType, View};
use super::win::SliderSet;
use super::{dir, ftype, pro, redraw};
use super::{wind_get_grect, wind_get_top, wind_get_work, FLATHAND};

// AES message ids (`aesdefs.h:227-261`).

// `Kbshift()` modifier bits (`include/biosdefs.h:90-94`), as `graf_mkstate`
// reports them in `intout[4]`.
const MODE_RSHIFT: u16 = 0x01;
const MODE_LSHIFT: u16 = 0x02;
const MODE_CTRL: u16 = 0x04;
const MODE_ALT: u16 = 0x08;
/// `MODE_SCA` (`biosdefs.h:94`) — the four "shift/control/alternate" bits, the
/// mask `do_fopen` tests against so that Alt **alone** (and not Alt+Shift or
/// Alt+Ctrl) opens a folder in a new window (`desksupp.c:1132-1134`).
const MODE_SCA: u16 = MODE_RSHIFT | MODE_LSHIFT | MODE_CTRL | MODE_ALT;

// `do_viewmenu` change bits (EmuTOS `deskmain.c:93-96`), fed to `desk_all`.
const VIEW_CHANGED: u16 = 0x0001;
const SORT_CHANGED: u16 = 0x0002;
const SIZETOFIT_CHANGED: u16 = 0x0004;

/// `static BOOL restart_desktop;` (`deskmain.c:359`) — *"TRUE iff we need to
/// restart the desktop"*. Set by [`do_read_inf`] (`:921`), cleared at the top
/// of every `deskmain` (`:1946`), read by its epilog (`:2136`) to pick between
/// "free everything and come straight back" and "terminate".
///
/// A plain `AtomicBool` rather than one of the desktop's `SpinMutex` cells: it
/// is a single flag with no invariant tying it to the rest of `G`, and the
/// epilog reads it while the desktop lock is free.
static RESTART_DESKTOP: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// `restart_desktop = FALSE;` (`deskmain.c:1946`) — the first statement of
/// `deskmain`.
pub(super) fn clear_restart_desktop() {
    RESTART_DESKTOP.store(false, core::sync::atomic::Ordering::Relaxed);
}

/// `if (restart_desktop)` (`deskmain.c:2136`) — the epilog's question.
pub(super) fn restart_desktop() -> bool {
    RESTART_DESKTOP.load(core::sync::atomic::Ordering::Relaxed)
}

// ── Event loop ─────────────────────────────────────────────────────────────

/// The desktop main loop (EmuTOS `deskmain.c:2077-2123`): block in `evnt_multi`
/// for keyboard / button / message events, dispatch whichever fired, then drain
/// any further queued messages with a message-only `evnt_multi` before blocking
/// again. Runs until a handler requests exit (currently only a Task-#9 shutdown
/// path would; the desktop otherwise runs forever, which is correct).
pub fn event_loop() {
    let mut msg = [0i16; 8];
    let mut done = false;
    while !done {
        let ev = super::evnt_multi(MU_KEYBD | MU_BUTTON | MU_MESAG, &mut msg);

        if ev.which & MU_KEYBD != 0 && hndl_kbd(ev.kret) {
            done = true;
        }
        if ev.which & MU_BUTTON != 0 && hndl_button(ev.bret, ev.mx, ev.my) {
            done = true;
        }

        // Message-drain sub-loop: process this message, then keep pulling
        // queued ones (a single user action can queue several) until the pipe
        // is empty — the `MU_TIMER` "quick-out" returns as soon as none is
        // pending (EmuTOS `deskmain.c:2107-2118`).
        let mut which = ev.which;
        while which & MU_MESAG != 0 {
            if done {
                break;
            }
            if hndl_msg(&msg) {
                done = true;
            }
            let ev2 = super::evnt_multi(MU_MESAG | MU_TIMER, &mut msg);
            which = ev2.which;
        }
    }
}

/// The AES message-pipe handler (EmuTOS `hndl_msg`, `deskmain.c:1250-1361`).
/// Dispatches on the message type (`msg[0]`) with `msg[3]` the window handle
/// (or, for `MN_SELECTED`, the menu title). Returns `true` to end the loop.
/// Every message except `WM_REDRAW` re-runs `men_update`.
///
/// `pub(super)` because EmuTOS' `end_dialog` drains the redraw messages a
/// closing dialog queued through this very function (`deskinf.c:379-388`).
pub(super) fn hndl_msg(msg: &[i16; 8]) -> bool {
    let handle = msg[3];
    let mut done = false;
    let mut change = false;
    match msg[0] {
        MN_SELECTED => {
            // Re-sync the current window to whichever last held a selection,
            // then dispatch the (title, item) exactly like a mouse menu click.
            desk_verify_last();
            if let Some(item) = menu::item_of_obj(msg[4]) {
                done = hndl_menu(item, msg[3]);
            }
        }
        WM_REDRAW => {
            // EmuTOS `deskmain.c:1275-1281`: `menu = TRUE` (suppress the trailing
            // `men_update`); `if (handle) do_wredraw(handle, &G.g_rmsg[4])`.
            // `handle == 0` (`DESKWH`) is **skipped** — the desktop surface is the
            // AES's own `WF_NEWDESK` redraw, not the app's `do_wredraw` job. Paint
            // exactly the AES-supplied damage rectangle (`msg[4..8]`).
            if handle != DESKWH {
                let clip = Grect { x: msg[4], y: msg[5], w: msg[6], h: msg[7] };
                redraw::do_wredraw(handle, clip);
            }
            return done; // WM_REDRAW suppresses the trailing men_update
        }
        WM_TOPPED => change = top_msg(handle),
        WM_CLOSED => do_close_folder(), // == File → Close (EmuTOS CLOSITEM)
        WM_FULLED => change = full_msg(handle),
        WM_ARROWED => arrow_msg(handle, msg[4]),
        WM_VSLID => slid_msg(handle, false, msg[4]),
        WM_HSLID => slid_msg(handle, true, msg[4]),
        WM_MOVED => change = moved_msg(handle, msg),
        WM_SIZED => change = sized_msg(handle, msg),
        _ => {}
    }
    // `if (change) cnx_put();` (`deskmain.c:1352-1353`) — a window that was
    // topped, fulled, moved or sized has a new saved context. This is the only
    // externally visible effect the window messages have beyond the screen.
    if change {
        cnx_put();
    }
    menu::men_update();
    done
}

// ── Mouse button (D3) ──────────────────────────────────────────────────────

/// `hndl_button` (EmuTOS `deskmain.c:947`) — the raw mouse-button handler.
///
/// Resolves the click through `wind_find` (**D3**: acts on whichever window it
/// fell in), clears the *previous* window's selection when the click landed in
/// a different window or the desktop (**F1**, `deskmain.c:957-958`), makes the
/// clicked window current (`desk_verify`), then:
///
/// * single click → `act_bsclick` (the four selection rules); if the button is
///   still held afterwards, `act_bdown` (rubber-band **or** drag of the
///   selection) → `fun_drag` on a cross-window/icon drop;
/// * double click → select-only then open (folder → new window, drive → root).
fn hndl_button(clicks: i16, mx: i16, my: i16) -> bool {
    let handle = super::wind_find(mx, my);

    // F1 (deskmain.c:957-958): a click in a *different* window — or on the
    // desktop while a window was current — clears the old window's selection.
    let cwin = global::with(|g| g.g_cwin).unwrap_or(DESKWH);
    if handle != cwin {
        desk_clear_all();
    }

    let work = if handle == DESKWH { g_desk() } else { wind_get_work(handle) };
    let root = global::with(|g| {
        g.desk_verify(handle, work, false);
        g.g_croot
    })
    .unwrap_or(DROOT);

    let mut done = false;
    if clicks == 1 {
        let obid = gr_obfind(root, mx, my);
        let keystate = super::graf_mkstate().kstate;
        let shifted = keystate & 0x03 != 0;
        // act_bsclick: apply the selection rules and repaint if changed.
        if global::with(|g| g.act_bsclick(handle, root, obid, shifted, false)).unwrap_or(false) {
            redraw_window(handle);
        }
        // If the button is still down, this is a rubber-band or a drag.
        let st = super::graf_mkstate();
        if st.buttons & 0x01 != 0 {
            done = act_bdown(handle, root, mx, my);
        }
        // `pw = win_find(wh); win_sinfo(pw, TRUE);` (`deskmain.c:982-983`) —
        // the click changed the selection (or cleared it), so the info line
        // switches between the selection count and the directory totals.
        win_sinfo(handle, true);
    } else {
        // `act_bsclick(…, TRUE); done = do_filemenu(OPENITEM);`
        // (`deskmain.c:987-988`) — select just the hit item, then open it. The
        // flag matters since DESK43 U3: opening an application is a
        // `shel_write(SHW_EXEC, …)`, and `done` is how the desktop leaves so
        // the AES shell can start it.
        let obid = gr_obfind(root, mx, my);
        global::with(|g| g.act_bsclick(handle, root, obid, false, true));
        redraw_window(handle);
        done = do_filemenu(Item::Open);
        // Drain the button-up so the AES does not re-fire the double-click.
        super::evnt_button(1, 0x01, 0x00);
    }

    menu::men_update();
    done
}

/// `gr_obfind` (EmuTOS `deskact.c:50`) — the deepest object of `root` under
/// `(mx, my)` (`objc_find`, the **one** `g_screen` geometry source, the **D7**
/// requirement), refined by `act_chkobj` to the object's own icon/text
/// sub-rectangle. Returns `root` when the point misses every item's glyph/label
/// (empty area / the dead space around a small icon).
fn gr_obfind(root: i16, mx: i16, my: i16) -> i16 {
    let Some(tree) = global::screen_addr() else {
        return root;
    };
    let obid = super::objc_find(tree, root, MAX_DEPTH, mx, my);
    if obid == NIL || obid == root {
        return root;
    }
    if global::with(|g| g.act_chkobj(root, obid, mx, my)).unwrap_or(false) {
        obid
    } else {
        root
    }
}

/// `gr_extent(numpts, xylnpts, pt)` (EmuTOS `deskact.c:169-188`) — the
/// bounding rectangle of a list of points, right/bottom **inclusive**
/// (`gx - lx + 1`). An empty list yields a zero rectangle, which is the
/// degenerate case neither caller can reach.
fn gr_extent(pts: &[Point]) -> Grect {
    let (mut lx, mut ly) = (i16::MAX, i16::MAX);
    let (mut gx, mut gy) = (0i16, 0i16);
    for p in pts {
        lx = lx.min(p.x);
        ly = ly.min(p.y);
        gx = gx.max(p.x);
        gy = gy.max(p.y);
    }
    if pts.is_empty() {
        return Grect::default();
    }
    Grect { x: lx, y: ly, w: gx - lx + 1, h: gy - ly + 1 }
}

/// `gr_obalign(numobs, x, y, xyobpts)` (EmuTOS `deskact.c:234-242`) — make the
/// per-object origins relative to the drag extent's top-left.
fn gr_obalign(pts: &mut [Point], x: i16, y: i16) {
    for p in pts {
        p.x -= x;
        p.y -= y;
    }
}

/// `rc_constrain(pc, pt)` (EmuTOS `util/rectfunc.c:32-42`) — slide `pt` back
/// inside `pc` without resizing it.
fn rc_constrain(pc: Grect, pt: &mut Grect) {
    if pt.x < pc.x {
        pt.x = pc.x;
    }
    if pt.y < pc.y {
        pt.y = pc.y;
    }
    if pt.x + pt.w > pc.x + pc.w {
        pt.x = pc.x + pc.w - pt.w;
    }
    if pt.y + pt.h > pc.y + pc.h {
        pt.y = pc.y + pc.h - pt.h;
    }
}

/// `gr_plns(x, y, numpts, xylnpts, numobs, xyobpts)` (EmuTOS
/// `deskact.c:195-207`) — complement the outline polyline once per dragged
/// object, at `(x, y)` plus that object's aligned origin, with the mouse
/// hidden for the whole batch.
fn gr_plns(x: i16, y: i16, xylnpts: &[Point], xyobpts: &[Point]) {
    super::graf_mouse(M_OFF);
    for o in xyobpts {
        super::vdibind::gsx_pline(x + o.x, y + o.y, xylnpts);
    }
    super::graf_mouse(M_ON);
}

/// `gr_isdown(out, x, y, w, h)` (EmuTOS `deskact.c:66-77`) — block until the
/// pointer leaves the `w`×`h` rectangle at `(x, y)` or the button comes up.
/// `true` means "still dragging".
fn gr_isdown(out: bool, x: i16, y: i16, w: i16, h: i16) -> bool {
    super::evnt_multi_m1(out, x, y, w, h) & MU_BUTTON == 0
}

/// `gr_bwait(po, mx, my, …)` (EmuTOS `deskact.c:209-232`) — draw the drag
/// outline at `po`, wait for the mouse to move or the button to come up, then
/// erase it again. Returns `gr_isdown`'s "still down".
///
/// The clip and the writing mode are re-established on **every** pass, which
/// is EmuTOS's own comment at `:211-214` ("Since the desktop and the AES
/// currently share the same VDI workstation, we have to reset the clipping and
/// drawing mode here"). Complementing twice at the same place is what erases
/// the outline, so between the two `gr_plns` calls nothing may repaint under
/// it — and nothing does: the highlight `act_chg` draws happens *after* the
/// erase, back in [`gr_drgplns`]'s loop body.
fn gr_bwait(po: Grect, mx: i16, my: i16, xylnpts: &[Point], xyobpts: &[Point]) -> bool {
    super::vdibind::gsx_sclip(super::screen_rect());
    super::vdibind::gsx_attr_line(super::vdibind::MD_XOR, super::vdibind::BLACK);
    gr_plns(po.x, po.y, xylnpts, xyobpts); // draw old
    let down = gr_isdown(true, mx, my, 2, 2); // wait for change
    gr_plns(po.x, po.y, xylnpts, xyobpts); // erase old
    down
}

/// `act_chg(wh, root, obj, pc, set, dodraw)` (EmuTOS `deskact.c:419-466`) —
/// the state change plus, when it actually changed something **and** `dodraw`
/// is set, the repaint of the object's own extent clipped to `pc`. The four
/// `gr_drgplns` callers pass `dodraw = TRUE`; `do_wopen` passes its `new_win`
/// (`desksupp.c:316`), so a same-window open or a program launch changes the
/// state without painting it.
///
/// Split here because [`global::with`] is not re-entrant and
/// [`redraw::do_wredraw`] takes the desktop lock itself: the `DeskState` half
/// is [`global::DeskState::act_chg`], the draw half is this wrapper.
fn act_chg_draw(wh: i16, root: i16, obj: i16, pc: Grect, set: bool, dodraw: bool) {
    let Some(t) = global::with(|g| {
        let extent = g.obj_extent(root, obj);
        if g.act_chg(wh, obj, set) {
            extent
        } else {
            None
        }
    })
    .flatten() else {
        return;
    };
    // `if (dodraw && rc_intersect(pc, &t)) do_wredraw(wh, &t);` (`:457-460`) —
    // `rc_intersect` narrows `t` to the visible part before the redraw.
    if !dodraw {
        return;
    }
    if let Some(t) = redraw::rc_intersect(pc, t) {
        redraw::do_wredraw(wh, t);
    }
}

/// `gr_drgplns(…)` (EmuTOS `deskact.c:248-355`) — the desktop's own drag
/// loop: it complements one outline polyline per selected item while the
/// pointer moves, and **highlights the object under the pointer** as it goes.
///
/// Returns `(dulx, duly, offx, offy, dst_wh, dobj)` — the release point, the
/// pointer's offset inside the dragged extent, and the window/object the drop
/// landed on, exactly the six values EmuTOS passes back by pointer.
///
/// **This replaces a `graf_dragbox` call (DESK36).** Before it, NewTOS's
/// desktop handed the whole gesture to the AES: `graf_dragbox` (71) ran the
/// loop, drew one box, and returned only the final position. That is why no
/// target ever lit up — the loop that must call `act_chg` was inside the AES,
/// which knows nothing about `ANODE`s. The loop is now the desktop's, where
/// EmuTOS has it, and reaches the screen through [`super::vdibind`].
///
/// Three rules live in the body and are easy to miss:
///
/// 1. **A plain file is not highlighted** (`:340-341`): the `ANODE` `i_find`
///    resolves must not be an `AT_ISFILE` that matched by *data* name. Folders,
///    drives, the trash, the printer and installed applications all are.
/// 2. **Changing target takes the old highlight back first** (`:325-333`), and
///    an object that is already `SELECTED` is never touched at all (`:335`) —
///    dragging over one's own selection must not flicker it.
/// 3. **The highlight is always taken back on release** (`:353-354`), whether
///    or not the drop then succeeds.
fn gr_drgplns(
    in_mx: i16,
    in_my: i16,
    pc: Grect,
    xylnpts: &[Point],
    xyobpts: &mut [Point],
) -> (i16, i16, i16, i16, i16, i16) {
    super::graf_mouse(FLATHAND); // `graf_mouse(FLATHAND, NULL)` (`:264`)

    let (mut l_mx, mut l_my) = (in_mx, in_my);

    // Extent of one polygon, then of the whole set of origins; the outline's
    // own size is added so the constrained box covers the drawn pixels
    // (`:267-275`).
    let ln = gr_extent(xylnpts);
    let mut o = gr_extent(xyobpts);
    o.w += ln.w;
    o.h += ln.h;
    gr_obalign(xyobpts, o.x, o.y);

    let offx = l_mx - o.x;
    let offy = l_my - o.y;

    let mut curr_wh = 0i16;
    let mut curr_root = 0i16;
    let mut curr_sel = 0i16;
    // Both are set on every pass of the loop below, which always runs at least
    // once — the `do { } while` of `deskact.c:284-352`.
    let mut dst_wh;
    let mut dobj;

    loop {
        o.x = l_mx - offx;
        o.y = l_my - offy;
        rc_constrain(pc, &mut o);
        let down = gr_bwait(o, l_mx, l_my, xylnpts, xyobpts);

        let st = super::graf_mkstate(); // `graf_mkstate(&l_mx, &l_my, …)` (`:301`)
        l_mx = st.mx;
        l_my = st.my;
        dst_wh = super::wind_find(l_mx, l_my);
        // `tree = G.g_screen; root = DROOT; if (dst_wh) { pw = win_find(dst_wh);
        //  if (pw) root = pw->w_root; else dst_wh = DESKWH; }` (`:303-315`).
        let root = if dst_wh == DESKWH {
            DROOT
        } else {
            match global::with(|g| g.win_slot(dst_wh).map(|s| g.windows.slots[s].w_root)).flatten() {
                Some(r) => r,
                None => {
                    dst_wh = DESKWH;
                    DROOT
                }
            }
        };
        dobj = gr_obfind(root, l_mx, l_my);

        // The body's two `continue`s jump to the `while (down)` test of a
        // do-while, i.e. they skip the rest of the body but not the exit test.
        'body: {
            // `:313-323` — nothing under the pointer any more: drop the
            // highlight and go round.
            if dobj == root && curr_sel != 0 {
                act_chg_draw(curr_wh, curr_root, curr_sel, pc, false, true);
                curr_wh = 0;
                curr_root = 0;
                curr_sel = 0;
                break 'body;
            }
            // `:325-333` — the target changed: take the old one back first.
            if dobj != curr_sel && curr_sel != 0 {
                act_chg_draw(curr_wh, curr_root, curr_sel, pc, false, true);
                curr_wh = 0;
                curr_root = 0;
                curr_sel = 0;
            }
            // `:334-351` — an object that is itself part of the dragged
            // selection is never highlighted.
            if global::with(|g| g.is_selected(dobj)).unwrap_or(false) {
                break 'body;
            }
            let Some((atype, isapp)) = dir::i_find_atype(dst_wh, dobj) else {
                break 'body; // `if (pa)` — no ANODE governs it
            };
            if atype == AType::File && !isapp {
                break 'body; // "don't highlight plain files" (`:340-341`)
            }
            curr_wh = dst_wh;
            curr_root = root;
            curr_sel = dobj;
            act_chg_draw(curr_wh, curr_root, curr_sel, pc, true, true);
        }

        if !down {
            break;
        }
    }

    // `:353-354` — the highlight always comes back off at the end.
    if curr_sel != 0 {
        act_chg_draw(curr_wh, curr_root, curr_sel, pc, false, true);
    }

    super::pro::desk_busy_off(); // `desk_busy_off()` (`:362`)
    (l_mx, l_my, offx, offy, dst_wh, dobj)
}

/// `act_bdown` (EmuTOS `deskact.c:609`) — the button-held handler, run after
/// `act_bsclick` when the button is still down: a **rubber-band** select when
/// the press missed every item, or a **drag** of the current selection when it
/// landed on an already-selected item. On a drag, resolves the drop
/// destination and either repositions desktop icons in place (`move_drvicon`,
/// bypassing `fun_drag`) or dispatches [`dir::fun_drag`]. Returns the
/// desktop-exit flag. The drop-time modifier keystate (Copy vs. Move) is re-read
/// from `graf_mkstate` at release, matching EmuTOS's per-frame keystate tracking.
fn act_bdown(handle: i16, root: i16, mx: i16, my: i16) -> bool {
    let sobj = gr_obfind(root, mx, my);

    // Nothing under the press point → rubber-band select. `minw = minh =
    // i16::MIN` (EmuTOS `deskact.c:631` passes `SHRT_MIN`) requests a free
    // 4-quadrant band: `graf_rubberbox` returns the *signed* `cursor - origin +
    // 1` deltas (§70) instead of clamping a leftward/upward drag back to a 1×1
    // box — that clamp was the original #9 bug. The reconstruction below then
    // mirrors `deskact.c:632-642` (`l_mw < 0` → `l_mx += l_mw; l_mw = -l_mw`):
    // `mx.min(mx + w)` is the left edge, `w.abs()` the width, so both drag
    // directions yield the same box `min(anchor, cursor)..max(anchor, cursor)`;
    // `.max(1)` keeps a degenerate zero-extent drag a 1 px selection band.
    if sobj == root {
        let (w, h) = super::graf_rubberbox(mx, my, i16::MIN, i16::MIN);
        let (rx, ry) = (mx.min(mx + w), my.min(my + h));
        let (rw, rh) = (w.abs().max(1), h.abs().max(1));
        if global::with(|g| g.act_allchg_rect(handle, root, rx, ry, rw, rh)).unwrap_or(false) {
            redraw_window(handle);
        }
        return false;
    }

    // A held button on an already-selected item → drag the whole selection.
    if !global::with(|g| g.is_selected(sobj)).unwrap_or(false) {
        return false;
    }
    // `obpts = gr_accobs(tree, root, &numobs); if (numobs) { … }` (`:649-651`).
    let Some(mut obpts) = global::with(|g| g.gr_accobs(root)) else {
        return false;
    };
    if obpts.is_empty() {
        return false;
    }
    // `view = (root == DROOT) ? V_ICON : G.g_iview;` (`:653-662`) — the desktop
    // is always icon view; a window uses its own. The outline list follows.
    let view = if root == DROOT {
        View::Icon
    } else {
        global::with(|g| g.view).unwrap_or(View::Icon)
    };
    let xylnpts: Vec<Point> = global::with(|g| match view {
        View::Icon => g.g_xyicon.to_vec(),
        View::Text => g.g_xytext.to_vec(),
    })
    .unwrap_or_default();
    // The extent origin `gr_drgplns`'s `gr_obalign` is about to subtract — kept
    // so the `move_drvicon` arm below can express the drag as a delta.
    let o0 = gr_extent(&obpts);

    // `gr_drgplns(l_mx, l_my, &gl_rfull, …)` (`:663-664`): the bound is the
    // screen below the menu bar, which is `G.g_desk`.
    let desk = g_desk();
    let (dulx, duly, offx, offy, dst_wh, dobj) =
        gr_drgplns(mx, my, desk, &xylnpts, &mut obpts);

    // `evnt_button(0x01, 0x01, 0x00, &l_mx, &l_my, &button, keystate)` (`:681`)
    // — consume the release the drag loop ended on, so the main loop's next
    // `evnt_multi` does not see it, and take the drop-time modifier state from
    // there (Copy vs. Move) as EmuTOS does.
    super::evnt_button(1, 0x01, 0x00);
    let drop_keystate = super::graf_mkstate().kstate as u16;

    if dst_wh != DESKWH {
        // `if ((wh == dst_wh) && (*pdobj == sobj)) dst_wh = NIL;` (`:666-670`)
        // — a drop on the dragged item itself cancels.
        if dst_wh == handle && dobj == sobj {
            return false;
        }
    } else if handle == DESKWH && dobj == DROOT {
        // `move_drvicon(tree, root, dulx, duly, obpts, offx, offy)` (`:673-676`)
        // — desktop icons dropped on free desktop space move in place, without
        // `fun_drag`. EmuTOS snaps each icon from `dulx + pts[i]`; because
        // `gr_obalign` made `pts[i]` relative to the extent origin, that is the
        // same rigid translation as "extent origin moved from `o0` to
        // `(dulx - offx, duly - offy)`", which is what `move_drvicon` takes.
        let dx = (dulx - offx) - o0.x;
        let dy = (duly - offy) - o0.y;
        if global::with(|g| g.move_drvicon(dx, dy)).unwrap_or(false) {
            redraw::do_wredraw(DESKWH, desk);
        }
        return false;
    }

    // Otherwise dispatch the drag through the deskfun dispatch tree. The point
    // handed on is EmuTOS's `*in_mx = dulx - offx; *in_my = duly - offy;`
    // (`deskact.c:683-684`) — the top-left of the dragged outline, which
    // `ins_shortcut` snaps to the grid.
    let done = dir::fun_drag(handle, dst_wh, dobj, dulx - offx, duly - offy, drop_keystate);
    desk_clear(handle);
    done
}

/// What a double-clicked / File→Open item resolves to.
enum OpenTarget {
    /// A sub-folder: its search spec + window title.
    Folder(String, String),
    /// A drive icon. `do_dopen` reads its letter from the icon itself
    /// (`ib_char & 0x00ff`, `desksupp.c:1050-1051`).
    Drive,
    /// A native NewTOS program to launch (ADR-018): its full NUL-terminated
    /// `C:` path, the bare filename (for a failure alert), and the native
    /// flavour that selects the launch behaviour.
    App { path: String, name: String, flavor: ftype::Flavor },
    /// A classic m68k Atari binary — needs the (not-yet-available) emulator.
    M68k,
    /// A document file: look up an installed application association (deskins
    /// #20) and, if one exists, launch it with the document as the command
    /// tail; otherwise report that no application is installed. Carries the
    /// document's bare file name and its full `C:` path (no trailing NUL).
    Doc { name: String, full_path: String },
    /// A desktop **file** shortcut (DESK30): its full path and bare name.
    /// Resolved into one of the arms above once the file has been confirmed
    /// to still exist — `do_aopen`'s `file_exists` check
    /// (`desksupp.c:861-868`), which needs a GEMDOS trap and so cannot run
    /// under the desktop lock. `obid` identifies the icon for the orphan
    /// prompt.
    Shortcut { obid: i16, path: String, name: String },
    /// Nothing openable (the Trash / Printer desktop fixtures).
    None,
}

/// `do_open(pwin, curr)` (EmuTOS `desksupp.c:1190-1280`) — open the item object
/// `obid` of window `handle`: a folder opens in a new directory window, a drive
/// icon at its root, a native `.NAP`/`.NCP`/`.NTP` program launches (ADR-018),
/// and a classic m68k binary raises an honest "needs the emulator" alert. A
/// document goes through its installed-application association.
///
/// Returns `do_open`'s `WORD`: *"TRUE iff `shel_write()` was issued
/// successfully"* (`do_aopen`'s contract, `desksupp.c:842`), which is what
/// makes the desktop's event loop end so the AES shell can start the program.
/// Every arm that does not launch answers `FALSE`, exactly as `do_open`'s
/// `break`s fall through to its trailing `return FALSE` (`:1277`).
fn open_item(handle: i16, obid: i16) -> bool {
    let target = global::with(|g| {
        let fnode = g.g_screeninfo[obid as usize].fnode;
        if fnode >= 0 {
            let Some(slot) = g.win_slot(handle) else {
                return OpenTarget::None;
            };
            let pn = &g.windows.slots[slot].w_pnode;
            let Some(f) = pn.p_flist.get(fnode as usize) else {
                return OpenTarget::None;
            };
            if f.is_dir() {
                let name = core::str::from_utf8(f.name_bytes()).unwrap_or("");
                let (spec, title) = child_spec(&pn.p_spec, name);
                OpenTarget::Folder(spec, title)
            } else {
                // Plain file: classify by extension (ADR-018 routing).
                let name_b = f.name_bytes();
                match ftype::classify(name_b) {
                    ftype::FileType::Native(flavor) => {
                        let name = String::from(core::str::from_utf8(name_b).unwrap_or(""));
                        let path = native_path(&pn.p_spec, name_b);
                        OpenTarget::App { path, name, flavor }
                    }
                    ftype::FileType::M68k => OpenTarget::M68k,
                    ftype::FileType::Doc => {
                        let name = String::from(core::str::from_utf8(name_b).unwrap_or(""));
                        let full_path = doc_full_path(&pn.p_spec, name_b);
                        OpenTarget::Doc { name, full_path }
                    }
                }
            }
        } else {
            // Desktop icon: `pa = i_find(DESKWH, curr, &pf, &isapp)`
            // (`desksupp.c:1206`), i.e. the `ANODE` the object was built from,
            // then a switch on its `a_type` (`:1219-1271`).
            match g.app_afind_by_id(obid).map(|i| &g.g_ahead[i]) {
                // A **desktop shortcut** (DESK30, `:1227-1245`): `a_pappl` is
                // the full path and `a_pdata` the bare name, so the item is
                // opened exactly as its twin in a window would be — the same
                // extension routing (ADR-018), the same document association.
                Some(pa) if pa.a_type == AType::File => OpenTarget::Shortcut {
                    obid,
                    path: pa.a_pappl.clone(),
                    name: pa.a_pdata.clone(),
                },
                // A folder shortcut: `strcat(pathname, "\\*.*")` after the
                // path, and the path itself as the window title (`:1231-1244`,
                // `add_one_level`). `a_pappl` has no trailing separator.
                Some(pa) if pa.a_type == AType::Folder => {
                    let mut title = pa.a_pappl.clone();
                    title.push('\\');
                    let mut spec = title.clone();
                    spec.push_str("*.*");
                    OpenTarget::Folder(spec, title)
                }
                // A drive: the drive letter overlay in `ib_char`'s low byte.
                _ => {
                    let letter = (g.g_screeninfo[obid as usize].block.ib_char & 0xFF) as u8;
                    if letter.is_ascii_uppercase() {
                        OpenTarget::Drive
                    } else {
                        OpenTarget::None
                    }
                }
            }
        }
    })
    .unwrap_or(OpenTarget::None);

    // `do_aopen`'s shortcut prologue (`desksupp.c:857-868`): "if this is a
    // desktop shortcut, first make sure that the file exists" — and if it does
    // not, offer to remove the icon or to locate the item. Outside the desktop
    // lock, because the probe is a GEMDOS trap. EmuTOS runs this check for the
    // `AT_ISFILE` arm only; a folder shortcut goes straight into `do_fopen`
    // (`:1253-1259`), so an orphaned one opens an empty window there as it does
    // here, and is caught by *Show info* or by a drag instead.
    let target = match target {
        OpenTarget::Shortcut { obid, path, name } => {
            if !dir::shortcut_exists(&path, false) {
                dir::remove_locate_shortcut(obid);
                return false; // `remove_locate_shortcut(curr); return FALSE;` (`:865-866`)
            }
            match ftype::classify(name.as_bytes()) {
                ftype::FileType::Native(flavor) => {
                    let mut p = path;
                    p.push('\0');
                    OpenTarget::App { path: p, name, flavor }
                }
                ftype::FileType::M68k => OpenTarget::M68k,
                ftype::FileType::Doc => OpenTarget::Doc { name, full_path: path },
            }
        }
        other => other,
    };

    match target {
        // `do_fopen(pw, curr, pathname, TRUE)` (`desksupp.c:1259`) — both of
        // NewTOS's callers are EmuTOS's `do_open`, which passes `allow_new_win
        // = TRUE` (a double click goes through `do_filemenu(OPENITEM)`,
        // `deskmain.c:988,666-668`, exactly as the File→Open menu pick does).
        OpenTarget::Folder(spec, title) => {
            // `pw->w_cvrow = 0;  /* reset slider */` (`:1258`) — on the window
            // the folder was opened **from**, before `do_fopen` decides whether
            // that window is reused.
            global::with(|g| {
                if let Some(slot) = g.win_slot(handle) {
                    g.windows.slots[slot].w_cvrow = 0;
                }
            });
            do_fopen(handle, obid, &spec, &title, true);
            false // `break;` → `return FALSE` (`:1272,1277`)
        }
        OpenTarget::Drive => {
            do_dopen(obid); // `do_dopen(curr); break;` (`:1263-1264`)
            false
        }
        OpenTarget::App { path, name, flavor } => {
            // `do_aopen`'s `else` arm (`desksupp.c:903-912`): the program is
            // not an installed application, so the **desktop** config flag
            // `G.g_appdir` picks the directory. A failure aborts the launch.
            let appdir = global::with(|g| g.g_appdir).unwrap_or(true);
            if !set_launch_dir(appdir, &path) {
                return false;
            }
            // `if (tail) … else if (isparm) ret = opn_appl(pname, ptail);`
            // (`desksupp.c:969-985`) — a plain double click passes no tail
            // (EmuTOS's `tail == NULL`), so a parameter-taking program is
            // asked for its command line here, before `pro_run`. `pname` is
            // the bare file name, which is what the box shows.
            let mut tail: Vec<u8> = Vec::new();
            if isparm(flavor) && !super::inf::opn_appl(&name, &mut tail) {
                // `return ret ? pro_run(…) : FALSE;` — Cancel starts nothing.
                return false;
            }
            // `return pro_run(isgraf, app_path, G.g_work, G.g_cwin, curr);`
            // (`:986`).
            pro_run_native(&path, &name, flavor, &tail, obid)
        }
        OpenTarget::M68k => {
            let text = b"[1][ This is an m68k Atari program. | The m68k emulator is not | available yet. ][ OK ]\0";
            super::form_alert(1, text.as_ptr());
            // No desktop repaint: the AES `form_alert` bb_save/restores the screen
            // under the alert (EmuTOS `fm_alert`), so the desktop returns intact.
            false
        }
        OpenTarget::Doc { name, full_path } => open_document(&name, &full_path, obid),
        // Resolved into one of the arms above by the prologue.
        OpenTarget::Shortcut { .. } | OpenTarget::None => false,
    }
}

/// Open a document (deskins #20 / EmuTOS `do_aopen`'s `installed_datafile`
/// branch, `desksupp.c:924-951`): resolve the installed application association
/// for the document's type and, if one exists, launch that application with the
/// document as its command tail. With no association installed, report it
/// honestly (EmuTOS would offer Show/Print or alert `STNOAPPL`; NewTOS raises
/// the "no application" alert — Show/Print is a separate, unimplemented feature).
///
/// The command tail is the association's fixed argument prefix (`a_pargs`),
/// then either the document's full path (`AF_ISFULL`) or just its file name
/// (`deskins.md` §`ins_app`, `desksupp.c:942-950`); `pro_run` turns it into the
/// classic Atari `[len][args][\0][\r]` image (`deskpro.c:56-57`). The
/// associated application's own extension selects `isgraf`, exactly as a direct
/// double-click would.
///
/// Returns `do_aopen`'s `WORD` — TRUE iff the `shel_write` was issued.
fn open_document(name: &str, full_path: &str, curr: i16) -> bool {
    let Some(assoc) = super::inf::lookup_association(name.as_bytes()) else {
        let text = b"[1][ No application is installed | for this document type. ][ OK ]\0";
        super::form_alert(1, text.as_ptr());
        // AES `form_alert` bb_save/restores the screen — no desktop repaint.
        return false; // `fun_alert(1, STNOAPPL); return FALSE;` (`:1016,1020`)
    };

    // `do_aopen`'s `if (is_installed(pa))` arm (`desksupp.c:896-902`): an
    // installed application answers the directory question from its **own**
    // `AF_APPDIR` flag, not from the desktop-wide `G.g_appdir`. A failure
    // aborts the launch, as it does for a direct one.
    if !set_launch_dir(assoc.app_dir, &assoc.app_path) {
        return false;
    }

    // The associated application must itself be a native NewTOS program.
    let app_name = filename_of(&assoc.app_path);
    let flavor = match ftype::classify(app_name.as_bytes()) {
        ftype::FileType::Native(f) => f,
        _ => {
            let text = b"[1][ The associated application is | not a native NewTOS program. ][ OK ]\0";
            super::form_alert(1, text.as_ptr());
            // AES `form_alert` bb_save/restores the screen — no desktop repaint.
            return false;
        }
    };

    // `strcpy(ptail, pa->a_pargs); … strcpy(p, pname);` (`:941-950`) — the
    // fixed args, then the full path or just the file name.
    let mut tail = String::from(assoc.args.as_str());
    tail.push_str(if assoc.full_path { full_path } else { name });

    // NUL-terminate the application path for `shel_write`'s `pcmd`.
    let mut app_path = String::from(assoc.app_path.as_str());
    app_path.push('\0');
    // `return pro_run(isgraf, pa->a_pappl, G.g_work, G.g_cwin, curr);` (`:951`)
    // The `installed_datafile` arm returns before `isapp`'s `isparm` test, so
    // an association never puts `opn_appl` up: the document *is* the tail.
    pro_run_native(&app_path, &app_name, flavor, tail.as_bytes(), curr)
}

/// `do_aopen(pa, TRUE, curr, pathname, pname, tail)` (`desksupp.c:843-1031`)
/// for the two cases that reach it with `isapp = TRUE` and no document
/// association: `fun_win2win`'s drop-on-an-application branch
/// (`deskfun.c:967`, `tail != NULL`) and a function-key launch
/// (`deskmain.c:1084`, `tail == NULL`). The `installed_datafile` arm (`:924`)
/// is ruled out by `isapp` in both. What is left is four steps:
///
/// * the current directory, set exactly as `do_aopen` sets it (`:873-890`):
///   from the node's own `AF_APPDIR` + `a_pappl` when it is an **installed**
///   application, otherwise from the desktop-wide `G.g_appdir` and the
///   **destination window's** `p_spec` — that is [`set_launch_dir`], shared
///   with the double-click and document paths;
/// * `app_path = pathname` with its file-name part replaced by `pname`
///   (`:958-960`) — here `dst_dir` (which already ends in `\`) plus
///   `prog_name`;
/// * the tail: `strcpy(ptail, tail)` when there is one, **else** `isparm`'s
///   [`super::inf::opn_appl`] prompt (`:969-985`). The `tail` branch takes
///   precedence, which is why a file dropped on a `.NTP` is passed straight
///   through without a box;
/// * `pro_run` (`:986`).
///
/// `installed`/`pappl`/`node_appdir` are the three fields of the destination
/// `ANODE` those steps read. `curr` is `do_aopen`'s `curr`, handed on to
/// `pro_run` for its `do_wopen` step: the object dropped on (`dst_ob`/`dobj`,
/// `deskfun.c:967,1034,1249`), or `-1` for a function key (`deskmain.c:1102`). `tail` is `Option` because EmuTOS's two branches
/// separate on `tail == NULL`, not on an empty string: `Some` is the command
/// line [`dir::build_selected_path`] built, `None` is the function key's
/// `do_aopen(pa, TRUE, -1, pathname, pfname, NULL)`.
///
/// Returns `do_aopen`'s `WORD`, i.e. EmuTOS's `exit_desktop` for this drop:
/// TRUE iff the `shel_write` was issued, which ends the desktop's event loop
/// (`fun_drag` → `hndl_button` → `deskmain.c:2103-2104`).
pub(super) fn do_aopen_drop(
    installed: bool,
    pappl: &str,
    node_appdir: bool,
    dst_dir: &str,
    prog_name: &str,
    tail: Option<&[u8]>,
    curr: i16,
) -> bool {
    // `if (is_installed(pa)) { … AF_APPDIR … pa->a_pappl } else { … G.g_appdir
    //  … pathname }` (`desksupp.c:884-905`).
    let (app_dir, own_path) = if installed {
        (node_appdir, String::from(pappl))
    } else {
        (global::with(|g| g.g_appdir).unwrap_or(true), String::from(dst_dir))
    };
    if !set_launch_dir(app_dir, &own_path) {
        return false;
    }

    // `strcpy(app_path, pathname); p = filename_start(app_path); strcpy(p, pname);`
    let mut app_path = String::from(dst_dir);
    app_path.push_str(prog_name);

    // EmuTOS hands `app_path` to `pro_run` whatever it is and lets the loader
    // fail; NewTOS's loader takes native AArch64 ELF only (ADR-018), so the
    // two non-native answers are reported here, in the wording the
    // double-click path already uses for them.
    match ftype::classify(prog_name.as_bytes()) {
        ftype::FileType::Native(flavor) => {
            app_path.push('\0'); // `shel_write`'s `pcmd` reads up to the NUL
            // `if (tail) strcpy(ptail, tail); else if (isparm) ret =
            //  opn_appl(pname, ptail);` (`:969-985`).
            let mut prompted: Vec<u8> = Vec::new();
            let tail = match tail {
                Some(t) => t,
                None if isparm(flavor) => {
                    if !super::inf::opn_appl(prog_name, &mut prompted) {
                        return false; // `return ret ? pro_run(…) : FALSE;`
                    }
                    &prompted
                }
                None => b"",
            };
            // `return pro_run(isgraf, app_path, G.g_work, G.g_cwin, curr);`
            // (`:986`).
            pro_run_native(&app_path, prog_name, flavor, tail, curr)
        }
        ftype::FileType::M68k => {
            let text = b"[1][ This is an m68k Atari program. | The m68k emulator is not | available yet. ][ OK ]\0";
            super::form_alert(1, text.as_ptr());
            false
        }
        // Reachable, and an EmuTOS quirk rather than an accident: an installed
        // application's `ANODE` carries `a_aicon = IG_APPL` **and** its
        // document wildcard, and `app_afind_by_name` tests `a_pdata` first
        // (`deskapp.c:1450-1454`), so a document of that type answers
        // `a_aicon >= 0` too and EmuTOS tries to run the document itself.
        ftype::FileType::Doc => {
            let text = b"[1][ This file is not a native | NewTOS program. ][ OK ]\0";
            super::form_alert(1, text.as_ptr());
            false
        }
    }
}

/// The bare file name of a `C:` path (the part after the last `\`).
fn filename_of(path: &str) -> String {
    match path.rfind('\\') {
        Some(i) => String::from(&path[i + 1..]),
        None => String::from(path),
    }
}

/// The full `C:` path of file `name` living in the directory of `p_spec` (a
/// window search spec like `C:\SUB\*.*`): the directory part (up to and
/// including the last `\`) + `name`. No trailing NUL.
fn doc_full_path(p_spec: &str, name: &[u8]) -> String {
    let dir = &p_spec[..=p_spec.rfind('\\').unwrap_or(0)]; // "C:\...\"
    let mut s = String::from(dir);
    if let Ok(n) = core::str::from_utf8(name) {
        s.push_str(n);
    }
    s
}

/// [`doc_full_path`] plus a trailing `\0`, ready to hand to `shel_write`'s
/// `pcmd` (which reads up to the first NUL).
fn native_path(p_spec: &str, name: &[u8]) -> String {
    let mut s = doc_full_path(p_spec, name);
    s.push('\0');
    s
}

/// `STDEFDIR` (`desk_rsc.c:2558-2559`) — the alert `do_aopen` raises when the
/// current directory cannot be set. [`dir::fun_alert`] supplies the `[1][`…
/// `][ OK ]` frame around it.
const STDEFDIR: &[u8] = b"Failed to set default|directory.";

/// The current-directory step of EmuTOS's `do_aopen` (`desksupp.c:893-916`) —
/// the piece NewTOS had no counterpart for at all, so that a launched program
/// inherited whatever directory happened to be current.
///
/// ```c
/// pw = win_ontop();
/// if (is_installed(pa)) {
///     if (!(pa->a_flags & AF_APPDIR) && pw) p = pw->w_pnode.p_spec;
///     else                                  p = pa->a_pappl;
/// } else {
///     if (!G.g_appdir && pw) p = pw->w_pnode.p_spec;
///     else                   p = pathname;
/// }
/// strcpy(app_path, p);
/// p = filename_start(app_path);
/// *p = '\0';
/// if (set_default_path(app_path) < 0) { fun_alert(1, STDEFDIR); return FALSE; }
/// ```
///
/// The two arms differ only in *where the switch comes from* — the
/// application's own `AF_APPDIR` when it is installed, the desktop-wide
/// `G.g_appdir` when it is not — so both callers pass it in as `app_dir` and
/// share this body, as they share `do_aopen`'s in EmuTOS.
///
/// `own_path` is EmuTOS's `pa->a_pappl` / `pathname`. For the non-installed
/// arm EmuTOS passes the **window's search spec** (`C:\SUB\*.*`) and NewTOS
/// passes the program's own full path (`C:\SUB\PROG.NCP`): `filename_start`
/// cuts both back to the identical `C:\SUB\`, which is the only thing this
/// function does with it.
///
/// Returns `false` when the directory could not be set — the caller must then
/// **not** launch, exactly as `do_aopen` returns `FALSE` there.
fn set_launch_dir(app_dir: bool, own_path: &str) -> bool {
    // `pw = win_ontop()` — ours is `wind_get(DESKWH, WF_TOP)`, as everywhere
    // else in this file; `DESKWH` is the "no real window" answer.
    let pw = wind_get_top();
    let from_window = if !app_dir && pw != DESKWH {
        global::with(|g| g.win_slot(pw).map(|s| g.windows.slots[s].w_pnode.p_spec.clone())).flatten()
    } else {
        None
    };
    let src = from_window.unwrap_or_else(|| String::from(own_path));

    // `p = filename_start(app_path); *p = '\0';` — keep everything up to and
    // including the last separator (`optimize.c:226-235`). A string with no
    // separator at all collapses to empty, as it does there.
    let dir = &src[..src.rfind('\\').map_or(0, |i| i + 1)];

    // The desktop lock is released before the trap: `set_default_path` issues
    // `graf_mouse`, `Dsetdrv` and `Dsetpath`.
    if pro::set_default_path(dir) < 0 {
        dir::fun_alert(1, STDEFDIR);
        // The AES `form_alert` bb_save/restores the screen under the alert, so
        // the desktop comes back intact and the caller simply does not launch.
        return false;
    }
    true
}

/// `do_aopen`'s launch tail (`desksupp.c:955-986`) for a native NewTOS program
/// (ADR-018):
///
/// ```c
/// strcpy(app_path, pathname);
/// p = filename_start(app_path);
/// strcpy(p, pname);
/// if (isapp)
/// {
///     ret = TRUE;
///     if (tail)        strcpy(ptail, tail);
///     else if (isparm) ret = opn_appl(pname, ptail);
///     return ret ? pro_run(isgraf, app_path, G.g_work, G.g_cwin, curr) : FALSE;
/// }
/// ```
///
/// The three callers assemble `app_path` themselves — the double click, the
/// document association and the drop-on-an-application reach it by different
/// routes — and hand it in NUL-terminated; what they share is the `G.g_work`
/// tail image and the single `pro_run`.
///
/// **This is the desktop's only way to start a program.** There is no loader
/// call here and none anywhere else in `desk/`: `pro_run` issues
/// `shel_write(SHW_EXEC, …)` (`deskpro.c:41`), the caller's `done` takes the
/// desktop out (`deskmain.c:988`, [`super::run`]'s epilog), and the start
/// itself belongs to [`crate::shell_loop::sh_ldapp`] — where EmuTOS leaves it
/// too (`gemshlib.c:645`).
///
/// The `ret = opn_appl(pname, ptail)` line above is **not** here: it belongs to
/// the branch, not to the launch, so it sits with each `do_aopen` embodiment —
/// [`open_item`]'s `App` arm and [`do_aopen_drop`] — exactly where the source
/// has it. What reaches this function is the finished tail.
///
/// `tail` is bytes, not a `&str`, because one of its three producers is
/// [`super::inf::opn_appl`]'s `X`-validated edit field, which admits every
/// character the AES's `check` lets through.
fn pro_run_native(path: &str, name: &str, flavor: ftype::Flavor, tail: &[u8], curr: i16) -> bool {
    if flavor == ftype::Flavor::Accessory {
        // A desk accessory (`.NAC`) is **not** launched on demand (ACC02): the
        // AES loads it once per session from `C:\ACC\` or the root and it lives
        // for the whole session. EmuTOS's desktop has no launch path for an
        // `.ACC` either — `count_accs`/`load_accs` (`aes/geminit.c:264-315`) is
        // the only way one ever starts. Say so rather than start a second,
        // unreachable copy of it.
        alert_accessory_not_launchable(name);
        return false;
    }
    // `G.g_work[1] = '\0'; ptail = G.g_work + 1; strcpy(ptail, tail);`
    // (`desksupp.c:918-919,969-975`) — the tail **text** goes at byte 1;
    // `pro_run` writes the length byte in front of it and the CR behind it
    // (`deskpro.c:56-57`). Clamped to the same bound `pro_run` applies.
    let mut work = [0u8; pro::CMDTAILSIZE];
    let n = tail.len().min(pro::CMDTAILSIZE - 3);
    work[1..1 + n].copy_from_slice(&tail[..n]);
    // `pro_run(isgraf, app_path, G.g_work, G.g_cwin, curr)` — `G.g_cwin` read
    // at the call, as the argument list evaluates it.
    let cwin = global::with(|g| g.g_cwin).unwrap_or(DESKWH);
    pro::pro_run(isgraf(flavor), path.as_bytes(), &mut work, cwin, curr)
}

/// `isgraf = pa->a_flags & AF_ISCRYS;` (`desksupp.c:876`) — *"is GEM
/// application"* (`deskapp.h:28`). It is `shel_write`'s `isgr` argument, which
/// the AES records as `sh_isgem` (`gemshlib.c:173`) and `sh_chgrf` turns into
/// the screen mode of the next `sh_main` pass (`:513-523`): a GEM program keeps
/// the graphics screen, a character program gets `sh_toalpha`.
///
/// For a file that is not an installed application EmuTOS reads the flag off
/// the built-in `EMUDESK.INF` entry for its extension (`deskapp.c:181-188`,
/// parsed by `app_parse`, `:311-322`): `#G *.APP`/`*.PRG` and `#Y *.GTP` carry
/// `AF_ISCRYS`, `#F *.TOS` and `#P *.TTP` do not. The native family maps onto
/// that table through the analogy [`super::ftype`] documents.
///
/// **`.NTP` is `*.TTP` (`#P`), not `*.GTP` (`#Y`)** — user decision 2026-09-26,
/// DESK44. The two classic parameter extensions differ in exactly this flag:
/// `app_parse`'s `case 'Y'` falls through into `case 'G'` and so sets
/// `AF_ISPARM | AF_ISCRYS`, while `case 'P'` falls through into `case 'F'` and
/// sets `AF_ISPARM` alone (`deskapp.c:310-322`). The ADR-018 family names the
/// console pair after the classic console pair — `.NCP` is `*.TOS`, `.NTP` is
/// `*.TOS`-with-parameters — so `.NTP` is a **character** program and
/// `sh_chgrf` must take the screen to alpha for it (`gemshlib.c:513-523`).
/// [`super::inf::default_type_anodes`] already writes `#P` for `*.NTP`; this is
/// the side that was out of step with it.
fn isgraf(flavor: ftype::Flavor) -> bool {
    match flavor {
        // `.NAP` = `*.PRG`/`*.APP` (`#G`), the only native GEM flavour.
        ftype::Flavor::App => true,
        // `.NCP` = `*.TOS` (`#F`), `.NTP` = `*.TTP` (`#P`) — both character
        // programs; `#P` differs from `#F` only in `AF_ISPARM`, which is
        // `isparm`, not `isgraf` (`desksupp.c:876-877`).
        ftype::Flavor::Console | ftype::Flavor::Params => false,
        // `.NAC` never reaches `pro_run` — [`pro_run_native`] turns it away
        // before this is asked.
        ftype::Flavor::Accessory => true,
    }
}

/// `isparm = pa->a_flags & AF_ISPARM;` (`desksupp.c:877`) — *"is parameter
/// application"* (`deskapp.h:29`), i.e. the program wants a command line typed
/// for it. It is the flag `do_aopen` tests before it puts
/// [`super::inf::opn_appl`] up (`:978-985`), and nothing else reads it.
///
/// `app_parse`'s table has it on exactly the two classic parameter extensions
/// (`deskapp.c:181-188`, parsed at `:310-322`): `#P *.TTP` sets `AF_ISPARM`
/// alone, `#Y *.GTP` sets `AF_ISPARM | AF_ISCRYS`. The native family has one
/// such extension, `.NTP`, and DESK44 fixed it as the `*.TTP` (`#P`) twin — so
/// it is the one flavour that answers `true` here, and the one that answers
/// `false` to [`isgraf`].
fn isparm(flavor: ftype::Flavor) -> bool {
    match flavor {
        // `.NTP` = `*.TTP` (`#P`): `AF_ISPARM`.
        ftype::Flavor::Params => true,
        // `.NAP` = `*.PRG`/`*.APP` (`#G`) and `.NCP` = `*.TOS` (`#F`) carry no
        // `AF_ISPARM`; a `.NAC` never reaches `do_aopen`'s `isapp` branch.
        ftype::Flavor::App | ftype::Flavor::Console | ftype::Flavor::Accessory => false,
    }
}

/// Alert for a double-clicked desk accessory (`.NAC`): it is already loaded (or
/// was not found at boot) and cannot be started a second time — see
/// [`pro_run_native`]'s `Accessory` guard. EmuTOS has no message for this
/// because its desktop cannot reach the case at all; ours can, so it says what
/// happened instead of doing nothing.
fn alert_accessory_not_launchable(name: &str) {
    let mut buf = [0u8; 128];
    let mut p = 0usize;
    for part in [
        b"[1][ ".as_slice(),
        name.as_bytes(),
        b" is a desk accessory.|It is started from C:\\ACC\\|at boot, not from here. ][ OK ]",
    ] {
        for &b in part {
            if p < buf.len() - 1 {
                buf[p] = b;
                p += 1;
            }
        }
    }
    buf[p] = 0;
    super::form_alert(1, buf.as_ptr());
}

// ── Keyboard (D4) ──────────────────────────────────────────────────────────

/// `hndl_kbd` (EmuTOS `deskmain.c:1201`) — the keyboard handler. `key` is the
/// AES `EV_KRET` (`scancode << 8 | ascii`). Checked in EmuTOS order, first match
/// wins; CR / ESC / arrow keys return early (no `men_update`), everything else
/// ends with `men_update`. Returns `true` to end the loop.
fn hndl_kbd(key: i16) -> bool {
    let ascii = (key & 0xFF) as u8;

    // 1. CR — deselect everything (no men_update).
    if ascii == 0x0D {
        desk_clear_all();
        return false;
    }
    // 2. ESC — refresh the top window (no men_update).
    if ascii == 0x1B {
        refresh_top();
        return false;
    }
    // 3. Arrow key — scroll the top window (no men_update).
    if let Some(action) = check_arrow_key(key) {
        kbd_arrow(action);
        return false;
    }
    // 4. Function key F1-F20 — launch its installed application. EmuTOS
    //    `return`s the answer straight out (`deskmain.c:1224-1226`, `if (done
    //    >= 0) return done;`), so — like CR/ESC/arrows above it — this path
    //    does **not** run `men_update` (DESK33).
    if let Some(done) = check_function_key(key) {
        return done;
    }
    // 5. Alt+letter — open that drive.
    if check_alt_letter_key(key) {
        menu::men_update();
        return false;
    }
    // 6. Ctrl-shortcut — dispatch through hndl_menu, same as a mouse menu click.
    let done = if let Some(item) = menu::lookup_ascii_shortcut(ascii) {
        let title = menu::title_obj(menu::item_title(item));
        super::menu_tnormal(menu::tree_ptr(), title, false); // flash the title
        hndl_menu(item, title)
    } else {
        false
    };
    menu::men_update();
    done
}

// The arrow-key scancode → `WM_ARROWED` action table (EmuTOS `arrow_table[]`,
// `deskmain.c:316-325`; the full 16-bit `scancode<<8|ascii` word). Shift-arrow
// page entries carry the numpad ASCII EmuTOS's IKBD produces; NewTOS's IKBD
// yields ascii 0 for arrows regardless of shift, so those page rows never match
// and plain arrows drive a line scroll — a minor page-scroll fidelity gap, not
// a defect.
const ARROW_TABLE: [(i16, i16); 6] = [
    (0x4838, WA_UPPAGE),
    (0x5032, WA_DNPAGE),
    (0x4800, WA_UPLINE),
    (0x5000, WA_DNLINE),
    (0x4b00, WA_LFLINE),
    (0x4d00, WA_RTLINE),
];

/// `check_arrow_key` (EmuTOS `deskmain.c:1067`) — an arrow scancode → its
/// `WM_ARROWED` action, or `None`.
fn check_arrow_key(key: i16) -> Option<i16> {
    ARROW_TABLE.iter().find(|&&(k, _)| k == key).map(|&(_, a)| a)
}

/// `kbd_arrow` (EmuTOS `deskmain.c:1043`) — scroll the topped window by an
/// arrow action: no-op on the desktop or an unresolved handle; else deselect
/// the window first, then scroll and repaint.
fn kbd_arrow(action: i16) {
    let handle = wind_get_top();
    if handle == DESKWH {
        return;
    }
    let Some(slot) = global::with(|g| g.win_slot(handle)).flatten() else {
        return;
    };
    desk_clear(handle);
    let work = wind_get_work(handle);
    if let Some(sliders) = global::with(|g| g.win_arrow(slot, work, action)).flatten() {
        set_sliders(handle, sliders);
        redraw::do_wredraw(handle, work);
    }
}

/// `check_function_key` (EmuTOS `deskmain.c:1117-1128`) — F1-F20 launch the
/// installed application bound to that function key.
///
/// ```c
/// if ((thechar >= FUNKEY_01) && (thechar <= FUNKEY_10))
///     funkey = HIBYTE(thechar-FUNKEY_01) + 1;
/// else if ((thechar >= FUNKEY_11) && (thechar <= FUNKEY_20))
///     funkey = HIBYTE(thechar-FUNKEY_11) + 11;
/// else return -1;
/// return process_funkey(funkey);
/// ```
///
/// EmuTOS's three answers are `-1` (not a function key, or no application
/// holds it), `0` (a non-graphics program was launched) and `1` (a graphics
/// one) — here [`None`] for the `-1` and `Some(exit_desktop)` otherwise.
fn check_function_key(key: i16) -> Option<bool> {
    let funkey = funkey_number(key)?; // must be F1-F20, else fall through
    process_funkey(funkey)
}

/// `process_funkey` (EmuTOS `deskmain.c:1087-1106`) — find the installed
/// application that holds function key `funkey` and open it.
///
/// ```c
/// for (pa = G.g_ahead; pa; pa = pa->a_next)
///     if (pa->a_funkey == funkey) break;
/// if (pa) {
///     pfname = filename_start(pa->a_pappl);
///     strlcpy(pathname,pa->a_pappl,pfname-pa->a_pappl+1);
///     return do_aopen(pa,TRUE,-1,pathname,pfname,NULL);
/// }
/// return -1;
/// ```
///
/// The `ANODE` search is [`super::inf::app_by_funkey`]. `pathname` is
/// `a_pappl` truncated at the file name, i.e. the directory including its
/// trailing separator, and `pfname` the file name — the two halves
/// [`do_aopen_drop`] rejoins.
///
/// `do_aopen` is reached with `isapp = TRUE` and `tail = NULL`, so the
/// `installed_datafile` arm (`desksupp.c:924`) is ruled out by `isapp` and
/// there is no document tail; what is left is the directory step, the rejoin
/// and the launch — **including** `opn_appl`'s parameter prompt, because
/// `tail == NULL` is exactly the condition under which `isparm` is asked
/// (`:969-985`). A `.NTP` bound to a function key therefore gets the same box
/// a double click gives it.
///
/// `curr` is `-1` in this call, which in EmuTOS only suppresses `pro_run`'s
/// `do_wopen` re-open of the desktop window (`deskpro.c:61-62`); NewTOS's
/// `pro_run` carries neither parameter (`pro.rs`), so there is nothing to
/// suppress.
fn process_funkey(funkey: u16) -> Option<bool> {
    let pa = super::inf::app_by_funkey(funkey as u8)?;
    let pname = filename_of(&pa.app_path);
    let dir = &pa.app_path[..pa.app_path.rfind('\\').map_or(0, |i| i + 1)];
    // `do_aopen(pa, TRUE, -1, pathname, pfname, NULL)` — an **installed**
    // application, so the directory comes from its own `AF_APPDIR`/`a_pappl`;
    // `NULL` (not an empty tail) is what sends a parameter-taking program
    // through `opn_appl`.
    Some(do_aopen_drop(true, &pa.app_path, pa.app_dir, dir, &pname, None, -1))
}

/// The function-key number 1..20 for `key`, or `None` if it is not a function
/// key. F1-F10 = scancodes `0x3B..0x44`, F11-F20 = `0x54..0x5D`
/// (`scancode.h` `FUNKEY_01`/`FUNKEY_10`/`FUNKEY_11`/`FUNKEY_20`).
fn funkey_number(key: i16) -> Option<u16> {
    let sc = ((key as u16) >> 8) & 0xFF;
    match sc {
        0x3B..=0x44 => Some(sc - 0x3B + 1),
        0x54..=0x5D => Some(sc - 0x54 + 11),
        _ => None,
    }
}

/// The structure `Keytbl`'s return value points at, as EmuTOS's desktop
/// declares it for itself (`deskmain.c:64-69`):
///
/// ```c
/// typedef struct {
///     UBYTE *normal;
///     UBYTE *shift;
///     UBYTE *caps;
/// } KEYTAB;
/// ```
///
/// The first three fields of the BIOS's `struct keytbl` (`bios/ikbd.h:57-86`);
/// the desktop needs no more of it. The XBIOS lays the image out at the
/// **caller's** pointer width (`tos-api/src/xbios/keytbl.rs`, `write_area`),
/// which for the desktop's kernel-boot context is LP64 — this `#[repr(C)]`
/// struct of three raw pointers.
#[repr(C)]
struct Keytab {
    normal: *const u8,
    shift: *const u8,
    caps: *const u8,
}

/// `check_alt_letter_key` (EmuTOS `deskmain.c:1136-1159`) — Alt+A..Z opens that
/// drive letter:
///
/// ```c
/// if (LOBYTE(thechar)) return FALSE;      /* not an alt key */
/// keytab = (KEYTAB *)Keytbl(-1, -1, -1);
/// drive = keytab->shift[HIBYTE(thechar)];
/// if ((drive < 'A') || (drive > 'Z')) return FALSE;
/// if (!valid_drive(drive)) return FALSE;
/// do_dopen(-drive); men_update(); return TRUE;
/// ```
///
/// The shifted character of the scancode *is* the letter — there is no
/// dedicated Alt table, in EmuTOS or here — and the desktop reads it through
/// the XBIOS, not through a kernel symbol: [`super::xbiosbind::keytbl_query`].
///
/// **This path is live** (re-established DESK13). An older note here, and the
/// one DESK11 copied into its report, called it inert on the grounds that the
/// keyboard layer "folds Alt into AltGr and never yields ASCII 0 for a letter
/// key". That is not what the layer does: the translator zeroes an alphabetic
/// result whenever the Alt modifier is set (`kernel/src/keymap.rs`, the
/// `if ascii.is_ascii_alphabetic()` arm of the `alt` branch), `ikbd::to_ascii`
/// turns that `0` into `None`, and `ikbd::decode_key` queues the word as
/// `scancode << 8 | 0` — exactly the bare-scancode shape this check wants.
/// Both key sources set the Alt bit: the native IKBD stream (`SC_ALT` = `0x38`)
/// and the USB-HID report, whose `modifiers()` maps LAlt **and** RAlt to
/// `0x38`. The AES hands the whole word through as `EV_KRET`
/// (`tos-api/src/aes/event.rs:598`), so nothing between the key and here
/// re-interprets it.
///
/// Returns `true` only if it actually opened a drive.
fn check_alt_letter_key(key: i16) -> bool {
    if (key & 0xFF) != 0 {
        return false;
    }
    let sc = ((key >> 8) & 0xFF) as u8;
    // `keytab = (KEYTAB *)Keytbl(-1, -1, -1);` (`:1144`). A 0 return means the
    // board has no keyboard translator to describe and must not be
    // dereferenced (`docs/api/xbios/10-keytbl.md`); EmuTOS cannot reach that
    // case, so there is no behaviour of its to match — the shortcut simply has
    // no letter to look up.
    let p = super::xbiosbind::keytbl_query();
    if p == 0 {
        return false;
    }
    // `drive = keytab->shift[HIBYTE(thechar)];` (`:1145`).
    // SAFETY: `p` is the `struct keytbl` image the XBIOS just wrote, at this
    // caller's own pointer width and in kernel-addressable memory (see
    // [`Keytab`] and `keytbl_query`'s doc). `shift` points at that image's
    // 128-byte `shft` table, and `sc` is a `u8` masked out of the key word,
    // i.e. < 128 for every scancode the IKBD produces — the two scancodes
    // ≥ 0x80 are key *releases*, which the AES never forwards as `EV_KRET`.
    //
    // The one case where `shift` would *not* point into that image is a
    // program having installed its own table: `Keytbl` then reports the
    // caller's pointer verbatim, as EmuTOS does (`bios/ikbd.c:126`), and in
    // EmuTOS's flat address space that stays readable. Ours would be an EL0 VA
    // read from EL1 — the address-space difference DESK43 names as the cost of
    // the move, and U9's business, not this unit's. It is unreachable in this
    // tree: every `Keytbl` caller in `sdk/` passes the `(void *)-1` sentinel
    // (`sdk/cflib/cflib-21/nkccinit.c:97-99`, `sdk/apps/apitest/apitest.c:1474`,
    // `sdk/apps/teradesk/library/xdialog/xdevent.c:76`), so `OVERRIDE` is never
    // assigned.
    let letter = unsafe {
        let tab = &*(p as usize as *const Keytab);
        if sc >= 128 {
            return false;
        }
        *tab.shift.add(sc as usize)
    };
    if letter.is_ascii_uppercase() {
        let drive = letter - b'A';
        // `if (dos_sdrv(dos_gdrv()) & (1L << drv))` — EmuTOS `valid_drive`
        // (`desksupp.c:1819`), the same GEMDOS drive-map read as `app_blddesk`.
        if dos_sdrv(dos_gdrv()) & (1 << drive) != 0 {
            // `do_dopen(-drive);  /* -ve indicates drive letter rather than
            // obid */` (`:1157`).
            do_dopen(-(letter as i16));
            return true;
        }
    }
    false
}

/// `valid_drive` (EmuTOS `desksupp.c:1810-1826`) — is drive letter `drive`
/// present?
///
/// ```c
/// if ((drv >= 0) && (drv < BLKDEVNUM))
///     if (dos_sdrv(dos_gdrv()) & (1L << drv))
///         return TRUE;
/// fun_alert_merge(1, STNODRIV, drvstr);
/// return FALSE;
/// ```
///
/// The alert is part of the function, not of its callers: a drive icon that no
/// longer has a drive behind it says so (`[1][Drive X: does not exist.]
/// [ Cancel ]`, `desk_rsc.c:2541`) instead of failing silently.
///
/// [`check_alt_letter_key`] keeps its own inline drive-map test: EmuTOS filters
/// out impossible letters *before* calling `valid_drive`, "otherwise
/// valid_drive() will issue an (ugly) error message" (`deskmain.c:1147-1152`),
/// and that filtering is a separate piece of work from DESK38.
pub(super) fn valid_drive(drive: u8) -> bool {
    /// `BLKDEVNUM` (`bdosdefs.h`) — the GEMDOS drive-map width.
    const BLKDEVNUM: u32 = 16;
    let drv = drive.wrapping_sub(b'A') as u32;
    if drv < BLKDEVNUM && dos_sdrv(dos_gdrv()) & (1 << drv) != 0 {
        return true;
    }
    // `fun_alert_merge(1, STNODRIV, drvstr)` — the `%s` is the bare letter.
    let mut text = alloc::vec::Vec::with_capacity(48);
    text.extend_from_slice(b"[1][Drive ");
    text.push(drive);
    text.extend_from_slice(b": does not exist.][ Cancel ]\0");
    super::form_alert(1, text.as_ptr());
    false
}

// ── Menu dispatch (D6 wiring) ──────────────────────────────────────────────

/// `hndl_menu` (EmuTOS `deskmain.c:1012`) — route a selected [`Item`] to its
/// `do_*menu`, feed a View change through `desk_all`, and un-highlight the
/// title. Shared by the mouse (`MN_SELECTED`) and keyboard-shortcut paths.
/// Returns the loop-exit flag.
fn hndl_menu(item: Item, title_obj: i16) -> bool {
    let done = match menu::item_title(item) {
        Title::Desk => {
            do_deskmenu(item);
            false
        }
        Title::File => do_filemenu(item),
        Title::View => {
            let bits = do_viewmenu(item);
            if bits != 0 {
                desk_all(bits);
            }
            false
        }
        Title::Options => do_optnmenu(item),
    };
    super::menu_tnormal(menu::tree_ptr(), title_obj, true);
    done
}

/// `do_deskmenu` (EmuTOS `deskmain.c:612-651`) — the Desk menu. Its one case is
/// `ABOUITEM`, the *Desktop info...* box ([`super::inf::do_aboutdesk`]); every
/// other Desk entry is a registered desk accessory, and the AES sends those an
/// `AC_OPEN` of its own rather than routing them here, so the switch has no
/// other arm (`deskmain.c:617-648`).
///
/// Until DESK01 a `form_alert` stood in for the resource dialog. It is gone: an
/// alert is a different object (the AES's own `DIALERT` tree, its own
/// `bb_save`/restore, no dialog rectangle), and `do_aboutdesk` now goes through
/// the same `start_dialog`/`form_do`/`end_dialog` chain as every other desktop
/// dialog — which is what makes `form_dial(FMD_FINISH)` take the box down by
/// repainting exactly its own rectangle.
fn do_deskmenu(item: Item) {
    // One case, as in the source; no other Desk-menu item exists, because the
    // accessories below the separator are opened by the AES with `AC_OPEN`.
    if item == Item::About {
        super::inf::do_aboutdesk();
    }
}

/// `do_filemenu` (EmuTOS `deskmain.c:654`) — the File menu. Task-#8 items:
/// Open, Close folder, Close top window, Cycle windows. The rest are Task-#9
/// ops kept disabled by `men_update`. Returns the loop-exit flag — `CLIITEM`
/// is the one item that sets it (`deskmain.c:753`).
fn do_filemenu(item: Item) -> bool {
    let mut done = false;
    match item {
        Item::Open => done = do_open_selected(),
        Item::CloseFolder => do_close_folder(),
        Item::CloseWindow => do_close_window(),
        Item::Cycle => do_cycle(),
        Item::Delete => {
            dir::fun_del();
        }
        Item::SelectAll => {
            dir::fun_selectall();
        }
        Item::SetMask => do_set_mask(),
        Item::Search => do_search(),
        Item::NewFolder => do_new_folder(),
        Item::InfoRename => do_showitem(),
        Item::ExecEmuCon => done = do_execcli(),
        _ => {} // remaining File items have no live handler yet
    }
    done
}

/// `#if WITH_CLI case CLIITEM:` (EmuTOS `deskmain.c:750-770`) — File → Execute
/// EmuCON, "Start EmuCON":
///
/// ```c
/// G.g_work[1] = '\0';
/// done = pro_run(FALSE, DEF_CONSOLE, G.g_work, -1, -1);
/// if (done && pw)
/// {
/// #if CONF_WITH_PCGEM
///     char *p;
///     strcpy(G.g_work, pw->w_pnode.p_spec);
///     p = filename_start(G.g_work);
///     *p = '\0';
///     shel_wdef("", G.g_work);
/// #endif
/// }
/// ```
///
/// `isgraf = FALSE` is what makes the AES switch the screen to **alpha** mode
/// for the console (`sh_write` records it as `sh_isgem = 0`,
/// `gemshlib.c:173`; `sh_chgrf` acts on it, `:513-523`): the console owns the
/// whole screen, exactly like a launched application, and no text window is
/// opened inside the GEM screen — EmuTOS has no such thing.
///
/// `pw` is the top window (`win_ontop()`, `deskmain.c:661`); ours comes from
/// `wind_get(DESKWH, WF_TOP)` as everywhere else in this file. Its search spec
/// (`C:\FOO\*.*`) minus the filename part is the directory EmuCON is to start
/// in; `sh_chdef`'s `CONSOLE_APP` arm is what applies it
/// (`gemshlib.c:545-549`).
///
/// Returns `done` for the caller. Since DESK43 U3 that flag does what it does
/// in EmuTOS: it ends the desktop, and the console is started by
/// [`crate::shell_loop::sh_main`]'s next pass through its `CONSOLE_APP` arm
/// (`gemshlib.c:614-622`) — `sh_chgrf` having taken the screen to alpha mode
/// first, because `isgraf = FALSE` recorded `sh_isgem = 0` (`:173,513-523`).
fn do_execcli() -> bool {
    // `G.g_work[1] = '\0';` — an empty command tail. Byte 0 is the length
    // byte `pro_run` fills in, byte 1 the first text byte.
    let mut tail = [0u8; pro::CMDTAILSIZE];
    // `pro_run(FALSE, DEF_CONSOLE, G.g_work, -1, -1)` (`deskmain.c:753`) —
    // `wh = -1`: no icon to deselect, no zoom.
    let done = pro::pro_run(false, pro::DEF_CONSOLE, &mut tail, -1, -1);

    if done {
        // `if (done && pw)` — `#if CONF_WITH_PCGEM`, which the Falcon030/TOS4
        // profile builds (`include/config.h`), so the step is ours too.
        let pw = wind_get_top();
        if pw != DESKWH {
            if let Some(spec) =
                global::with(|g| g.win_slot(pw).map(|s| g.windows.slots[s].w_pnode.p_spec.clone()))
                    .flatten()
            {
                // `filename_start(G.g_work); *p = '\0';` — cut after the last
                // path separator, keeping it (`C:\FOO\*.*` → `C:\FOO\`); a
                // spec with no separator at all becomes empty, as in EmuTOS.
                let cut = spec.rfind('\\').map_or(0, |i| i + 1);
                let mut dir = String::from(&spec[..cut]);
                dir.push('\0');
                // `shel_wdef("", G.g_work)` — the empty program name clears
                // `sh_desk`; `set_default_desktop` puts the built-in desktop
                // back into it when the console ends (`gemshlib.c:583-589`).
                let empty = [0u8; 1];
                super::shel_wdef(empty.as_ptr(), dir.as_ptr());
            }
        }
    }
    done
}

/// `do_optnmenu` (EmuTOS `deskmain.c:836`) — the Options menu. Live items: Set
/// background (`inf_backgrounds`), Install Application (deskins #20), Read .INF
/// (which restarts the desktop, `:918-925`), Save desktop (`app_save`). The
/// remaining preferences / resolution items stay disabled by `men_update`
/// (deskrez is config.txt-driven, ADR-017 rev 2).
///
/// Returns `done` (`WORD do_optnmenu(WORD item)`, `:836`) — `READITEM` is the
/// one live item that sets it.
fn do_optnmenu(item: Item) -> bool {
    let mut done = false;
    match item {
        Item::InstallApp => do_install_app(),
        // `case PREFITEM: if (inf_pref()) desk_all(FALSE);` (`deskmain.c:885-
        // 887`). **Named deviation in the argument**: EmuTOS passes `FALSE`
        // (no flags), because its text lines are formatted *at draw time* by
        // the `G_USERDEF` `dr_fnode` (`deskinf.c:310-326`), so repainting is
        // enough to pick a new date format up. NewTOS builds the line once, in
        // `win_bldview` → `format_fnode`, and caches it in the object's label —
        // so the windows have to be **rebuilt**, which is what `VIEW_CHANGED`
        // asks `desk_all` for. `inf_pref` already returns `true` only when a
        // rebuild is actually needed (text view + a format change).
        Item::SetPrefs => {
            let rebuild = super::inf::inf_pref();
            if rebuild {
                desk_all(VIEW_CHANGED);
            }
        }
        // `case IDSKITEM: rebld = ins_devices(); if (rebld) app_blddesk();`
        // (`deskmain.c:847-852`).
        Item::InstallDevices => {
            let rebld = super::ins::ins_devices();
            if rebld {
                rebuild_desktop_icons();
            }
        }
        // `case IICNITEM: rebld = ins_icon(curr); if (rebld > 0) app_blddesk();
        //  else if (rebld < 0) { win_bdall(); win_shwall(); }`
        // (`deskmain.c:863-876`). `curr` is the first selected item of the
        // current root, taken at the top of `do_optnmenu` (`:842`). The
        // negative arm is the `CONF_WITH_WINDOW_ICONS` one: a window icon was
        // (re)assigned, so every open window's view has to be rebuilt to pick
        // the new `ANODE` up through `win_icalc` (DESK15).
        Item::InstallIcon => {
            let rebld = super::ins::ins_icon(first_selected());
            if rebld > 0 {
                rebuild_desktop_icons();
            } else if rebld < 0 {
                win_bdall();
                redraw::win_shwall();
            }
        }
        // `case RICNITEM: if (curr) rebld = rmv_icon(curr); if (rebld)
        //  app_blddesk();` (`deskmain.c:875-881`).
        Item::RemoveIcon => {
            let curr = first_selected();
            if curr != 0 && super::ins::rmv_icon(curr) != 0 {
                rebuild_desktop_icons();
            }
        }
        // `#if CONF_WITH_DESKTOP_CONFIG case CONFITEM: inf_conf(); break;`
        // (`deskmain.c:895-898`) — no rebuild signal and no repaint: the dialog
        // edits `menu_shortcuts[]` and the menu strings, neither of which is on
        // screen while a drop-down is closed, and `end_dialog`'s
        // `form_dial(FMD_FINISH)` restores what the dialog covered (DESK05).
        Item::DeskConfig => super::inf::inf_conf(),
        Item::ReadInf => done = do_read_inf(),
        Item::SaveDesktop => do_save_desktop(),
        _ => {} // remaining Options items stay disabled
    }
    done
}

/// `curr = win_isel(G.g_screen, G.g_croot, 0);` — the first selected item of the
/// current window's root, the value `do_optnmenu` computes before its switch
/// (`deskmain.c:842`). `0` when nothing is selected.
fn first_selected() -> i16 {
    global::with(|g| g.win_isel(g.g_croot, 0)).unwrap_or(0)
}

/// `app_blddesk()` plus the repaint EmuTOS folds into it (`deskapp.c:1402`,
/// `do_wredraw(DESKWH, &G.g_desk)`) — rebuild the desktop icon tree from
/// `G.g_ahead` and repaint the work area, which is what an icon that appeared,
/// moved or vanished needs.
pub(super) fn rebuild_desktop_icons() {
    let desk = global::with(|g| {
        g.app_blddesk();
        g.g_desk
    })
    .unwrap_or_default();
    redraw::do_wredraw(DESKWH, desk);
    // The selection is gone with the objects that carried it, so the Remove /
    // Open gating has to be recomputed (`men_update`, the D6 machinery).
    menu::men_update();
}

/// `ins_app` (EmuTOS `deskins.c:721-756`) — Options → Install Application.
/// Resolves the single selected native program in the current window, runs the
/// [`super::insapp`] dialog pre-filled from any existing association, and commits
/// the result into the [`super::inf`] `NEWDESK.INF` association model. Like
/// EmuTOS, the change lives in memory until the user does Save Desktop (which
/// persists every association into `NEWDESK.INF`); the menu item is only enabled
/// when a native program is selected (`men_update`).
fn do_install_app() {
    use super::insapp::Outcome;
    let Some((app_path, app_name, flavor)) = selected_native_program() else {
        return; // menu gating should have prevented this
    };

    // Pre-fill the dialog from an existing association for this program, if any.
    let existing = super::inf::installed_doctype(&app_path);
    let installed = existing.is_some();
    let (init_ext, init_flavor, init_full, init_funkey) = match existing {
        Some(a) => (a.doc_ext, a.flavor, a.full_path, a.funkey),
        None => (String::new(), default_flavor(flavor), false, 0),
    };

    let outcome = super::insapp::ins_app_dialog(
        &app_name,
        &app_path,
        &init_ext,
        init_flavor,
        init_full,
        init_funkey,
        installed,
    );
    match outcome {
        Outcome::Install { doc_ext, flavor: fl, full_path, funkey } => {
            super::inf::install_application(&app_path, &doc_ext, fl, full_path, false, funkey, "");
        }
        Outcome::Remove => {
            super::inf::remove_application(&app_path);
        }
        Outcome::Cancel => {}
    }
    // The dialog took its own rectangle down (`end_dialog`); nothing else on
    // screen changed.
}

/// The default Install-Application flavour for a native program, from its
/// extension: `.NAP` (GEM app) → GEM, `.NCP` (console) → TOS, `.NTP`
/// (takes-parameters) → TTP. The user can change it in the dialog.
fn default_flavor(flavor: ftype::Flavor) -> super::inf::AppFlavor {
    match flavor {
        ftype::Flavor::App => super::inf::AppFlavor::Gem,
        ftype::Flavor::Console => super::inf::AppFlavor::Tos,
        ftype::Flavor::Params => super::inf::AppFlavor::Ttp,
        // A `.NAC` is never an *installed application* — it is not launched at
        // all (see [`pro_run_native`]) — but the enum must be total. GEM is the
        // nearest description of what the file is; the Install-Application
        // dialog is unreachable for it, since [`selected_native_program`] only
        // offers a program the desktop would run.
        ftype::Flavor::Accessory => super::inf::AppFlavor::Gem,
    }
}

/// The single selected native program in the current window as
/// `(full_path, file_name, flavour)` — the target of Install Application. Walks
/// the current item-root for the first selected object that resolves to a native
/// (`.N_P`) program `FNODE`. [`None`] on the desktop, with no selection, or when
/// the selection is not a native program.
fn selected_native_program() -> Option<(String, String, ftype::Flavor)> {
    global::with(|g| {
        let root = g.g_croot;
        let handle = g.g_cwin;
        if handle == DESKWH {
            return None;
        }
        let slot = g.win_slot(handle)?;
        let mut child = g.g_screen[root as usize].ob_head;
        while child != NIL && child != root {
            if g.g_screen[child as usize].ob_state & SELECTED != 0 {
                let fnode = g.g_screeninfo[child as usize].fnode;
                if fnode >= 0 {
                    let pn = &g.windows.slots[slot].w_pnode;
                    if let Some(f) = pn.p_flist.get(fnode as usize) {
                        if !f.is_dir() {
                            if let ftype::FileType::Native(flavor) = ftype::classify(f.name_bytes()) {
                                let name = String::from(core::str::from_utf8(f.name_bytes()).unwrap_or(""));
                                let full = doc_full_path(&pn.p_spec, f.name_bytes());
                                return Some((full, name, flavor));
                            }
                        }
                    }
                }
            }
            child = g.g_screen[child as usize].ob_next;
        }
        None
    })
    .flatten()
}

/// `case READITEM:` of `do_optnmenu` (`deskmain.c:918-925`, `#if
/// CONF_WITH_READ_INF`) — *Options → Read .INF file*:
///
/// ```c
/// if (app_read_inf())
/// {
///     restart_desktop = TRUE;
///     done = TRUE;
/// }
/// ```
///
/// [`super::inf::app_read_inf`] prompts, loads, validates and — since DESK43 U3
/// — merges the file into the AES shell buffer (`deskapp.c:1530-1534`). The two
/// flags then end this desktop: `done` takes the event loop out, `restart_
/// desktop` picks the epilog's free-everything arm (`:2136-2150`), and
/// [`super::run`] re-enters [`super::deskmain`] immediately, which reads the
/// new file back through `app_start` exactly the way the boot path reads one.
///
/// **The DESK14 §4 deviation ("no desktop restart", lead decision 2026-09-23)
/// is hereby retired**, together with the in-place re-apply that stood in for
/// it (`cnx_get_prefs` + `install_shortcuts` + `win_view`/`app_blddesk`/
/// `win_srtall` + `win_bdall` + `redraw_desktop`): the restart does all of it,
/// from `deskmain`'s own prolog.
fn do_read_inf() -> bool {
    if !super::inf::app_read_inf() {
        return false;
    }
    RESTART_DESKTOP.store(true, core::sync::atomic::Ordering::Relaxed);
    true
}

/// Options → Save desktop: snapshot the session context, then serialise it to
/// `NEWDESK.INF` — EmuTOS `SAVEITEM`: `desk_busy_on(); cnx_put(); app_save(TRUE);
/// desk_busy_off();` (`deskmain.c:888-893`). Reports failure through an alert.
fn do_save_desktop() {
    cnx_put();
    if !super::inf::app_save(true) {
        dir::fun_alert(1, b"Could not save the desktop file.");
        // AES `form_alert` (fun_alert) bb_save/restores the screen — no repaint.
    }
}

/// `fun_mkdir` (EmuTOS `deskfun.c:680`) — File → New folder: prompt for a name,
/// create it (`Dcreate`) in the current window's directory, and rebuild that
/// window. The name-entry box is [`super::dlg::edit_name`]; the deeply-nested
/// path-length guard is `Dcreate`'s own `EPTHNF`.
fn do_new_folder() {
    let cwin = global::with(|g| g.g_cwin).unwrap_or(DESKWH);
    if cwin == DESKWH {
        return;
    }
    let Some(name) = super::dlg::edit_name(b"Name of new folder:\0", "") else {
        return; // cancelled — `end_dialog` already restored the screen
    };
    let created = global::with(|g| {
        g.win_slot(cwin).and_then(|s| {
            let spec = &g.windows.slots[s].w_pnode.p_spec;
            spec.rfind('\\').map(|i| String::from(&spec[..=i]))
        })
    })
    .flatten();
    if let Some(dir_path) = created {
        let mut full = dir_path;
        full.push_str(name.trim());
        if dir::mkdir(&full) {
            dir::rebuild_window(cwin);
            return;
        }
        dir::fun_alert(1, b"Could not create the folder.");
    }
}

/// `case SHOWITEM:` of `do_filemenu` (EmuTOS `deskmain.c:674-689`) — File →
/// Info/Rename.
///
/// ```c
/// if (curr) {
///     for ( ; curr; curr = win_isel(G.g_screen, G.g_croot, curr))
///         if (!do_info(curr)) break;
///     fun_rebld_marked();     /* rebuild any changed windows */
///     break;
/// }
/// /* no highlighted icons: display info for the disk of the top window */
/// if (pw) inf_disk(pw->w_pnode.p_spec[0]);
/// ```
///
/// Two things DESK38 restored here. The **loop**: a *Show info* on three
/// selected items puts three boxes up in turn, and *Cancel* on one of them ends
/// the series (that is what `do_info`'s `0` means). And the **no-selection
/// case**: with nothing highlighted, the top window's drive gets the
/// [`super::inf::inf_disk`] box, "just like TOS" (`:683-684`).
///
/// [`dir::fun_rebld_marked`] runs **after** the loop, not inside `do_info`,
/// because rebuilding a window throws away the very `FNODE`s the loop is
/// walking (`deskfun.c:119-129` and its caller here).
fn do_showitem() {
    // `curr = win_isel(G.g_screen, G.g_croot, 0);` (`deskmain.c:668`).
    let mut curr = first_selected();
    if curr != 0 {
        while curr != 0 {
            if do_info(curr) == 0 {
                break; // Cancel — stop walking the selection
            }
            curr = global::with(|g| g.win_isel(g.g_croot, curr)).unwrap_or(0);
        }
        dir::fun_rebld_marked();
        return;
    }
    // `if (pw) inf_disk(pw->w_pnode.p_spec[0]);` — `win_ontop()`, i.e. the
    // topped **window**; the desktop is not one, so nothing happens there.
    let pw = wind_get_top();
    if pw != DESKWH {
        let letter = global::with(|g| {
            let slot = g.win_slot(pw)?;
            g.windows.slots[slot].w_pnode.p_spec.as_bytes().first().copied()
        })
        .flatten();
        if let Some(letter) = letter {
            super::inf::inf_disk(letter);
        }
    }
}

/// `do_info` (EmuTOS `desksupp.c:1288-1354`) — the *Show info* box for one
/// icon, chosen by its type:
///
/// ```c
/// pa = i_find(G.g_cwin, curr, &pf, NULL);
/// if (!pa) return ret;
/// switch(pa->a_type) {
/// case AT_ISFOLD: case AT_ISFILE:  … inf_file_folder(pathptr, pf) …  break;
/// case AT_ISDISK: drive = LOBYTE(get_iconblk_ptr(G.g_screen,curr)->ib_char);
///                 ret = inf_disk(drive); break;
/// case AT_ISTRSH: trash_alert(pa); break;
/// case AT_ISPRNT: printer_alert(pa); break;
/// }
/// return ret;
/// ```
///
/// Returns EmuTOS's `1` (continue) / `0` (cancel) / `-1` (continue, and a
/// window was marked for rebuild). Only `0` stops [`do_showitem`]'s loop.
///
/// **DESK38.** The predecessor of this function returned immediately for
/// `G.g_cwin == DESKWH`, so *Show info* did not exist on the desktop surface at
/// all — not for a drive, not for the trash, not for the printer, and since
/// DESK30 not for a shortcut either. Every one of those four is a case of this
/// switch.
fn do_info(curr: i16) -> i16 {
    let cwin = global::with(|g| g.g_cwin).unwrap_or(DESKWH);
    // `pa = i_find(G.g_cwin, curr, &pf, NULL)` — the governing `ANODE`'s type
    // is all this switch reads off it.
    let Some((atype, _isapp)) = dir::i_find_atype(cwin, curr) else {
        return 1; // `if (!pa) return ret;`
    };
    match atype {
        AType::Folder | AType::File => do_info_file_folder(cwin, curr),
        AType::Disk => {
            // `drive = LOBYTE(get_iconblk_ptr(G.g_screen, curr)->ib_char)` —
            // the drive letter drawn into the icon's bitmap.
            let letter = global::with(|g| {
                g.g_screeninfo.get(curr as usize).map(|i| (i.block.ib_char & 0xFF) as u8)
            })
            .flatten()
            .unwrap_or(0);
            super::inf::inf_disk(letter)
        }
        AType::Trash => {
            trash_alert(curr);
            1
        }
        AType::Printer => {
            printer_alert(curr);
            1
        }
        AType::None => 1,
    }
}

/// `do_info`'s `AT_ISFOLD`/`AT_ISFILE` arm (`desksupp.c:1306-1339`).
///
/// Two sources for the same box. A **window** item takes its fields from the
/// `FNODE` the object carries and its directory from the window's `p_spec`;
/// a **desktop shortcut** (`pa->a_flags & AF_ISDESK`, which every
/// [`global::Anode`] is) has no `FNODE`, so EmuTOS probes the file it points at
/// — `file_exists(pa->a_pappl, NULL)` — and builds one on its stack from the
/// DTA (`memcpy(&pf->f_attr, &dta->d_attrib, 23)`). A shortcut whose file is
/// gone goes to `remove_locate_shortcut` instead of showing a box, exactly as
/// opening it would (`:1320-1324`).
fn do_info_file_folder(cwin: i16, curr: i16) -> i16 {
    if cwin == DESKWH {
        // `win_find(DESKWH)` is the desktop pseudo-window and never NULL
        // (`deskwin.c:202-203`), so the `if (pw)` is always taken here.
        let path = global::with(|g| g.app_afind_by_id(curr).map(|i| g.g_ahead[i].a_pappl.clone()))
            .flatten();
        let Some(path) = path else {
            return 1;
        };
        let Some(mut info) = dir::file_exists(&path) else {
            dir::remove_locate_shortcut(curr);
            return 1; // `break` — `ret` is still 1
        };
        // `strcpy(pathname, pa->a_pappl); del_fname(pathname);`
        let dir_path = String::from(&path[..path.rfind('\\').map_or(0, |i| i + 1)]);
        return show_item_info(&dir_path, &mut info);
    }
    // `pathptr = pw->w_pnode.p_spec; pf = G.g_screeninfo[item].fnptr;`
    let sel = global::with(|g| {
        let slot = g.win_slot(cwin)?;
        let spec = &g.windows.slots[slot].w_pnode.p_spec;
        let dir = spec.rfind('\\').map(|i| String::from(&spec[..=i]))?;
        let fnode = g.g_screeninfo.get(curr as usize)?.fnode;
        if fnode < 0 {
            return None;
        }
        let f = g.windows.slots[slot].w_pnode.p_flist.get(fnode as usize)?;
        let name = core::str::from_utf8(f.name_bytes()).ok().map(String::from)?;
        // The `FNODE` fields `inf_file_folder` reads, in the shape
        // `desksupp.c:1326` gives its stack copy.
        let info = dir::FileInfo {
            name,
            attr: f.f_attr,
            time: f.f_time,
            date: f.f_date,
            size: f.f_size,
        };
        Some((dir, info))
    })
    .flatten();
    let Some((dir_path, mut info)) = sel else {
        return 1;
    };
    show_item_info(&dir_path, &mut info)
}

/// `ret = inf_file_folder(pathptr, pf); if (ret < 0) fun_mark_for_rebld(pathptr);`
/// (`desksupp.c:1335-1337`) — run the box and act on its answer.
///
/// Since DESK42 the rename, the `STRENAME` *Retry* loop and the attribute
/// write-back all live inside [`super::inf::inf_file_folder`], where EmuTOS has
/// them; what is left here is the source's two lines. The deferral is the
/// point of `fun_mark_for_rebld`: rebuilding the window at once would throw
/// away the `FNODE` list the `SHOWITEM` loop is still walking.
///
/// **Named deviation.** EmuTOS's `pf` *is* the window's live `FNODE`, so a
/// changed name or attribute is visible in it immediately; ours is the copy
/// `do_info` made. The window is refreshed by the rebuild that the same `-1`
/// triggers ([`dir::fun_rebld_marked`], which re-scans the directory), so the
/// two agree by the time anything reads them again.
fn show_item_info(dir_path: &str, info: &mut dir::FileInfo) -> i16 {
    let ret = super::inf::inf_file_folder(dir_path, info);
    if ret < 0 {
        dir::fun_mark_for_rebld(dir_path);
    }
    ret
}

/// `trash_alert` (EmuTOS `desksupp.c:1165-1172`) — `STTRINFO`
/// (`desk_rsc.c:2517-2519`) with the trash icon's **current** name merged in,
/// which is why it is read from the `ANODE` and not from a literal:
///
/// ```text
/// [1][%s is used to|permanently delete files|or folders.][  OK  ]
/// ```
fn trash_alert(curr: i16) {
    let name = anode_label(curr);
    let mut text = alloc::vec::Vec::with_capacity(96);
    text.extend_from_slice(b"[1][");
    text.extend_from_slice(name.as_bytes());
    text.extend_from_slice(b" is used to|permanently delete files|or folders.][  OK  ]\0");
    super::form_alert(1, text.as_ptr());
}

/// `printer_alert` (EmuTOS `desksupp.c:1177-1183`, `CONF_WITH_PRINTER_ICON`) —
/// `STPRINFO` (`desk_rsc.c:2520-2521`), the same shape:
///
/// ```text
/// [1][%s|is a printer.][  OK  ]
/// ```
fn printer_alert(curr: i16) {
    let name = anode_label(curr);
    let mut text = alloc::vec::Vec::with_capacity(64);
    text.extend_from_slice(b"[1][");
    text.extend_from_slice(name.as_bytes());
    text.extend_from_slice(b"|is a printer.][  OK  ]\0");
    super::form_alert(1, text.as_ptr());
}

/// `pa->a_pappl` of the desktop icon `curr` — its label for a drive, the trash
/// and the printer (`deskapp.h:88-90`, usage (1)).
fn anode_label(curr: i16) -> String {
    global::with(|g| g.app_afind_by_id(curr).map(|i| g.g_ahead[i].a_pappl.clone()))
        .flatten()
        .unwrap_or_default()
}

/// `fun_mask` (EmuTOS `deskfun.c:639`, `CONF_WITH_FILEMASK`) — File → Set file
/// mask: prompt for the current window's display filter (default `*.*`), store
/// it as the window's `p_mask`, re-scan + re-filter, and repaint. The mask
/// filters **files only** — folders always show (`PNode::pn_active`). The edit
/// field is [`super::dlg::edit_name`] (real keyboard only); the filter itself is
/// applied by [`set_window_mask`], which a headless selftest can call directly.
fn do_set_mask() {
    let cwin = global::with(|g| g.g_cwin).unwrap_or(DESKWH);
    if cwin == DESKWH {
        return;
    }
    // Current mask (default `*.*` when unset), shown in the dialog for editing.
    let cur = global::with(|g| {
        let slot = g.win_slot(cwin)?;
        let m = g.windows.slots[slot].w_pnode.p_mask.clone();
        Some(if m.is_empty() { String::from("*.*") } else { m })
    })
    .flatten()
    .unwrap_or_else(|| String::from("*.*"));

    let Some(entered) = super::dlg::edit_name(b"Show only files matching:\0", &cur) else {
        return; // cancelled — `end_dialog` restored the screen under the dialog
    };
    set_window_mask(cwin, &entered);
}

/// Apply file mask `raw` to window `handle`: normalise it to an upper-case 8.3
/// wildcard (empty → `*.*`), store it as the window's `p_mask`, rescan + refilter
/// the listing, update the title + info line, rebuild the view and repaint —
/// EmuTOS `fun_mask` → `refresh_window` → `do_fopen`/`do_diropen`. Factored out
/// of [`do_set_mask`] so the filter can be driven with a mask value directly
/// (the dialog's text entry needs a real keyboard).
fn set_window_mask(handle: i16, raw: &str) {
    let mask = mask_normalize(raw);
    let work = wind_get_work(handle);
    // Store the mask, rescan with it (files filtered, folders kept), re-sort.
    let scanned = global::with(|g| {
        let sort = g.sort;
        let slot = g.win_slot(handle)?;
        let spec = {
            let pn = &mut g.windows.slots[slot].w_pnode;
            pn.p_mask = mask.clone();
            pn.pn_active(true);
            pn.pn_sort(sort);
            pn.p_spec.clone()
        };
        let pn = &g.windows.slots[slot].w_pnode;
        Some((slot, spec, pn.p_count, pn.p_size))
    })
    .flatten();
    let Some((slot, spec, count, size)) = scanned else {
        return;
    };

    // Title: directory, plus the active mask when it is a real filter (EmuTOS
    // `win_sname` always shows the full spec; NewTOS keeps the clean path for
    // the unfiltered `*.*`).
    let dir = spec.rfind('\\').map(|i| String::from(&spec[..=i])).unwrap_or(spec);
    let mut title = dir;
    if !mask.is_empty() && mask != "*.*" {
        title.push_str(&mask);
    }
    // Info line reflects the filtered item count / size.
    set_window_strings(handle, slot, &title, count, size);

    if let Some(sliders) = global::with(|g| g.win_bldview(slot, work)) {
        set_sliders(handle, sliders);
    }
    redraw::redraw_desktop();
}

/// Normalise a raw mask to an upper-case TOS wildcard: trim, keep only the
/// filename part if a path separator was typed, and map empty → `*.*`.
fn mask_normalize(raw: &str) -> String {
    let last = raw.trim().rsplit(['\\', '/']).next().unwrap_or("").trim();
    if last.is_empty() {
        String::from("*.*")
    } else {
        last.to_ascii_uppercase()
    }
}

/// Store `title` and the ` N items   M bytes used` line in the [`WNode`] of
/// window slot `slot` and push both to the AES by **address** — `WF_NAME`
/// then `WF_INFO`, as EmuTOS's `do_diropen` does with `pw->w_name`
/// (`desksupp.c:451`, filled by `win_sname`, `deskwin.c:847-858`) and
/// `win_sinfo` with `pwin->w_info` (`deskwin.c:875-906`). The AES
/// keeps the pointer, not a copy (A83, `gemwmlib.c:1661,1669`), and reads it
/// at every frame build; the strings therefore live in the node — whose
/// address is stable for the desktop's lifetime ([`WNode::w_name`]) — and
/// never on this stack frame.
///
/// [`WNode`]: super::win::WNode
/// [`WNode::w_name`]: super::win::WNode::w_name
fn set_window_strings(handle: i16, slot: usize, title: &str, count: usize, size: u64) {
    let ptrs = global::with(|g| {
        let node = &mut g.windows.slots[slot];
        node.set_name(title);
        // `win_sinfo(pw, FALSE)` — the whole directory, `STINFOST`.
        fmt_info(&mut node.w_info, count, size, false);
        (node.name_ptr(), node.info_ptr())
    });
    if let Some((name, info)) = ptrs {
        super::wind_set_ptr(handle, WF_NAME, name);
        super::wind_set_ptr(handle, WF_INFO, info);
    }
}

/// `win_sinfo` (EmuTOS `desk/deskwin.c:867-906`) — rebuild window `handle`'s
/// info line from its file list and hand it back to the AES.
///
/// ```c
/// if (check_selected)
///     for (i = 0, fn = pn->p_fbase; i < pn->p_count; i++, fn++)
///         if (fn->f_selected) { select_count++; select_size += fn->f_size; }
/// if (select_count)
///     alert = desktop_str_addr(STINFST2);
/// else {
///     alert = desktop_str_addr(STINFOST);
///     select_count = pn->p_count;  select_size = pn->p_size;
/// }
/// sprintf(pwin->w_info, alert, select_size, select_count);
/// wind_set(pwin->w_id, WF_INFO, pwin->w_info, 0, 0);
/// ```
///
/// `check_selected == false` is the whole-directory line
/// ([`set_window_strings`], the `do_diropen`/`rebuild_window` path,
/// `desksupp.c:450`, `deskfun.c:141`); `true` is the selection line, which is
/// what every selection change runs — a selection of *nothing* falls back to
/// the directory totals through the same `select_count == 0` test, so
/// deselecting restores the byte count by itself.
///
/// The desktop is passed straight through, as in EmuTOS: `win_find(DESKWH)`
/// answers `&G.g_wdesktop` (`deskwin.c:202-203`), whose file list is empty, so
/// the count falls back to its zero totals and the `wind_set` lands on the
/// desktop window — which is `VF_INUSE` but never `VF_ISOPEN`, so it stores a
/// pointer and draws nothing (`wm_set`'s `if (pwin->w_flags & VF_ISOPEN)`,
/// `gemwmlib.c:1666-1672`). `desk_clear` guards the call anyway
/// (`desksupp.c:150-153`), `hndl_button` does not (`deskmain.c:982-983`).
pub(super) fn win_sinfo(handle: i16, check_selected: bool) {
    let info = global::with(|g| {
        let node = g.win_find(handle)?;
        let mut select_count = 0usize;
        let mut select_size = 0u64;
        if check_selected {
            for f in node.w_pnode.p_flist.iter().filter(|f| f.f_selected) {
                select_count += 1;
                select_size += u64::from(f.f_size);
            }
        }
        let selected = select_count != 0;
        if !selected {
            select_count = node.w_pnode.p_count;
            select_size = node.w_pnode.p_size;
        }
        fmt_info(&mut node.w_info, select_count, select_size, selected);
        Some((node.w_id, node.info_ptr()))
    })
    .flatten();
    if let Some((w_id, ptr)) = info {
        super::wind_set_ptr(w_id, WF_INFO, ptr);
    }
}

/// `desk_clear` (EmuTOS `desk/desksupp.c:121-153`) — deselect everything in
/// window `wh` and refresh its info line.
///
/// The state half is [`DeskState::desk_clear`] (the `act_allchg(…, FALSE)`
/// call, `:147`); the second half is `win_sinfo(pw, TRUE)` for anything but
/// the desktop (`:150-153`), which is how a deselect puts the directory's byte
/// count back in place of a selection count. Returns whether anything changed,
/// for the caller's repaint decision.
///
/// [`DeskState::desk_clear`]: super::global::DeskState::desk_clear
pub(super) fn desk_clear(wh: i16) -> bool {
    let changed = global::with(|g| g.desk_clear(wh)).unwrap_or(false);
    if wh != DESKWH {
        win_sinfo(wh, true);
    }
    changed
}

/// `fun_search` (EmuTOS `deskfun.c:560`, `CONF_WITH_SEARCH`) — File → Search:
/// prompt for a name, convert it to a TOS wildcard, and search the current
/// window. The name entry is [`super::dlg::edit_name`] (real keyboard only);
/// [`search_current_window`] does the work and can be driven headless.
///
/// **Scope:** the current window only — EmuTOS's `mark_matching_fnodes` path
/// (open window, no icon selected). The recursive descent through selected
/// disk/folder icons (`search_recursive`/`search_icon`, which opens a scratch
/// window per matching folder) is **not** implemented here — a documented
/// subset, not a stub: a name typed while a window is on top is found, selected,
/// and scrolled into view; nothing is faked.
fn do_search() {
    let cwin = global::with(|g| g.g_cwin).unwrap_or(DESKWH);
    if cwin == DESKWH {
        return;
    }
    let Some(name) = super::dlg::edit_name(b"Search for file:\0", "") else {
        return; // cancelled — `end_dialog` restored the screen under the dialog
    };
    search_current_window(cwin, &name);
}

/// Select every file in window `handle` matching search `name`
/// (`convert`ed to a wildcard, EmuTOS `deskfun.c:mark_matching_fnodes`), scroll
/// the first match to the top, and repaint; alert "file not found" when nothing
/// matches. Returns the match count. Factored out of [`do_search`] so the search
/// can be driven with a name directly (the dialog needs a real keyboard).
fn search_current_window(handle: i16, name: &str) -> usize {
    let wild = search_wildcard(name);
    let work = wind_get_work(handle);
    // Mark matches on the (already-scanned) file list; note the first match's
    // linear index so the view can scroll to it.
    let result = global::with(|g| {
        let slot = g.win_slot(handle)?;
        let mut count = 0usize;
        let mut first: Option<usize> = None;
        for (i, f) in g.windows.slots[slot].w_pnode.p_flist.iter_mut().enumerate() {
            if super::model::wildcmp(wild.as_bytes(), f.name_bytes()) {
                f.f_selected = true;
                count += 1;
                if first.is_none() {
                    first = Some(i);
                }
            }
        }
        if let Some(idx) = first {
            // Scroll so the first match sits at the top of the window; a bad
            // row is re-clamped inside `win_bldview`/`win_ocalc`.
            let vncol = g.windows.slots[slot].w_vncol.max(1) as usize;
            g.windows.slots[slot].w_cvrow = (idx / vncol) as i16;
            let sliders = g.win_bldview(slot, work);
            Some((count, Some(sliders)))
        } else {
            Some((count, None))
        }
    })
    .flatten()
    .unwrap_or((0, None));

    let (count, sliders) = result;
    if let Some(sliders) = sliders {
        // `win_sinfo(pw, TRUE); win_dispfile(pw, first_match);`
        // (`deskfun.c:380-382`) — the matches are now the selection.
        win_sinfo(handle, true);
        set_sliders(handle, sliders);
        redraw::redraw_desktop();
    } else {
        dir::fun_alert(1, b"No file matching the search was found.");
        redraw::redraw_desktop();
    }
    count
}

/// `convert` (EmuTOS `deskfun.c:264`) — turn a Search name into a TOS wildcard,
/// upper-cased: a partial name/extension gets a trailing `*` in that field
/// (`"A"` → `"A*.*"`, `".DOC"` → `"*.DOC"`, `"X?Y.?Z"` → `"X?Y*.?Z*"`).
fn search_wildcard(name: &str) -> String {
    let s = name.trim().to_ascii_uppercase();
    let b = s.as_bytes();
    let at = |i: usize| -> u8 { b.get(i).copied().unwrap_or(0) };
    let mut i = 0usize;
    let mut out = String::new();

    // Name field (up to 8): stop early on end/'.'/'*', padding with '*'.
    for _ in 0..8 {
        let c = at(i);
        if c == 0 || c == b'.' {
            out.push('*');
            break;
        }
        if c == b'*' {
            out.push('*');
            i += 1;
            break;
        }
        out.push(c as char);
        i += 1;
    }
    out.push('.');
    // Skip the rest of the input's name field, past the '.' separator.
    loop {
        let c = at(i);
        if c == 0 {
            break;
        }
        i += 1;
        if c == b'.' {
            break;
        }
    }
    // Extension field (up to 3), same padding rule. Iterate the remaining bytes
    // (no manual cursor) so the final step is not a dead store.
    let mut ext = b.get(i..).unwrap_or(&[]).iter();
    for _ in 0..3 {
        match ext.next() {
            None => {
                out.push('*');
                break;
            }
            Some(&b'*') => {
                out.push('*');
                break;
            }
            Some(&c) => out.push(c as char),
        }
    }
    out
}

/// `do_viewmenu` (EmuTOS `deskmain.c:786`) — the View menu. Flips the view /
/// sort / size-to-fit state and returns the change bitmask `hndl_menu` feeds to
/// [`desk_all`]. A view/sort item that is already current is a no-op (`0`);
/// size-to-fit toggles.
fn do_viewmenu(item: Item) -> u16 {
    use super::model::{Sort, View};
    let bits = global::with(|g| match item {
        Item::ShowIcons => {
            if g.view == View::Icon {
                0
            } else {
                g.view = View::Icon;
                g.win_view();
                VIEW_CHANGED
            }
        }
        Item::ShowText => {
            if g.view == View::Text {
                0
            } else {
                g.view = View::Text;
                g.win_view();
                VIEW_CHANGED
            }
        }
        Item::SortName => set_sort(g, Sort::Name),
        Item::SortType => set_sort(g, Sort::Type),
        Item::SortSize => set_sort(g, Sort::Size),
        Item::SortDate => set_sort(g, Sort::Date),
        Item::NoSort => set_sort(g, Sort::NoSort),
        Item::SizeToFit => {
            // Toggle (no "already selected" guard, unlike view/sort). This makes
            // the horizontal slider real when off — WF_HSLSIZ/WF_HSLIDE are
            // computed and pushed via `set_sliders`; `tos-api` currently ignores
            // them (a real AES gap, reported to the lead, not worked around).
            g.size_to_fit = !g.size_to_fit;
            g.win_view();
            SIZETOFIT_CHANGED
        }
        _ => 0,
    })
    .unwrap_or(0);
    // `case BACKGRND: if (inf_backgrounds()) rc = BACKGROUND_HAS_CHANGED;`
    // (`deskmain.c:823-828`) — `BACKGRND` (46) is a *View* item, so its arm
    // belongs in this function (DESK21). It runs outside the `global::with`
    // above because the dialog issues AES traps and cannot hold the desktop
    // lock.
    //
    // **Named deviation in the predicate, not in the placement.** EmuTOS's
    // `inf_backgrounds` returns TRUE only when the **window** background
    // changed (`deskinf.c:910-916`); a *desktop* background change it applies
    // and repaints itself with `do_wredraw(DESKWH, &G.g_desk)` and then
    // returns FALSE (`:900-908`). NewTOS's stand-in offers only the desktop
    // half — the `#Q` window bucket is round-tripped but has no consumer — and
    // its `true` means exactly that desktop change, which
    // [`super::inf::set_background`] has already repainted the same way. So
    // the EmuTOS-equivalent `rc` for every outcome this dialog can produce is
    // `0`, and `BACKGROUND_HAS_CHANGED` has no NewTOS counterpart to set: a
    // `desk_all` here would repaint every window for a change that touched
    // none of them.
    if item == Item::SetBackground {
        // `end_dialog`'s `form_dial(FMD_FINISH)` repaints exactly the
        // dialog's rectangle and re-asks the windows it covered — no
        // whole-desktop composite (G7).
        super::inf::set_background();
    }
    bits
}

/// Switch the sort key, returning `SORT_CHANGED` (or `0` if unchanged).
fn set_sort(g: &mut global::DeskState, sort: super::model::Sort) -> u16 {
    if g.sort == sort {
        0
    } else {
        g.sort = sort;
        SORT_CHANGED
    }
}

/// `desk_all` (EmuTOS `deskmain.c:438`) — the View-menu → redraw pipeline:
/// re-sort every window (`SORT_CHANGED`), rebuild every window's view
/// (`VIEW`/`SORT`/`SIZETOFIT`), then repaint every window's content. The
/// self-clearing white item-root box makes `win_shwall`'s `do_wredraw` erase
/// the old view before laying the new one — **this is the path that makes the
/// view-switch residue reproducible and structurally fixes it (D2/R2)**.
fn desk_all(bits: u16) {
    if bits & SORT_CHANGED != 0 {
        global::with(|g| g.win_srtall());
    }
    if bits & (VIEW_CHANGED | SORT_CHANGED | SIZETOFIT_CHANGED) != 0 {
        win_bdall();
    }
    redraw::win_shwall();
}

/// `win_bdall` (EmuTOS `deskwin.c:786`) — rebuild every open window's view for
/// its current work rect and push the new scrollbar geometry. No redraw
/// (`win_shwall` does that).
fn win_bdall() {
    let (handles, n) = open_handles();
    for &handle in &handles[..n] {
        let work = wind_get_work(handle);
        if let Some(slot) = global::with(|g| g.win_slot(handle)).flatten() {
            if let Some(sliders) = global::with(|g| g.win_bldview(slot, work)) {
                set_sliders(handle, sliders);
            }
        }
    }
}

// ── File-menu actions ──────────────────────────────────────────────────────

/// `case OPENITEM:` of `do_filemenu` (`deskmain.c:666-668`):
/// `if (pw || curr) done = do_open(pw, curr);` — open the single selected item
/// of the current window (a folder in a new window, a drive icon at its root, a
/// program through [`pro_run_native`]). Enabled only when exactly one item is
/// selected (`men_update`).
///
/// Returns `do_open`'s answer: TRUE iff a `shel_write` was issued, i.e. the
/// desktop is to end so the AES shell can start the program. Nothing selected
/// is EmuTOS's `if (pw || curr)` being false — no call, `done` stays FALSE.
fn do_open_selected() -> bool {
    let picked = global::with(|g| {
        let root = g.g_croot;
        let handle = g.g_cwin;
        let mut child = g.g_screen[root as usize].ob_head;
        while child != NIL && child != root {
            if g.g_screen[child as usize].ob_state & SELECTED != 0 {
                return Some((handle, child));
            }
            child = g.g_screen[child as usize].ob_next;
        }
        None
    })
    .flatten();
    match picked {
        Some((handle, obid)) => open_item(handle, obid),
        None => false,
    }
}

/// File → Close top window (EmuTOS `CLSWITEM`) — close the top window entirely.
fn do_close_window() {
    let handle = wind_get_top();
    if handle != DESKWH {
        close_window(handle);
    }
}

/// File → Close folder (EmuTOS `CLOSITEM`, also `WM_CLOSED`) — navigate the top
/// window up one directory level; at a drive root, close the window entirely.
///
/// The "up one level" arm is EmuTOS's `w_setpath` (`deskfun.c:809-825`):
/// `pw->w_cvrow = 0;  /* must reset slider */` and `do_fopen(pw, 0, pathname,
/// FALSE)` — `curr = 0`, so `do_wopen` neither zooms nor deselects. Its
/// `graf_shrinkbox` from the window centre is not built (no binding yet).
fn do_close_folder() {
    let handle = wind_get_top();
    if handle == DESKWH {
        return;
    }
    let spec = global::with(|g| {
        g.win_slot(handle).map(|s| g.windows.slots[s].w_pnode.p_spec.clone())
    })
    .flatten();
    let Some(spec) = spec else {
        return;
    };
    if is_drive_root(&spec) {
        close_window(handle);
    } else {
        let (pspec, ptitle) = parent_spec(&spec);
        global::with(|g| {
            if let Some(slot) = g.win_slot(handle) {
                g.windows.slots[slot].w_cvrow = 0;
            }
        });
        do_fopen(handle, 0, &pspec, &ptitle, false);
    }
}

/// File → Cycle windows (EmuTOS `BTOPITEM`, `deskmain.c:711-717`) — bring the
/// bottom-most window to the top. Enabled only with two or more windows
/// (`men_update`). EmuTOS synthesizes `fun_msg(WM_TOPPED, win_onbottom(), …)`;
/// NewTOS dispatches the same [`top_msg`] handler directly on the bottom window,
/// so cycle and a frame-click top are indistinguishable once dispatched.
fn do_cycle() {
    let bottom = global::with(|g| g.windows.zorder.last().map(|&s| g.windows.slots[s].w_id))
        .flatten();
    if let Some(handle) = bottom {
        top_msg(handle);
    }
}

// ── Window-manager messages ────────────────────────────────────────────────

/// `WM_TOPPED` (EmuTOS `deskmain.c:1282-1292`) — bring `handle` to the front.
/// Exact sequence: `desk_clear(G.g_cwin)` (deselect the *previously* current
/// window before switching); `pw = win_find(handle)`; if it exists,
/// `wind_set(handle, WF_TOP, …)` (tell the AES), `win_top(pw)` (reorder the
/// desktop's own z-chain + `objc_order`), `desk_verify(pw->w_id, FALSE)`
/// (`changed = FALSE` — topping doesn't rebuild the item grid), `change = TRUE`.
///
/// No explicit content repaint: EmuTOS relies on the AES sending `WM_REDRAW` for
/// the newly-uncovered pieces after `WF_TOP`, drained by the event loop into
/// [`redraw::do_wredraw`]. **Exposed gap (#123, P5-family):** NewTOS's AES
/// `wind_set(WF_TOP)` repaints background + frames but does not yet `send_redraw`
/// the topped/uncovered windows' content — so a topped window that was partly
/// covered is not re-laid here (only triggerable with two or more overlapping
/// windows, i.e. real-HW interaction / File→Cycle). Flagged, not masked with a
/// desktop-side composite.
fn top_msg(handle: i16) -> bool {
    desk_clear(global::with(|g| g.g_cwin).unwrap_or(DESKWH));
    if global::with(|g| g.win_find(handle).is_some()).unwrap_or(false) {
        super::wind_set_int(handle, WF_TOP, 0); // AES: top + repaint bg/frames
        global::with(|g| g.win_top(handle));
        let work = wind_get_work(handle);
        global::with(|g| g.desk_verify(handle, work, false));
        return true; // `change = TRUE` → the caller's `cnx_put()`
    }
    false
}

/// `WM_FULLED` (EmuTOS `deskmain.c:1296-1305`) — toggle the window between its
/// full extent and its previous size. Exact sequence: `pw = win_find(handle)`;
/// if it exists, `win_top(pw)` (fuller implies top), `do_wfull(handle)` (the
/// full/previous toggle via `wind_set(WF_CXYWH)`), `desk_verify(handle, TRUE)`
/// (`changed = TRUE` — a size change rebuilds the item grid), `change = TRUE`.
///
/// The window content repaint comes from the AES `send_redraw` that
/// [`do_wfull`]'s `wind_set(WF_CXYWH)` queues — drained by the loop into
/// [`redraw::do_wredraw`], after `desk_verify` has re-flowed the grid.
fn full_msg(handle: i16) -> bool {
    if global::with(|g| g.win_find(handle).is_some()).unwrap_or(false) {
        global::with(|g| g.win_top(handle)); // fuller implies top
        do_wfull(handle);
        let work = wind_get_work(handle);
        if let Some(sliders) = global::with(|g| g.desk_verify(handle, work, true)).flatten() {
            set_sliders(handle, sliders);
        }
        return true; // `change = TRUE` → the caller's `cnx_put()`
    }
    false
}

/// `do_wfull(handle)` (EmuTOS `desksupp.c` `do_wfull`) — toggle a window between
/// its full extent (`WF_FXYWH`) and its previous size (`WF_PXYWH`) via
/// `wind_set(handle, WF_CXYWH, …)`. The AES applies the geometry, repaints
/// background + frames over the damage, and `send_redraw`s the window's content.
fn do_wfull(handle: i16) {
    let cur = wind_get_grect(handle, WF_CXYWH);
    let full = wind_get_grect(handle, WF_FXYWH);
    let target = if cur.w >= full.w && cur.h >= full.h {
        wind_get_grect(handle, WF_PXYWH) // currently full → restore previous
    } else {
        full
    };
    super::wind_set_grect(handle, WF_CXYWH, target);
}

/// `do_xyfix(px, py)` (EmuTOS `desksupp.c:262-272`) — fix a desktop window's
/// position: align `x` to the nearest 16-pixel boundary and clamp `y` below the
/// menu bar. Applied to the `WM_MOVED`/`WM_SIZED` message rectangle before it is
/// confirmed to the AES with `wind_set(WF_CXYWH)`. (The `CONF_WITH_3D_OBJECTS`
/// "keep the mover on-screen" nudge, `desksupp.c:265-269`, is omitted — a niche
/// right-edge case not needed for the redraw contract.)
fn do_xyfix(r: &mut Grect) {
    r.x = (r.x + 8) & !0x000F; // align to the nearest 16-pixel boundary
    let desk_y = global::with(|g| g.g_desk.y).unwrap_or(0);
    if r.y < desk_y {
        r.y = desk_y; // ensure it's below the menu bar
    }
}

/// `WM_MOVED` (EmuTOS `deskmain.c:1317-1338`, shared move/size path) — confirm
/// the window's new position to the AES. Exact sequence: `pw = win_find(handle)`
/// (bail if gone); `rc_copy(&G.g_rmsg[4], &gr)`; `do_xyfix(&gr.x, &gr.y)`;
/// `wind_set_grect(handle, WF_CXYWH, &gr)`; then (the `WM_MOVED` arm)
/// `wind_get_grect(handle, WF_WXYWH, &gr)` and copy it straight into
/// `G.g_screen[pw->w_root].ob_x/y/w/h` — the desktop's cached copy of the
/// window's screen position for later screen-relative math.
///
/// The window content repaint comes from the AES `send_redraw` that
/// `wind_set(WF_CXYWH)` queues (drained by the loop into [`redraw::do_wredraw`]),
/// and so does the repaint of every *other* window the move uncovered — the AES
/// draws nothing during the drag itself, which is only the `gr_dragbox` XOR
/// outline. `change = TRUE → cnx_put()` (session save) is a future module.
fn moved_msg(handle: i16, msg: &[i16; 8]) -> bool {
    let Some(slot) = global::with(|g| g.win_slot(handle)).flatten() else {
        return false;
    };
    let mut gr = Grect { x: msg[4], y: msg[5], w: msg[6], h: msg[7] };
    do_xyfix(&mut gr);
    super::wind_set_grect(handle, WF_CXYWH, gr); // AES: repaint bg/frames + send_redraw
    let work = wind_get_work(handle); // WF_WXYWH — the confirmed work-area rect
    global::with(|g| {
        let root = g.windows.slots[slot].w_root as usize;
        g.g_screen[root].ob_x = work.x;
        g.g_screen[root].ob_y = work.y;
        g.g_screen[root].ob_width = work.w;
        g.g_screen[root].ob_height = work.h;
    });
    true // `change = TRUE` → the caller's `cnx_put()`
}

/// `WM_SIZED` (EmuTOS `deskmain.c:1317-1338`, shared move/size path) — confirm
/// the window's new geometry and rebuild the item grid. Exact sequence:
/// `pw = win_find(handle)` (bail if gone); `rc_copy(&G.g_rmsg[4], &gr)`;
/// `do_xyfix(&gr.x, &gr.y)`; `wind_set_grect(handle, WF_CXYWH, &gr)`; then (the
/// `WM_SIZED` arm) `desk_verify(handle, TRUE)` — rebuild the window, updating
/// `w_pncol`.
///
/// EmuTOS additionally detects a *shrink with a changed column count* and
/// synthesizes `fun_msg(WM_REDRAW, …)` because its AES sends no `WM_REDRAW` when a
/// window only shrinks (`deskmain.c:1325-1350`). NewTOS's AES `wind_set(WF_CXYWH)`
/// **unconditionally** `send_redraw`s the whole work area (grow *and* shrink), so
/// the re-flowed content is always repainted via [`redraw::do_wredraw`] once the
/// loop drains that message — the shrink-specific fixup is subsumed and omitted
/// (the observable contract, content re-flowing into fewer columns on shrink, is
/// met). `send_redraw` fires after `desk_verify` has rebuilt the grid.
fn sized_msg(handle: i16, msg: &[i16; 8]) -> bool {
    if global::with(|g| g.win_find(handle).is_some()).unwrap_or(false) {
        let mut gr = Grect { x: msg[4], y: msg[5], w: msg[6], h: msg[7] };
        do_xyfix(&mut gr);
        super::wind_set_grect(handle, WF_CXYWH, gr); // AES: repaint bg/frames + send_redraw
        let work = wind_get_work(handle);
        if let Some(sliders) = global::with(|g| g.desk_verify(handle, work, true)).flatten() {
            set_sliders(handle, sliders);
        }
        return true; // `change = TRUE` → the caller's `cnx_put()`
    }
    false
}

/// `WM_ARROWED` — an arrow-gadget scroll of `handle`.
fn arrow_msg(handle: i16, action: i16) {
    let Some(slot) = global::with(|g| g.win_slot(handle)).flatten() else {
        return;
    };
    let work = wind_get_work(handle);
    if let Some(sliders) = global::with(|g| g.win_arrow(slot, work, action)).flatten() {
        set_sliders(handle, sliders);
        redraw::do_wredraw(handle, work);
    }
}

/// `WM_VSLID` / `WM_HSLID` — a slider drag of `handle` (`value` = per-mille of
/// travel).
fn slid_msg(handle: i16, horizontal: bool, value: i16) {
    let Some(slot) = global::with(|g| g.win_slot(handle)).flatten() else {
        return;
    };
    let work = wind_get_work(handle);
    if let Some(sliders) = global::with(|g| g.win_slide(slot, work, horizontal, value)).flatten() {
        set_sliders(handle, sliders);
        redraw::do_wredraw(handle, work);
    }
}

// ── Session context (`cnx_put` / `cnx_get`, `deskmain.c:1365-1501`) ────────

/// `cnx_put` (EmuTOS `deskmain.c:1371-1427`) — the **in-memory** save of the
/// desktop's preferences and window layout into `G.g_cnxsave`.
///
/// Slot assignment, verbatim from the source: count the unused slots first and
/// blank them from the **end** of the array backwards, then walk the window
/// chain — which is front-to-back, most-recently-topped first — filling
/// downwards from where that first loop stopped. The topmost window therefore
/// lands in the highest used slot and **slot 0 holds the bottom-most window**
/// (`:1367-1369`), which is the order `cnx_get` needs to rebuild the z-order by
/// opening slot 0 first.
///
/// Every field is read live: the frame rectangle through `wind_get(WF_CXYWH)`
/// (then `do_xyfix`'d, `:1420-1421`), the scroll position from `w_cvcol`/
/// `w_cvrow`, the directory from `w_pnode.p_spec`.
pub(super) fn cnx_put() {
    // View/sort/size-to-fit and the Preferences fields — the preferences this
    // desktop has live state for (see `global::CSave` for the ones it does not).
    // `cs_dblclick` comes from `G.g_cdclkpref` and **not** from a fresh
    // `evnt_dclick` query (`deskmain.c:1382`): the field is a cache of what the
    // AES last reported, refreshed by `inf_pref` and by `cnx_get_prefs`, and
    // `cnx_put` runs on every window move/size — it must not trap.
    global::with(|g| {
        g.g_cnxsave.cs_view = g.view;
        g.g_cnxsave.cs_sort = g.sort;
        g.g_cnxsave.cs_sizefit = g.size_to_fit;
        g.g_cnxsave.cs_confdel = g.g_cdelepref;
        g.g_cnxsave.cs_confcpy = g.g_ccopypref;
        g.g_cnxsave.cs_confovwr = g.g_covwrpref;
        g.g_cnxsave.cs_dblclick = g.g_cdclkpref;
        g.g_cnxsave.cs_timefmt = g.g_ctimeform;
        g.g_cnxsave.cs_datefmt = g.g_cdateform;
        // `cnxsave->cs_appdir = G.g_appdir;` (`deskmain.c:1391`).
        g.g_cnxsave.cs_appdir = g.g_appdir;
        // `cnxsave->cs_fullpath = G.g_fullpath;` (`deskmain.c:1392`).
        g.g_cnxsave.cs_fullpath = g.g_fullpath;
    });

    // The open windows, front-to-back, with their desktop slot and handle.
    let open: alloc::vec::Vec<(usize, i16)> = global::with(|g| {
        g.windows
            .zorder
            .iter()
            .filter(|&&slot| g.windows.slots[slot].w_id != 0)
            .map(|&slot| (slot, g.windows.slots[slot].w_id))
            .collect()
    })
    .unwrap_or_default();

    // The AES geometry query runs with the desktop lock released.
    let mut saved: alloc::vec::Vec<global::WSave> = alloc::vec::Vec::new();
    for &(slot, handle) in &open {
        let mut gr = wind_get_grect(handle, WF_CXYWH);
        do_xyfix(&mut gr);
        let (cvcol, cvrow, spec) = global::with(|g| {
            let w = &g.windows.slots[slot];
            (w.w_cvcol, w.w_cvrow, w.w_pnode.p_spec.clone())
        })
        .unwrap_or((0, 0, String::new()));
        saved.push(global::WSave {
            x_save: gr.x,
            y_save: gr.y,
            w_save: gr.w,
            h_save: gr.h,
            hsl_save: cvcol,
            vsl_save: cvrow,
            pth_save: spec,
        });
    }

    global::with(|g| {
        // "first, count the unused slots & initialise them" — the trailing
        // slots keep their rectangle (it is the default geometry `win_alloc`
        // hands the next window) and lose only their path.
        let used = saved.len().min(NUM_WNODES);
        for ws in g.g_cnxsave.cs_wnode[used..].iter_mut() {
            ws.pth_save.clear();
        }
        // "next, save the info for the open windows": front-to-back into
        // descending slots, so slot 0 ends up holding the bottom-most window.
        for (i, ws) in saved.into_iter().take(NUM_WNODES).enumerate() {
            g.g_cnxsave.cs_wnode[used - 1 - i] = ws;
        }
    });
}

/// `cnx_get` (EmuTOS `deskmain.c:1430-1501`) — apply `G.g_cnxsave` to the live
/// desktop: the saved preferences, then the saved windows.
///
/// The preferences go through `do_viewmenu` exactly as EmuTOS does
/// (`do_viewmenu(ICONITEM + cs_view)` / `do_viewmenu(NAMEITEM + cs_sort)`,
/// `:1437-1438`) — the menu dispatcher is reused so the view/sort machinery and
/// the menu state stay in one place; `cs_sizefit` is assigned directly, as there
/// its menu item is a toggle.
///
/// Every saved rectangle is then **validated against the current desktop**
/// (`:1466-1483`): an off-screen origin is pulled back to the middle, a
/// non-positive or oversized extent becomes the full desktop. That is what makes
/// a layout saved at one resolution survive being read at another. Slots are
/// walked `0..NUM_WNODES`, i.e. bottom-most first, so the z-order comes back the
/// way it was saved. A slot with an empty path is not a window.
///
/// Returns how many windows were opened.
pub(super) fn cnx_get() -> usize {
    cnx_get_prefs();

    let desk = g_desk();
    let mut opened = 0usize;
    for nw in 0..NUM_WNODES {
        let Some(mut ws) = global::with(|g| g.g_cnxsave.cs_wnode[nw].clone()) else {
            break;
        };

        // "Check for valid position" / "Check for valid width + height".
        if ws.x_save >= desk.w {
            ws.x_save = desk.w / 2;
        }
        if ws.y_save >= desk.h {
            ws.y_save = desk.h / 2;
        }
        if ws.w_save <= 0 || ws.w_save > desk.w {
            ws.w_save = desk.w;
        }
        if ws.h_save <= 0 || ws.h_save > desk.h {
            ws.h_save = desk.h;
        }
        let mut gr = Grect { x: ws.x_save, y: ws.y_save, w: ws.w_save, h: ws.h_save };
        do_xyfix(&mut gr);
        ws.x_save = gr.x;
        ws.y_save = gr.y;
        // EmuTOS validates in place, so the corrected values are what a later
        // `app_save` writes out even if the window never opens.
        let path = ws.pth_save.clone();
        let (cvcol, cvrow) = (ws.hsl_save, ws.vsl_save);
        global::with(|g| g.g_cnxsave.cs_wnode[nw] = ws);

        if path.is_empty() {
            continue;
        }
        // `WORD obid = obj_get_obid(pws->pth_save[0]);
        //  if ((pw = win_alloc(obid))) {
        //      pw->w_cvcol = pws->hsl_save; pw->w_cvrow = pws->vsl_save;
        //      do_xyfix(&pws->x_save, &pws->y_save);
        //      if (!do_diropen(pw, TRUE, obid, pws->pth_save, (GRECT *)pws, TRUE))
        //          win_free(pw);
        //  }` (`:1489-1499`). The scroll position is set before the view is
        // built so `win_bldview` lays out at that offset; `do_xyfix` ran above.
        let obid = obj_get_obid(path.as_bytes()[0]);
        let Some((slot, handle)) = win_alloc_window(obid) else {
            continue;
        };
        global::with(|g| {
            g.windows.slots[slot].w_cvcol = cvcol;
            g.windows.slots[slot].w_cvrow = cvrow;
        });
        let (spec, title) = spec_and_title(&path);
        if do_diropen(handle, true, obid, &spec, &title, gr, true) {
            opened += 1;
        } else {
            win_free_window(handle);
        }
    }
    opened
}

/// The preference half of [`cnx_get`] (`deskmain.c:1437-1461`), on its own
/// because NewTOS's Options → Read .INF re-applies a re-read file **in place**
/// where EmuTOS restarts the whole desktop (`READITEM`, `deskmain.c:919-924`):
/// there the context's windows are already on screen and must not be opened a
/// second time.
fn cnx_get_prefs() {
    let (view, sort, sizefit) = global::with(|g| {
        (g.g_cnxsave.cs_view, g.g_cnxsave.cs_sort, g.g_cnxsave.cs_sizefit)
    })
    .unwrap_or((super::model::View::Icon, super::model::Sort::Name, true));

    do_viewmenu(view_item(view));
    do_viewmenu(sort_item(sort));
    // `G.g_cdelepref = cnxsave->cs_confdel;` and the format pair (`:1441-1446`).
    let dclick = global::with(|g| {
        g.size_to_fit = sizefit;
        g.g_cdelepref = g.g_cnxsave.cs_confdel;
        g.g_ccopypref = g.g_cnxsave.cs_confcpy;
        g.g_covwrpref = g.g_cnxsave.cs_confovwr;
        g.g_cdclkpref = g.g_cnxsave.cs_dblclick;
        g.g_ctimeform = g.g_cnxsave.cs_timefmt;
        g.g_cdateform = g.g_cnxsave.cs_datefmt;
        // `G.g_appdir = cnxsave->cs_appdir;` (`deskmain.c:1453`).
        g.g_appdir = g.g_cnxsave.cs_appdir;
        // `G.g_fullpath = cnxsave->cs_fullpath;` (`deskmain.c:1454`).
        g.g_fullpath = g.g_cnxsave.cs_fullpath;
        g.win_view();
        g.g_cdclkpref
    })
    .unwrap_or(global::DEF_DCLICK);
    // `G.g_cdclkpref = evnt_dclick(G.g_cdclkpref, TRUE);` (`:1460`) — push the
    // restored rate into the AES and take back what it actually set, outside the
    // desktop lock (the trap may block).
    let applied = super::evnt_dclick(dclick as i16, true).clamp(0, 4) as u8;
    global::with(|g| g.g_cdclkpref = applied);
}

/// Split a saved `#W` path into the scanner spec and the window title.
///
/// EmuTOS saves `w_pnode.p_spec`, which always carries a mask
/// (`C:\SUB\*.*`), and rebuilds the title from it in `win_sname`
/// (`deskwin.c:847-858`). A path written by hand without a mask gets the
/// all-files one; the title is the directory, with the mask appended only when
/// it is a real filter — the same rule [`set_window_mask`] uses.
fn spec_and_title(path: &str) -> (String, String) {
    let mut spec = String::from(path);
    if spec.is_empty() || spec.ends_with('\\') {
        spec.push_str("*.*");
    }
    let cut = spec.rfind('\\').map(|i| i + 1).unwrap_or(0);
    let (dir, mask) = spec.split_at(cut);
    let mut title = String::from(dir);
    if !mask.is_empty() && mask != "*.*" {
        title.push_str(mask);
    }
    (spec, title)
}

/// The View-menu item for a saved view mode — EmuTOS `ICONITEM + cs_view`
/// (`deskmain.c:1437`).
fn view_item(view: super::model::View) -> Item {
    match view {
        super::model::View::Icon => Item::ShowIcons,
        super::model::View::Text => Item::ShowText,
    }
}

/// The View-menu item for a saved sort key — EmuTOS `NAMEITEM + cs_sort`
/// (`deskmain.c:1438`).
fn sort_item(sort: super::model::Sort) -> Item {
    match sort {
        super::model::Sort::Name => Item::SortName,
        super::model::Sort::Type => Item::SortType,
        super::model::Sort::Size => Item::SortSize,
        super::model::Sort::Date => Item::SortDate,
        super::model::Sort::NoSort => Item::NoSort,
    }
}

// ── Window open / close / navigate ─────────────────────────────────────────

/// `STNOWIND` (`desk_rsc.c:2492-2495`) — the alert `do_dopen`/`do_fopen` raise
/// when `win_alloc` has no node left. Its own icon and button, so it goes
/// straight to `form_alert` (`fun_alert(1, STNOWIND)` = `form_alert(1,
/// desktop_str_addr(STNOWIND))`, `deskfun.c:75-78`).
const STNOWIND: &[u8] =
    b"[1][The GEM Desktop has no more|windows.  Please close a|window that you are not|using.][  OK  ]\0";

/// `obj_get_obid(drive)` (EmuTOS `deskobj.c:249-261`) — the desktop icon of
/// drive letter `drive`: the first `G_ICON` child of `DROOT` whose `ib_char`
/// low byte is the letter, or **0** when there is none. `do_dopen` uses it for
/// a drive opened by Alt+letter and `cnx_get` for a restored window, both to
/// get a source object for `do_wopen`'s zoom and for `win_alloc`'s `w_obid`.
///
/// The walk stops at the parent the way every child walk in this file does;
/// EmuTOS's `objnum >= 0` runs on through `DROOT`'s own siblings, which are
/// window boxes and never `G_ICON`s, so the answer is the same.
fn obj_get_obid(drive: u8) -> i16 {
    global::with(|g| {
        let mut obj = g.g_screen[DROOT as usize].ob_head;
        while obj != NIL && obj != DROOT {
            if g.g_screen[obj as usize].ob_type == G_ICON
                && (g.g_screeninfo[obj as usize].block.ib_char & 0x00FF) as u8 == drive
            {
                return obj;
            }
            obj = g.g_screen[obj as usize].ob_next;
        }
        0
    })
    .unwrap_or(0)
}

/// `win_alloc(obid)` (EmuTOS `deskwin.c:156-190`) — a window node **and** its
/// AES window: `if (G.g_wcnt == NUM_WNODES) return NULL;`, then
/// `wind_create_grect(WINDOW_STYLE, &G.g_desk)`, and on failure nothing is
/// left behind (`win_free`). [`global::DeskState::win_alloc`] is the list half
/// only (it needs the handle), so the two are joined here. `obid` becomes
/// `w_obid`, the icon the window was opened from (`:173`).
///
/// Returns the slot and the AES handle.
fn win_alloc_window(obid: i16) -> Option<(usize, i16)> {
    if global::with(|g| g.win_count() >= NUM_WNODES).unwrap_or(true) {
        return None;
    }
    let handle = super::wind_create(WINDOW_STYLE as i16, g_desk());
    if handle < 1 {
        return None;
    }
    match global::with(|g| g.win_alloc(handle, obid)).flatten() {
        Some(slot) => Some((slot, handle)),
        None => {
            super::wind_delete(handle);
            None
        }
    }
}

/// `win_free(pw)` (EmuTOS `deskwin.c:141-150`) for a node whose window was
/// created but never opened — the cleanup after a failed `do_diropen`:
/// `wind_delete` plus the list half [`global::DeskState::win_free`].
fn win_free_window(handle: i16) {
    super::wind_delete(handle);
    global::with(|g| g.win_free(handle));
}

/// The rectangle a freshly allocated node opens at — EmuTOS's
/// `G.g_screen[pw->w_root].ob_x` after `win_alloc` copied
/// `cs_wnode[G.g_wcnt]` into it (`deskwin.c:165-167`), which `do_dopen` and
/// `do_fopen` hand to `do_diropen` (`desksupp.c:1067,1148`). Ours is the saved
/// `#W` rectangle of the slot ([`super::inf::window_rect`]) or a staggered
/// sub-rectangle of the desktop when there is none (`deskmain.md` §`cnx_get`,
/// deviation 4). `do_wopen`'s `do_xyfix` aligns it.
fn win_default_rect(slot: usize) -> Grect {
    let desk = g_desk();
    let step = (slot as i16) * 24;
    let rw = (desk.w - 96).min(560);
    let rh = (desk.h - 56).min(400);
    let default_r = Grect { x: desk.x + 48 + step, y: desk.y + 24 + step, w: rw, h: rh };
    super::inf::window_rect(slot).unwrap_or(default_r)
}

/// `do_wopen(new_win, wh, curr, pt)` (EmuTOS `desksupp.c:294-322`) — the tail
/// every window open and every program launch share
/// (`docs/api/desktop/desksupp-wopen.md`):
///
/// ```c
/// t = *pt;
/// do_xyfix(&t.g_x, &t.g_y);
/// if (curr > 0) {
///     get_xywh(G.g_screen, G.g_croot, &c.g_x, &c.g_y, &c.g_w, &c.g_h);
///     get_xywh(G.g_screen, curr, &d.g_x, &d.g_y, &d.g_w, &d.g_h);
///     d.g_x += c.g_x;  d.g_y += c.g_y;
///     graf_growbox_grect(&d, &t);
///     act_chg(G.g_cwin, G.g_croot, curr, &gl_rfull, FALSE, new_win?TRUE:FALSE);
/// }
/// if (new_win)
///     wind_open_grect(wh, &t);
/// G.g_wlastsel = wh;
/// ```
///
/// The zoom source is `curr`'s rectangle offset by the **current** root
/// (`G.g_croot`), which is [`global::DeskState::obj_extent`]; the deselect
/// paints only when a new window opens — in the same-window case the window's
/// file list has already been replaced, and for `pro_run` the desktop is
/// about to end (`:282-292`). `gl_rfull` is `G.g_desk` here as at the drag
/// loop's `gr_drgplns` call.
pub(super) fn do_wopen(new_win: bool, wh: i16, curr: i16, pt: Grect) {
    let mut t = pt;
    do_xyfix(&mut t);

    if curr > 0 {
        let (cwin, croot, d) = global::with(|g| (g.g_cwin, g.g_croot, g.obj_extent(g.g_croot, curr)))
            .unwrap_or((DESKWH, DROOT, None));
        if let Some(d) = d {
            super::graf_growbox(d, t);
        }
        act_chg_draw(cwin, croot, curr, g_desk(), false, new_win);
    }

    if new_win {
        super::wind_open(wh, t);
    }

    global::with(|g| g.g_wlastsel = wh);
}

/// `do_diropen(pw, new_win, curr_icon, pathname, pt, redraw)` (EmuTOS
/// `desksupp.c:422-471`) — fill window `handle`'s node from the directory
/// `pathname` and show it. **Every** directory window goes through here:
///
/// | EmuTOS caller | NewTOS |
/// |---|---|
/// | `do_dopen` (`desksupp.c:1067`) | [`do_dopen`] |
/// | `do_fopen` (`desksupp.c:1156`) | [`do_fopen`], both arms |
/// | `cnx_get` (`deskmain.c:1496`) | [`cnx_get`] |
/// | `search_display` (`deskfun.c:413`) | none — the recursive search is not built ([`do_search`]) |
///
/// ```c
/// desk_busy_on();
/// if (!pn_open(pathname, pw)) { desk_busy_off(); return FALSE; }
/// ret = pn_active(&pw->w_pnode, TRUE);
/// if (ret < 0) { pn_close(&pw->w_pnode); desk_busy_off(); return FALSE; }
/// win_sname(pw); win_sinfo(pw, FALSE); wind_set(pw->w_id, WF_NAME, …);
/// do_wopen(new_win, pw->w_id, curr_icon, pt);
/// if (new_win) win_top(pw);
/// desk_verify(pw->w_id, TRUE);
/// if (redraw && !new_win) fun_msg(WM_REDRAW, pw->w_id, pt->g_x, …);
/// desk_busy_off();
/// return TRUE;
/// ```
///
/// `title` is passed in because NewTOS titles a window with its directory
/// rather than `win_sname`'s whole `p_spec` (named difference 1 of the spec
/// entry). `pn_sort` follows the scan because ours is a separate step.
/// `fun_msg(WM_REDRAW, …)` runs `hndl_msg`'s `WM_REDRAW` arm synchronously,
/// which is `do_wredraw(handle, pt)`. On `false` the caller owns the cleanup,
/// as in EmuTOS.
fn do_diropen(
    handle: i16,
    new_win: bool,
    curr_icon: i16,
    pathname: &str,
    title: &str,
    pt: Grect,
    redraw: bool,
) -> bool {
    pro::desk_busy_on();

    // `if (!pn_open(pathname, pw))` — "pathname is too long".
    let slot = global::with(|g| {
        let slot = g.win_slot(handle)?;
        g.windows.slots[slot].w_pnode.pn_open(pathname).then_some(slot)
    })
    .flatten();
    let Some(slot) = slot else {
        pro::desk_busy_off();
        return false;
    };

    // `ret = pn_active(&pw->w_pnode, TRUE); if (ret < 0) { pn_close(…); … }`
    // — "error reading directory".
    let scanned = global::with(|g| {
        let sort = g.sort;
        let pn = &mut g.windows.slots[slot].w_pnode;
        if pn.pn_active(true) < 0 {
            pn.pn_close();
            return None;
        }
        pn.pn_sort(sort);
        Some((pn.p_count, pn.p_size))
    })
    .flatten();
    let Some((count, size)) = scanned else {
        pro::desk_busy_off();
        return false;
    };

    // `win_sname(pw); win_sinfo(pw, FALSE); wind_set(pw->w_id, WF_NAME, …);`
    set_window_strings(handle, slot, title, count, size);

    do_wopen(new_win, handle, curr_icon, pt);
    if new_win {
        global::with(|g| g.win_top(handle));
    }

    // `desk_verify(pw->w_id, TRUE);` — the view for the actual work area, and
    // this window becomes the current one.
    let work = wind_get_work(handle);
    if let Some(sliders) = global::with(|g| g.desk_verify(handle, work, true)).flatten() {
        set_sliders(handle, sliders);
    }

    // `if (redraw && !new_win) fun_msg(WM_REDRAW, pw->w_id, pt…);` — a new
    // window is painted by the AES's own `WM_REDRAW` after `wind_open`.
    if redraw && !new_win {
        redraw::do_wredraw(handle, pt);
    }

    pro::desk_busy_off();
    true
}

/// `do_dopen(curr)` (EmuTOS `desksupp.c:1041-1079`) — open a window on a
/// drive's root:
///
/// ```c
/// if (curr >= 0) { pib = get_iconblk_ptr(G.g_screen, curr); drv = pib->ib_char & 0x00ff; }
/// else           { drv = -curr; curr = obj_get_obid(drv); }
/// if (!valid_drive(drv)) return FALSE;
/// pw = win_alloc(curr);
/// if (pw) {
///     build_root_path(path, drv); set_all_files(path+3);
///     if (!do_diropen(pw, TRUE, curr, path, &G.g_screen[pw->w_root].ob_x, TRUE)) {
///         win_free(pw);
///         act_chg(DESKWH, DROOT, curr, &gl_rfull, FALSE, TRUE);
///     }
/// } else
///     fun_alert(1, STNOWIND);
/// return FALSE;
/// ```
///
/// `curr >= 0` is a drive icon ([`open_item`]); a negative `curr` is
/// `-letter` from Alt+letter ([`check_alt_letter_key`], `deskmain.c:1157`).
/// The answer is always `FALSE`, as in the source.
pub(super) fn do_dopen(curr: i16) -> bool {
    let (drv, curr) = if curr >= 0 {
        let drv = global::with(|g| (g.g_screeninfo[curr as usize].block.ib_char & 0x00FF) as u8).unwrap_or(0);
        (drv, curr)
    } else {
        let drv = (-curr) as u8;
        (drv, obj_get_obid(drv))
    };

    if !valid_drive(drv) {
        return false;
    }

    match win_alloc_window(curr) {
        Some((slot, handle)) => {
            // `build_root_path(path, drv); set_all_files(path+3);` → `X:\*.*`.
            let mut path = String::new();
            path.push(drv as char);
            path.push_str(":\\");
            let title = path.clone();
            path.push_str("*.*");
            if !do_diropen(handle, true, curr, &path, &title, win_default_rect(slot), true) {
                win_free_window(handle);
                act_chg_draw(DESKWH, DROOT, curr, g_desk(), false, true);
            }
        }
        None => {
            super::form_alert(1, STNOWIND.as_ptr());
        }
    }
    false
}

/// `do_fopen(pw, curr, pathname, allow_new_win)` (EmuTOS
/// `desksupp.c:1089-1160`) — open the folder `spec` **for the window `handle`**:
/// normally *in that same window*, in a new one only in the cases EmuTOS lists
/// in the function's own header comment (`:1084-1088`).
///
/// ```c
/// wind_get_grect(pw->w_id, WF_WXYWH, &t);                 /* :1095 */
/// if (allow_new_win) {
///     graf_mkstate(&junk, &junk, &junk, &keystate);
///     if ((keystate & MODE_SCA) == MODE_ALT)      /* :1130-1135 */
///         new_win = TRUE;
/// }
/// if (new_win) { pw = win_alloc(curr); if (!pw) { fun_alert(1, STNOWIND); return; }
///                rc_copy(&G.g_screen[pw->w_root].ob_x, &t); }  /* :1141-1149 */
/// else         { pn_close(&pw->w_pnode); }                       /* :1151-1154 */
/// if (!do_diropen(pw, new_win, curr, app_path, &t, TRUE))       /* :1156 */
///     if (new_win) win_free(pw);
/// ```
///
/// * `new_win == FALSE` → the same `WNODE` is re-filled: same frame, new path
///   in the title, no `wind_open`, and a direct `WM_REDRAW` of the work area
///   `t`. This is also what Close-folder's "up one level" uses
///   ([`do_close_folder`], EmuTOS `w_setpath` → `do_fopen(pw, 0, …, FALSE)`,
///   `deskfun.c:824`) — `curr = 0` there, so no zoom.
/// * `new_win == TRUE` → `win_alloc` + `do_diropen(…, TRUE, …)` at the new
///   node's **own** default rectangle (`rc_copy` copies the new node's rect
///   *into* `t`, so the old window's geometry is discarded).
///
/// Both arms zoom from `curr` (`do_wopen`, when `curr > 0`) and deselect it —
/// painted only for a new window, because in the same-window arm the item
/// tree is rebuilt from the new directory anyway.
///
/// **Two of EmuTOS's three new-window cases reach this function; the third is
/// a separate caller.** A drive icon is [`do_dopen`] (`:1041-1069`), which
/// always allocates. The other two are here: the Alt-held case above, and the
/// `WN_DESKTOP` case (`:1112-1127`, `CONF_WITH_DESKTOP_SHORTCUTS`, DESK30) — a
/// **folder shortcut** opened from the desktop surface, which always gets a
/// new window because the "window" it was opened from is the desktop
/// pseudo-window and has no file list to replace:
///
/// ```c
/// if (pw->w_flags & WN_DESKTOP) {
///     char *p = filename_start(app_path); *p = '\0';
///     if (set_default_path(app_path) == EPTHNF) { remove_locate_shortcut(curr); return; }
///     set_all_files(p);
///     new_win = TRUE;
/// }
/// ```
///
/// The `set_default_path` is both the *chdir* and the **existence probe**:
/// `EPTHNF` from it is how EmuTOS notices that the folder a shortcut points at
/// has been renamed or deleted, and it is where the orphan prompt is raised
/// ("handle renamed target of shortcut"). `curr` is the icon that prompt names.
fn do_fopen(handle: i16, curr: i16, spec: &str, title: &str, allow_new_win: bool) {
    // `wind_get_grect(pw->w_id, WF_WXYWH, &t);` (`:1095`) — the damage
    // rectangle of the same-window redraw. The desktop pseudo-window has no
    // AES window behind it; its arm always replaces `t` below.
    let t = if handle == DESKWH { g_desk() } else { wind_get_work(handle) };

    let new_win = if handle == DESKWH {
        // The `WN_DESKTOP` arm (`:1112-1126`): chdir into the folder, which
        // doubles as the existence probe, then always open a new window.
        let dir = &spec[..=spec.rfind('\\').unwrap_or(0)];
        if pro::set_default_path(dir) == tos_types::error::EPTHNF {
            dir::remove_locate_shortcut(curr);
            return;
        }
        true
    } else {
        // `graf_mkstate(&junk, &junk, &junk, &keystate);` (`:1132`) — read at
        // the moment of the open, as EmuTOS does, not the keystate the click
        // carried.
        allow_new_win && {
            let keystate = super::graf_mkstate().kstate as u16;
            keystate & MODE_SCA == MODE_ALT
        }
    };

    if new_win {
        // `pw = win_alloc(curr); if (!pw) { fun_alert(1, STNOWIND); return; }
        //  rc_copy((GRECT *)&G.g_screen[pw->w_root].ob_x, &t);` (`:1141-1149`)
        let Some((slot, new_handle)) = win_alloc_window(curr) else {
            super::form_alert(1, STNOWIND.as_ptr());
            return;
        };
        if !do_diropen(new_handle, true, curr, spec, title, win_default_rect(slot), true) {
            win_free_window(new_handle); // `if (new_win) win_free(pw);`
        }
    } else {
        // `pn_close(&pw->w_pnode);` (`:1153`).
        global::with(|g| {
            if let Some(slot) = g.win_slot(handle) {
                g.windows.slots[slot].w_pnode.pn_close();
            }
        });
        do_diropen(handle, false, curr, spec, title, t, true);
    }
}

/// Close window `handle`: AES `wind_close` + `wind_delete`, free the desktop
/// slot, and re-verify whatever is now on top.
fn close_window(handle: i16) {
    super::wind_close(handle);
    super::wind_delete(handle);
    global::with(|g| g.win_free(handle));
    let top = wind_get_top();
    let work = if top == DESKWH { g_desk() } else { wind_get_work(top) };
    global::with(|g| g.desk_verify(top, work, false));
    // No desktop-side repaint: the AES `wind_close` repainted the desktop
    // background + icons + the frames of the windows below over the vacated rect.
    // A window that was *below* the closed one needs its content re-laid — EmuTOS
    // relies on the AES sending it a `WM_REDRAW`. **Exposed gap (#123, P5):**
    // NewTOS's AES `wind_close` does not yet `send_redraw` the newly-uncovered
    // windows, so a lower window's content is not re-laid here (only observable
    // with overlapping windows; closing the single boot window reveals the
    // desktop surface, which the AES does repaint). Flagged, not masked.
}

// ── Small helpers ──────────────────────────────────────────────────────────

/// The desktop work-area rectangle.
fn g_desk() -> Grect {
    global::with(|g| g.g_desk).unwrap_or_default()
}

/// The open (non-desktop) window handles, into a fixed buffer.
fn open_handles() -> ([i16; NUM_WNODES], usize) {
    let mut h = [0i16; NUM_WNODES];
    let n = global::with(|g| {
        let mut n = 0;
        for w in &g.windows.slots {
            if w.w_id != DESKWH && n < h.len() {
                h[n] = w.w_id;
                n += 1;
            }
        }
        n
    })
    .unwrap_or(0);
    (h, n)
}

/// Push a window's scrollbar size/position (`wind_set` `WF_*SLSIZ`/`WF_*SLIDE`).
/// The horizontal pair is currently ignored by `tos-api` (see the size-to-fit
/// note); the values are still computed and sent so the desktop side is correct
/// the moment the AES renders a horizontal slider.
fn set_sliders(handle: i16, s: SliderSet) {
    super::wind_set_int(handle, WF_VSLSIZ, s.vslsize);
    super::wind_set_int(handle, WF_VSLIDE, s.vslide);
    super::wind_set_int(handle, WF_HSLSIZ, s.hslsize);
    super::wind_set_int(handle, WF_HSLIDE, s.hslide);
}

/// `set_sliders` for [`dir::rebuild_window`] (cross-module caller).
pub fn set_sliders_pub(handle: i16, s: SliderSet) {
    set_sliders(handle, s);
}

/// `fun_msg(MN_SELECTED, …)` (EmuTOS `deskfun.c:101`) — synthesize a menu
/// message and dispatch it synchronously through `hndl_menu`, exactly as a real
/// `MN_SELECTED` would (`fun_msg` writes into `G.g_rmsg` and calls `hndl_msg`
/// in-process). Used by the Delete→Remove redirect ([`dir::fun_del`]).
pub fn fun_msg_mn_selected(title: Title, item: Item) {
    desk_verify_last();
    let title_obj = menu::title_obj(title);
    hndl_menu(item, title_obj);
}

/// Deselect every window (and the desktop), repainting those that changed
/// (EmuTOS `desk_clear_all`).
fn desk_clear_all() {
    let (handles, n) = open_handles();
    let mut any_win = false;
    for &handle in &handles[..n] {
        if desk_clear(handle) {
            any_win = true;
        }
    }
    let any_desk = desk_clear(DESKWH);
    if any_win {
        redraw::win_shwall();
    }
    if any_desk {
        redraw::do_wredraw(DESKWH, g_desk());
    }
}

/// Rebuild + repaint the topped window (EmuTOS ESC `refresh_window`).
fn refresh_top() {
    let handle = wind_get_top();
    if handle == DESKWH {
        return;
    }
    let work = wind_get_work(handle);
    if let Some(slot) = global::with(|g| g.win_slot(handle)).flatten() {
        if let Some(sliders) = global::with(|g| g.win_bldview(slot, work)) {
            set_sliders(handle, sliders);
        }
    }
    redraw::do_wredraw(handle, work);
}

/// Make `g_wlastsel` the current window again (EmuTOS `desk_verify` before an
/// `MN_SELECTED` dispatch).
fn desk_verify_last() {
    let handle = global::with(|g| g.g_wlastsel).unwrap_or(DESKWH);
    let work = if handle == DESKWH { g_desk() } else { wind_get_work(handle) };
    global::with(|g| g.desk_verify(handle, work, false));
}

/// Repaint a single window's (or the desktop's) content.
fn redraw_window(handle: i16) {
    if handle == DESKWH {
        redraw::do_wredraw(DESKWH, g_desk());
    } else {
        let work = wind_get_work(handle);
        redraw::do_wredraw(handle, work);
    }
}

/// Whether `spec` is a drive root (`X:\*.*`) — its directory part is just
/// `X:\`.
fn is_drive_root(spec: &str) -> bool {
    let dir = &spec[..=spec.rfind('\\').unwrap_or(0)];
    dir.len() == 3 && dir.as_bytes().get(1) == Some(&b':')
}

/// The parent directory's `(spec, title)` for a non-root `spec` (`C:\A\B\*.*`
/// → `C:\A\*.*`, `C:\A\`).
fn parent_spec(spec: &str) -> (String, String) {
    let dir = &spec[..=spec.rfind('\\').unwrap_or(0)]; // ".../"
    let trimmed = dir.trim_end_matches('\\'); // "..."
    let parent_dir = match trimmed.rfind('\\') {
        Some(i) => &trimmed[..=i],
        None => trimmed,
    };
    let mut s = String::from(parent_dir);
    s.push_str("*.*");
    (s, String::from(parent_dir))
}

/// The child directory's `(spec, title)` for folder `name` inside `parent_spec`
/// (`C:\*.*`, `SUB` → `C:\SUB\*.*`, `C:\SUB\`).
fn child_spec(parent_spec: &str, name: &str) -> (String, String) {
    let dir = &parent_spec[..=parent_spec.rfind('\\').unwrap_or(0)];
    let mut d = String::from(dir);
    d.push_str(name);
    d.push('\\');
    let title = d.clone();
    d.push_str("*.*");
    (d, title)
}

/// Build a directory-window info line into `out` (NUL-terminated, truncated
/// to fit) — the node's `w_info` buffer, EmuTOS `win_sinfo`'s
/// `sprintf(pwin->w_info, alert, select_size, select_count)`
/// (`desk/deskwin.c:905`).
///
/// The format string is the resource item `STINFOST`, `" %ld bytes used in
/// %d items."` (`desk/desk_rsc.c:2468`, index 0 of `desk_rs_fstr`): leading
/// blank, **size first**, count second, trailing full stop. NewTOS wrote
/// `" N items   M bytes used"` — its own wording and the two numbers the
/// other way round (A109).
///
/// `selected` picks the second resource string, `STINFST2` — `" %ld bytes used
/// in %d selected items."` (`desk_rsc.c:2469`, index 1) — which `win_sinfo`
/// uses when it counted selected `FNODE`s (`deskwin.c:891-902`). Both strings
/// take the same two arguments in the same order; only the noun phrase differs
/// (A110).
fn fmt_info(out: &mut [u8], count: usize, size: u64, selected: bool) {
    let mut p = 0usize;
    let mut num = [0u8; 20];
    let push = |out: &mut [u8], p: &mut usize, s: &[u8]| {
        for &b in s {
            if *p < out.len() - 1 {
                out[*p] = b;
                *p += 1;
            }
        }
    };
    push(out, &mut p, b" ");
    push(out, &mut p, fmt_u64(size, &mut num));
    push(out, &mut p, b" bytes used in ");
    push(out, &mut p, fmt_u64(count as u64, &mut num));
    push(out, &mut p, if selected { b" selected items." as &[u8] } else { b" items." });
    out[p] = 0;
}

/// Format `v` as decimal into `buf`, returning the written slice.
fn fmt_u64(mut v: u64, buf: &mut [u8; 20]) -> &[u8] {
    if v == 0 {
        buf[0] = b'0';
        return &buf[..1];
    }
    let mut tmp = [0u8; 20];
    let mut i = 20;
    while v > 0 {
        i -= 1;
        tmp[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    let len = 20 - i;
    buf[..len].copy_from_slice(&tmp[i..]);
    &buf[..len]
}
