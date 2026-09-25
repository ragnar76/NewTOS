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
use super::global::{self, Grect, Point, DROOT, MAX_DEPTH, NIL, NUM_WNODES, WINDOW_STYLE};
use super::menu::{self, Item, Title};
use super::model::{AType, View};
use super::win::{DESKWH, SliderSet, WA_DNLINE, WA_DNPAGE, WA_LFLINE, WA_RTLINE, WA_UPLINE, WA_UPPAGE};
use super::{dir, ftype, pro, redraw};
use super::{
    wind_get_grect, wind_get_top, wind_get_work, FLATHAND, MU_BUTTON, MU_KEYBD, MU_MESAG, MU_TIMER,
    M_OFF, M_ON, WF_CXYWH, WF_FXYWH, WF_HSLIDE, WF_HSLSIZ, WF_INFO, WF_NAME, WF_PXYWH, WF_TOP,
    WF_VSLIDE, WF_VSLSIZ,
};

// AES message ids (`aesdefs.h:227-261`).
const MN_SELECTED: i16 = 10;
const WM_REDRAW: i16 = 20;
const WM_TOPPED: i16 = 21;
const WM_CLOSED: i16 = 22;
const WM_FULLED: i16 = 23;
const WM_ARROWED: i16 = 24;
const WM_HSLID: i16 = 25;
const WM_VSLID: i16 = 26;
const WM_SIZED: i16 = 27;
const WM_MOVED: i16 = 28;

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
        // Double-click: select just the hit item, then open it.
        let obid = gr_obfind(root, mx, my);
        global::with(|g| g.act_bsclick(handle, root, obid, false, true));
        redraw_window(handle);
        do_open_selected();
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
    super::evnt_multi_m1(out, x, y, w, h) & super::MU_BUTTON == 0
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
/// the state change plus, when it actually changed something, the repaint of
/// the object's own extent clipped to `pc`.
///
/// Split here because [`global::with`] is not re-entrant and
/// [`redraw::do_wredraw`] takes the desktop lock itself: the `DeskState` half
/// is [`global::DeskState::act_chg`], the draw half is this wrapper.
fn act_chg_draw(wh: i16, root: i16, obj: i16, pc: Grect, set: bool) {
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
                act_chg_draw(curr_wh, curr_root, curr_sel, pc, false);
                curr_wh = 0;
                curr_root = 0;
                curr_sel = 0;
                break 'body;
            }
            // `:325-333` — the target changed: take the old one back first.
            if dobj != curr_sel && curr_sel != 0 {
                act_chg_draw(curr_wh, curr_root, curr_sel, pc, false);
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
            act_chg_draw(curr_wh, curr_root, curr_sel, pc, true);
        }

        if !down {
            break;
        }
    }

    // `:353-354` — the highlight always comes back off at the end.
    if curr_sel != 0 {
        act_chg_draw(curr_wh, curr_root, curr_sel, pc, false);
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
    /// A drive icon: the drive letter.
    Drive(u8),
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

/// Open the item object `obid` of window `handle`: a folder opens in a new
/// directory window, a drive icon at its root, a native `.NAP`/`.NCP`/`.NTP`
/// program launches (ADR-018), and a classic m68k binary raises an honest
/// "needs the emulator" alert. A document is a no-op (association is future).
fn open_item(handle: i16, obid: i16) {
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
                        OpenTarget::Drive(letter)
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
                return;
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
        OpenTarget::Folder(spec, title) => do_fopen(handle, obid, &spec, &title, true),
        OpenTarget::Drive(letter) => open_root_window(letter),
        OpenTarget::App { path, name, flavor } => {
            // `do_aopen`'s `else` arm (`desksupp.c:903-912`): the program is
            // not an installed application, so the **desktop** config flag
            // `G.g_appdir` picks the directory. A failure aborts the launch.
            let appdir = global::with(|g| g.g_appdir).unwrap_or(true);
            if set_launch_dir(appdir, &path) {
                launch_native(&path, &name, flavor, &[]);
            }
        }
        OpenTarget::M68k => {
            let text = b"[1][ This is an m68k Atari program. | The m68k emulator is not | available yet. ][ OK ]\0";
            super::form_alert(1, text.as_ptr());
            // No desktop repaint: the AES `form_alert` bb_save/restores the screen
            // under the alert (EmuTOS `fm_alert`), so the desktop returns intact.
        }
        OpenTarget::Doc { name, full_path } => open_document(&name, &full_path),
        // Resolved into one of the arms above by the prologue.
        OpenTarget::Shortcut { .. } | OpenTarget::None => {}
    }
}

/// Open a document (deskins #20 / EmuTOS `do_aopen`'s `installed_datafile`
/// branch, `desksupp.c:924-951`): resolve the installed application association
/// for the document's type and, if one exists, launch that application with the
/// document as its command tail. With no association installed, report it
/// honestly (EmuTOS would offer Show/Print or alert `STNOAPPL`; NewTOS raises
/// the "no application" alert — Show/Print is a separate, unimplemented feature).
///
/// The command tail is the classic Atari `[len][args][\0][\r]` image: the
/// association's fixed argument prefix (`a_pargs`), then either the document's
/// full path (`AF_ISFULL`) or just its file name (`deskins.md` §`ins_app`,
/// `desksupp.c:942-950`). The associated application's own extension selects the
/// launch behaviour (console screen setup etc.), exactly as a direct
/// double-click would.
fn open_document(name: &str, full_path: &str) {
    let Some(assoc) = super::inf::lookup_association(name.as_bytes()) else {
        let text = b"[1][ No application is installed | for this document type. ][ OK ]\0";
        super::form_alert(1, text.as_ptr());
        // AES `form_alert` bb_save/restores the screen — no desktop repaint.
        return;
    };

    // `do_aopen`'s `if (is_installed(pa))` arm (`desksupp.c:896-902`): an
    // installed application answers the directory question from its **own**
    // `AF_APPDIR` flag, not from the desktop-wide `G.g_appdir`. A failure
    // aborts the launch, as it does for a direct one.
    if !set_launch_dir(assoc.app_dir, &assoc.app_path) {
        return;
    }

    // The associated application must itself be a native NewTOS program.
    let app_name = filename_of(&assoc.app_path);
    let flavor = match ftype::classify(app_name.as_bytes()) {
        ftype::FileType::Native(f) => f,
        _ => {
            let text = b"[1][ The associated application is | not a native NewTOS program. ][ OK ]\0";
            super::form_alert(1, text.as_ptr());
            // AES `form_alert` bb_save/restores the screen — no desktop repaint.
            return;
        }
    };

    // Build the command tail: fixed args + full path or file name.
    let mut tail_str = String::from(assoc.args.as_str());
    tail_str.push_str(if assoc.full_path { full_path } else { name });
    let mut tailbuf = [0u8; 128];
    let taillen = build_cmd_tail(&mut tailbuf, &tail_str);

    // NUL-terminate the application path for the loader.
    let mut app_path = String::from(assoc.app_path.as_str());
    app_path.push('\0');
    launch_native(&app_path, &app_name, flavor, &tailbuf[..taillen]);
}

/// `do_aopen(pa, TRUE, curr, pathname, pname, tail)` (`desksupp.c:843-1031`)
/// for the single case `fun_win2win`'s drop-on-an-application branch reaches
/// (`deskfun.c:967`): `isapp` is `TRUE` and `tail` is **not** `NULL`, so
/// neither the `installed_datafile` arm (`:924`, `isapp` rules it out) nor
/// `opn_appl`'s `.TTP` parameter prompt (`:978-985`, the `tail` branch takes
/// precedence) runs. What is left is three steps:
///
/// * the current directory, set exactly as `do_aopen` sets it (`:873-890`):
///   from the node's own `AF_APPDIR` + `a_pappl` when it is an **installed**
///   application, otherwise from the desktop-wide `G.g_appdir` and the
///   **destination window's** `p_spec` — that is [`set_launch_dir`], shared
///   with the double-click and document paths;
/// * `app_path = pathname` with its file-name part replaced by `pname`
///   (`:958-960`) — here `dst_dir` (which already ends in `\`) plus
///   `prog_name`;
/// * `strcpy(ptail, tail)` and `pro_run` (`:969-975`, `:986`).
///
/// `installed`/`pappl`/`node_appdir` are the three fields of the destination
/// `ANODE` those steps read; `tail` is the command line
/// [`dir::build_selected_path`] built.
///
/// Returns EmuTOS's `exit_desktop`. NewTOS launches **nested** instead of
/// leaving the desktop process (AES gap G1, see [`launch_native`]), so the
/// answer is always `false` — there is no desktop to exit, and the desktop is
/// back on screen by the time this returns.
pub(super) fn do_aopen_drop(
    installed: bool,
    pappl: &str,
    node_appdir: bool,
    dst_dir: &str,
    prog_name: &str,
    tail: &str,
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
            let mut tailbuf = [0u8; 128];
            let taillen = build_cmd_tail(&mut tailbuf, tail);
            app_path.push('\0'); // the loader reads up to the first NUL
            launch_native(&app_path, prog_name, flavor, &tailbuf[..taillen]);
        }
        ftype::FileType::M68k => {
            let text = b"[1][ This is an m68k Atari program. | The m68k emulator is not | available yet. ][ OK ]\0";
            super::form_alert(1, text.as_ptr());
        }
        // Reachable, and an EmuTOS quirk rather than an accident: an installed
        // application's `ANODE` carries `a_aicon = IG_APPL` **and** its
        // document wildcard, and `app_afind_by_name` tests `a_pdata` first
        // (`deskapp.c:1450-1454`), so a document of that type answers
        // `a_aicon >= 0` too and EmuTOS tries to run the document itself.
        ftype::FileType::Doc => {
            let text = b"[1][ This file is not a native | NewTOS program. ][ OK ]\0";
            super::form_alert(1, text.as_ptr());
        }
    }
    false
}

/// Build the classic Atari command-tail image into `buf` and return its length:
/// `buf[0]` = argument length, `buf[1..]` = the argument characters, then a NUL
/// and a trailing `\r` (EmuTOS `pro_run`, `deskpro.c:56-57`; the loader appends
/// the final NUL into `p_cmdlin`). The argument string is capped so the whole
/// image fits the 128-byte `p_cmdlin`.
fn build_cmd_tail(buf: &mut [u8; 128], s: &str) -> usize {
    let bytes = s.as_bytes();
    let n = bytes.len().min(124);
    buf[0] = n as u8;
    buf[1..1 + n].copy_from_slice(&bytes[..n]);
    buf[1 + n] = 0; // NUL after the arguments
    buf[2 + n] = 0x0D; // trailing CR, just like Atari TOS
    3 + n
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

/// [`doc_full_path`] plus a trailing `\0`, ready to hand to
/// [`crate::launch::run_program`] (which reads up to the first NUL).
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

/// Launch a native NewTOS program (ADR-018) and repaint the desktop on return.
///
/// The launch primitive is [`crate::launch::run_program`], which loads the ELF
/// from the `C:` FAT16 path, drops to EL0, runs it, and returns on `Pterm`.
/// The flavour selects the surrounding behaviour:
///
/// * [`ftype::Flavor::App`] (`.NAP`) — run directly; the app draws its own AES
///   UI over the desktop, so nothing special is needed around the call.
/// * [`ftype::Flavor::Console`] (`.NCP`) — bracket the run with the AES screen
///   mode switch, [`crate::shell_loop::sh_alpha_enter`] /
///   [`crate::shell_loop::sh_alpha_leave`], i.e. EmuTOS' `sh_toalpha` /
///   `sh_tographic` (`aes/gemshlib.c:246-260,223-239`): the pointer goes away
///   and the screen is cleared before the child's output starts at the top
///   left, and the mouse comes back afterwards (DESK22). The desktop is then
///   repainted over the text by the trailing [`redraw::redraw_desktop`]
///   (whole-screen recovery — the console cleared the screen). `run_program`
///   stays pure: no console/redraw coupling inside it.
/// * [`ftype::Flavor::Params`] (`.NTP`) — a direct double-click runs it with an
///   empty tail (`tail == &[]`, Stage A). When launched as a document's
///   associated application (deskins #20), `tail` carries the document path.
///
/// `tail` is the command-tail image handed to the child's `p_cmdlin` (empty for
/// a plain double-click; the document tail for an association launch). A
/// load/launch failure (`run_program` → `false`) raises an alert. The desktop is
/// repainted on return in every case (it overpaints the console text, the app's
/// UI, or the alert).
fn launch_native(path: &str, name: &str, flavor: ftype::Flavor, tail: &[u8]) {
    match flavor {
        ftype::Flavor::Accessory => {
            // A desk accessory (`.NAC`) is **not** launched on demand (ACC02):
            // the AES loads it once per session from `C:\ACC\` or the root and
            // it lives for the whole session. EmuTOS's desktop has no launch
            // path for an `.ACC` either — `count_accs`/`load_accs`
            // (`aes/geminit.c:264-315`) is the only way one ever starts. Say so
            // rather than start a second, unreachable copy of it.
            alert_accessory_not_launchable(name);
        }
        ftype::Flavor::App | ftype::Flavor::Params => {
            // GEM app (`.NAP`) / takes-parameters (`.NTP`): both draw their own
            // AES UI directly over the screen, so they go through the
            // single-tasking screen handoff (F5) — [`run_gem_app`] relinquishes
            // the desktop before the child and rebuilds it on the child's
            // `Pterm`. A load/launch failure surfaces as an alert on the
            // (already rebuilt) desktop.
            if !run_gem_app(path.as_bytes(), tail) {
                alert_launch_failed(name);
                // `run_gem_app` already rebuilt the desktop (`desktop_resume`); the
                // AES `form_alert` bb_save/restores over it — no further repaint.
            }
        }
        ftype::Flavor::Console => {
            // Console (`.NCP`): the AES screen-mode switch a character-mode
            // program gets in EmuTOS — `pro_run(isgraf = FALSE, …)` →
            // `shel_write` → `sh_main`'s `sh_chgrf` → `sh_toalpha`
            // (`aes/gemshlib.c:513-523,246-260`), which puts the arrow shape
            // back and **removes the mouse pointer** (`ratexit()`, `:256`)
            // before the text screen appears. Clearing the screen alone
            // (`fbcon::clear_and_home`, which `sh_toalpha` still does at its
            // end) left the pointer standing in the text as a black smudge —
            // DESK22.
            crate::shell_loop::sh_alpha_enter();
            let ran = crate::launch::run_program_with_tail(path.as_bytes(), tail);
            // Classic TOS keeps the console up until the user acknowledges, so
            // the program's final output stays readable before the desktop
            // repaints over it. Only reached from a real double-click (an
            // interaction path that requires HID input — verified on hardware).
            console_pause_for_key();
            // Back into graphics mode — `sh_main`'s next pass (`:712-713`),
            // `sh_tographic` (`:223-239`): mouse on, hourglass while the
            // desktop is rebuilt.
            crate::shell_loop::sh_alpha_leave();
            if !ran {
                // EmuTOS shows the failure alert on the next pass too, right
                // after the mode switch and before the desktop is drawn
                // (`fm_error`, `gemshlib.c:722-723`).
                alert_launch_failed(name);
            }
            // The console program cleared the whole screen: genuine whole-screen
            // recovery of the desktop (background + windows + menu bar).
            redraw::redraw_desktop();
            // …and the arrow back, which in EmuTOS is the last act of the
            // restarted desktop's start-up (`desk_busy_off`,
            // `deskmain.c:2071`) undoing `sh_tographic`'s hourglass. Same step,
            // same place as after the nested EmuCON pass (`desk::run`'s loop).
            pro::desk_busy_off();
        }
    }
}

/// Launch a GEM application: the EmuTOS desktop *epilog* → child → *prolog*
/// sequence, run **nested** inside the desktop's event loop.
///
/// **This is not how EmuTOS launches a program, and the difference is AES gap
/// G1.** In EmuTOS the desktop calls `shel_write(SHW_EXEC, …)` (`deskpro.c:41`,
/// AES opcode 121), `hndl_button` gets TRUE back and sets `done`
/// (`deskmain.c:988`), `deskmain` runs its epilog and **returns**, the desktop
/// process terminates, and the AES shell loop `sh_main` (`gemshlib.c:708-731`)
/// does the rest: `wm_init()`, `ratinit()`, `sh_draw(D.s_cmd, TRUE)`, then
/// `sh_ldapp` loads the child. When the child `Pterm`s, `sh_main` loops and
/// re-enters the desktop from the top. NewTOS has that shell loop
/// ([`crate::shell_loop::sh_main`], which already performs every one of those
/// steps) — but [`crate::desk::run`] never returns, so the desktop can never
/// hand control back to it and instead calls the loader itself.
///
/// * **relinquish** ([`desktop_relinquish`], before the child): the EmuTOS
///   desktop epilog — `cnx_put()`, `menu_bar(NULL,0)`, `wind_set(DESKWH,
///   WF_NEWDESK, NULL)`, `appl_exit()` (`deskmain.c:2152-2166`) — **plus** the
///   `gl_shgem` block of the `sh_main` pass that loads the program,
///   `wm_init(); ratinit(); sh_draw(D.s_cmd, TRUE)` (`gemshlib.c:715-720`,
///   DESK35), so the program gets an empty screen and an empty window table;
/// * **show** ([`crate::shell_loop::sh_gem_show`], still before the child):
///   `sh_ldapp`'s `sh_show(fname)` (`gemshlib.c:632`, DESK37) — the AES draws
///   its whole `DESKTOP` tree back over that white, so what stands behind the
///   program is the `#Q` background and a menu-bar strip with its name;
/// * **resume** ([`desktop_resume`], after the child's `Pterm`): `wm_new()`,
///   then `deskmain`'s prolog — `appl_init()`, `desk_verify(DESKWH)`,
///   re-register the background tree, `cnx_get()` to re-open the windows,
///   `men_update()`, repaint.
///
/// Returns whether the child launched (`run_program` → `true`).
fn run_gem_app(path: &[u8], tail: &[u8]) -> bool {
    desktop_relinquish();
    // `sh_show(fname)` (`gemshlib.c:632`) — `sh_ldapp`'s own step, between the
    // `sh_main` block [`desktop_relinquish`] ends with and the `dos_exec` on
    // the next line (`:645`). It undoes the white of `sh_draw(…, TRUE)`: the
    // whole built-in `DESKTOP` tree with its real `ob_spec`, i.e. the `#Q`
    // background and the menu-bar strip carrying the program's name (DESK37).
    // Same order as EmuTOS — clear, then draw, then load.
    crate::shell_loop::sh_gem_show(path);
    let ran = crate::launch::run_program_with_tail(path, tail);
    desktop_resume();
    ran
}

/// The **relinquish** half of [`run_gem_app`]: the EmuTOS desktop *epilog*
/// (`deskmain.c:2152-2166`), followed by the graphics half of the `sh_main`
/// pass that starts the program (`gemshlib.c:715-720`,
/// [`crate::shell_loop::sh_gem_enter`]).
///
/// **The screen the program gets is empty, and its window table is empty**
/// (DESK35). Before that, the desktop's windows stayed both painted and
/// registered: a GEM application draws only into its own rectangles, so the
/// leftovers sat there as pixels with no event loop behind them and as AES
/// windows the program's own had to composite against. The single-tasking
/// decision is that a launched application owns the **whole** screen.
fn desktop_relinquish() {
    // `cnx_put()` — the first statement of the EmuTOS epilog
    // (`deskmain.c:2154`), so the window layout the desktop is about to give up
    // is in `G.g_cnxsave`. Its `app_save(FALSE)` companion, which parks the
    // whole INF image in the AES shell buffer, has no counterpart yet: NewTOS's
    // `NEWDESK.INF` goes to and from the file directly, with no
    // `shel_get`/`shel_put` memory image in between.
    //
    // This is what [`desktop_resume`]'s `cnx_get()` reads the windows back
    // from, exactly as EmuTOS's restarted `deskmain` does (`deskmain.c:2055`):
    // since DESK35 the `wm_init()` below deletes them for real, so the context
    // save area is the *only* place their paths, rectangles and scroll
    // positions survive the program.
    cnx_put();
    // Remove the desktop menu bar (EmuTOS `menu_bar(NULL,0)`). Our AES
    // `menu_bar(tree, 0)` needs the real (non-null) tree to erase the strip.
    super::menu_bar(menu::tree_ptr(), 0);
    // Un-register the desktop background tree (EmuTOS
    // `wind_set(DESKWH, WF_NEWDESK, NULL, 0, 0)`, `deskmain.c:2163`) so the AES
    // `w_drawdesk` falls back to its own built-in DESKTOP root
    // (`gemwmlib.c:340-359,1268-1271`).
    super::wind_set_newdesk(0, 0);
    // The desktop-side `WNODE`s go with the AES handles the `wm_init` below
    // deletes. EmuTOS gets this for free — its desktop *process* ends and the
    // restarted one rebuilds the array with `win_start()` (`deskmain.c:2028`,
    // `deskwin.c:105-135`) — but ours resumes in place, so the nodes have to be
    // released explicitly, and through `win_free` so each window's item
    // `OBJECT`s go back to the `g_screen` free chain (`obj_wfree`,
    // `deskwin.c:148`) instead of leaking one window's worth per launch.
    win_free_all();
    // `wm_init(); ratinit(); sh_draw(D.s_cmd, TRUE);` — the `gl_shgem` block of
    // the `sh_main` pass that loads the program (`gemshlib.c:715-720`). It runs
    // *before* the `appl_exit()` below rather than after it; the reason is in
    // [`crate::shell_loop::sh_gem_enter`] (the sweep queues `WM_REDRAW`s that
    // only the drain may be allowed to see).
    crate::shell_loop::sh_gem_enter();
    // Leave the AES — EmuTOS `appl_exit()` (`deskmain.c:2166`), the last step of
    // the desktop epilog and the first of the two state resets at this boundary.
    // `ap_exit` (`gemaplib.c:251-261`) **drains the message queue** and clears
    // `AP_OPEN`; it does not destroy the descriptor (A29). That drain is what
    // stops the child inheriting the desktop's unread `WM_*` — desktop and first
    // child share pid 1 here exactly as EmuTOS's successive programs share `rlr`
    // (`gemshlib.c:633-636`), so without it the child's first `evnt_mesag` hands
    // it a `WM_REDRAW` for a window it does not own (A28). Last, as in EmuTOS:
    // the step above must be able to queue, this one must not be followed by
    // anything that can. The second reset is `rlr->p_flags = 0`, done where
    // EmuTOS does it — at the handover in `tos_api::aes::register_process`.
    super::appl_exit();
}

/// The **resume** half of [`run_gem_app`]: rebuild the desktop exactly as it was
/// before the launch (EmuTOS `deskmain.c:2028-2074` + `gemshlib.c:606,668-677`),
/// in `deskmain`'s own prolog order.
fn desktop_resume() {
    // The shell loop's sweep after the program — `sh_ldapp`: `unsync
    // (&wind_spb)` + `wm_new()` (`gemshlib.c:668-669,676-677`), which in EmuTOS
    // runs between the child's `Pterm` and the desktop's restart. It is a
    // shell-loop step, not a desktop one (like everything in this function,
    // gap G1), and the built-in desktop's launch path bypasses the shell loop,
    // so it has to be taken here or not at all (A96). Since DESK35 it is the
    // **plain** `wm_new`: `desktop_relinquish`'s `wm_init` already took this
    // desktop's own windows, so what is left to sweep is the program's, and
    // any a `Pexec` descendant outlived it with — closed, deleted, locks
    // released, `WF_NEWDESK` tree dropped.
    crate::shell_loop::sh_gem_leave();
    // Re-enter the AES — EmuTOS `deskmain.c:1953`, `gl_apid = appl_init();`,
    // the first statement of the desktop it re-enters after every program
    // (`sh_main` → `sh_ldapp`'s DESKTOP arm → `deskstart`,
    // `gemshlib.c:600-612,710-727`). The counterpart of the `appl_exit()` in
    // [`desktop_relinquish`]: without it the resumed desktop would run with
    // `AP_OPEN` clear, which is not a state EmuTOS's running desktop is ever
    // in. `ap_init` only ORs the flag back on — descriptor, name and queue are
    // untouched (`gemaplib.c:54-63`, A25) — so this restores a flag, it does
    // not undo the relinquish's drain.
    super::appl_init();
    // `desk_verify(DESKWH, FALSE)` (`deskmain.c:2035`) — "initialise g_croot,
    // g_cwin, g_wlastsel". There is no window left to be current: `wm_init`
    // deleted them all and `cnx_get` below has not re-opened any yet. (The
    // desk rectangle is read first: `global::with` takes the desktop lock and
    // is not re-entrant.)
    let desk = g_desk();
    global::with(|g| g.desk_verify(DESKWH, desk, false));
    // Re-register the desktop's background object tree (EmuTOS
    // `wind_set(DESKWH, WF_NEWDESK, G.g_screen, 1, 0)`, `deskmain.c:2049`).
    // The AES `w_drawdesk` draws `DROOT` + icons again from here.
    if let Some(tree) = super::global::screen_addr() {
        super::wind_set_newdesk(tree, super::global::DROOT);
    }
    // `cnx_get()` (`deskmain.c:2055`) — "establish desktop's state from info
    // found in app_start, open windows". This is the half of the EmuTOS prolog
    // that used to be missing (AES gap G2, now closed by DESK35): the windows
    // `desktop_relinquish`'s `cnx_put()` recorded are re-opened here, in
    // z-order from slot 0 up, each at its saved rectangle, path and scroll
    // position. EmuTOS brackets it in `wind_update(BEG_UPDATE/END_UPDATE)`
    // (`:2054,2056`); the desktop is the only AES client alive here, so the
    // bracket has nothing to keep out and the whole-screen repaint below
    // covers what the individual `wind_open`s drew.
    cnx_get();
    // `men_update()` (`deskmain.c:2069`) — the File/View items that depend on
    // how many windows are open and what is selected in them.
    menu::men_update();
    // App-return recovery: the child cleared/overdrew the whole screen, so rebuild
    // the desktop background + windows + menu bar (`redraw_desktop` re-installs the
    // menu bar itself, EmuTOS `menu_bar(menutree, 1)`, `deskmain.c:2073`). Not a
    // per-event shortcut — a genuine whole-screen restore.
    redraw::redraw_desktop();
}

/// Release every open desktop `WNODE` — `win_free` per window
/// (`deskwin.c:141-150`), which is also the `obj_wfree` that returns the
/// window's item `OBJECT`s to the `g_screen` free chain (`:148`).
///
/// EmuTOS has no such function because it never needs one: the desktop's
/// `WNODE` array belongs to the desktop *process*, which ends, and the
/// restarted desktop allocates a fresh one in `win_start()`
/// (`deskmain.c:2028`). NewTOS's desktop resumes in place, so the array it
/// keeps must be emptied by hand when the AES handles in it are about to be
/// deleted ([`crate::shell_loop::sh_gem_enter`]'s `wm_init`). No AES call is
/// made here — the windows are closed and deleted by that `wm_init`, exactly
/// as in EmuTOS.
fn win_free_all() {
    let (handles, n) = open_handles();
    for &handle in &handles[..n] {
        global::with(|g| g.win_free(handle));
    }
}

/// Keep the VT52 console up after a `.NCP` program returns and block until the
/// user presses a key, then let the caller repaint the desktop — the classic-TOS
/// "the program's output stays on screen until acknowledged" affordance.
///
/// The prompt renders through the same `fbcon` console the program wrote to
/// (still active on return); the blocking read reuses the EmuCON shell's console
/// input source ([`crate::host_env`] `console_read_key_blocking` → IKBD / USB
/// HID). That input path exists only on hardware (and interactive QEMU), which
/// is also the only place a `.NCP` can be double-clicked — so this never stalls
/// a headless boot (the `\AUTO\` path does not go through here).
fn console_pause_for_key() {
    for &b in b"\r\n[ Press any key to return to the desktop ]" {
        crate::fbcon::write_byte(b);
    }
    if let Some(h) = tos_api::host::host() {
        let _ = h.console_read_key_blocking();
    }
}

/// Raise a `form_alert` reporting that `name` could not be launched (an
/// `ENSMEM` / bad-format / not-found from the loader).
fn alert_launch_failed(name: &str) {
    let mut buf = [0u8; 128];
    let mut p = 0usize;
    for part in [b"[1][ Could not launch ".as_slice(), name.as_bytes(), b". ][ OK ]"] {
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

/// Alert for a double-clicked desk accessory (`.NAC`): it is already loaded (or
/// was not found at boot) and cannot be started a second time — see
/// [`launch_native`]'s `Accessory` arm. EmuTOS has no message for this because
/// its desktop cannot reach the case at all; ours can, so it says what happened
/// instead of doing nothing.
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
/// `do_aopen` is reached with `isapp = TRUE` and `tail = NULL`, so neither the
/// `installed_datafile` arm (`desksupp.c:924`, ruled out by `isapp`) nor a
/// document tail applies; what is left is the directory step, the rejoin and
/// the launch. The one EmuTOS branch this cannot reach is `opn_appl`'s
/// parameter prompt for a `.TTP`/`.GTP` (`:978-985`), which NewTOS does not
/// have for `.NTP` either — a `.NTP` launched from a key runs with the same
/// empty tail a double click gives it ([`launch_native`], Stage A). No new gap:
/// the same named one, now reachable from a second place.
///
/// `curr` is `-1` in this call, which in EmuTOS only suppresses `pro_run`'s
/// `do_wopen` re-open of the desktop window (`deskpro.c:61-62`); NewTOS launches
/// nested and repaints on return, so there is nothing to suppress.
fn process_funkey(funkey: u16) -> Option<bool> {
    let pa = super::inf::app_by_funkey(funkey as u8)?;
    let pname = filename_of(&pa.app_path);
    let dir = &pa.app_path[..pa.app_path.rfind('\\').map_or(0, |i| i + 1)];
    // `do_aopen(pa, TRUE, -1, pathname, pfname, NULL)` — an **installed**
    // application, so the directory comes from its own `AF_APPDIR`/`a_pappl`.
    Some(do_aopen_drop(true, &pa.app_path, pa.app_dir, dir, &pname, ""))
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

/// `check_alt_letter_key` (EmuTOS `deskmain.c:1136`) — Alt+A..Z opens that
/// drive letter. Only fires for a key whose ASCII byte is genuinely `0` (a bare
/// scancode), reading the shifted character for the scancode to infer the
/// letter (EmuTOS reads `KEYTAB.shift[]`; there is no dedicated Alt table).
///
/// **This path is live** (re-established DESK13). An older note here, and the
/// one DESK11 copied into its report, called it inert on the grounds that the
/// keyboard layer "folds Alt into AltGr and never yields ASCII 0 for a letter
/// key". That is not what the layer does: `crate::keymap::to_ascii` zeroes an
/// alphabetic result whenever the Alt modifier is set
/// (`kernel/src/keymap.rs`, the `if ascii.is_ascii_alphabetic()` arm of the
/// `alt` branch), `ikbd::to_ascii` turns that `0` into `None`, and
/// `ikbd::decode_key` queues the word as `scancode << 8 | 0` — exactly the
/// bare-scancode shape this check wants. Both key sources set the Alt bit: the
/// native IKBD stream (`SC_ALT` = `0x38`) and the USB-HID report, whose
/// `modifiers()` maps LAlt **and** RAlt to `0x38`. The AES hands the whole word
/// through as `EV_KRET` (`tos-api/src/aes/event.rs:598`), so nothing between
/// the key and here re-interprets it.
///
/// Returns `true` only if it actually opened a drive.
fn check_alt_letter_key(key: i16) -> bool {
    if (key & 0xFF) != 0 {
        return false;
    }
    let sc = ((key >> 8) & 0xFF) as u8;
    let letter = crate::keymap::to_ascii(sc, true, false, false);
    if letter.is_ascii_uppercase() {
        let drive = letter - b'A';
        // `if (dos_sdrv(dos_gdrv()) & (1L << drv))` — EmuTOS `valid_drive`
        // (`desksupp.c:1819`), the same GEMDOS drive-map read as `app_blddesk`.
        if dos_sdrv(dos_gdrv()) & (1 << drive) != 0 {
            open_root_window(letter);
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
        Title::Options => {
            do_optnmenu(item);
            false
        }
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
        Item::Open => do_open_selected(),
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
/// Returns `done` for the caller, and — because NewTOS's desktop cannot
/// terminate on it — that flag is consumed by [`super::run`]'s loop rather
/// than by the AES shell loop. See
/// [`crate::shell_loop::sh_run_pending_console`] for the gap and its reason.
fn do_execcli() -> bool {
    // `G.g_work[1] = '\0';` — an empty command tail. Byte 0 is the length
    // byte `pro_run` fills in, byte 1 the first text byte.
    let mut tail = [0u8; pro::CMDTAILSIZE];
    let done = pro::pro_run(false, pro::DEF_CONSOLE, &mut tail);

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

/// `do_optnmenu` (EmuTOS `deskmain.c:786`) — the Options menu. Live items: Set
/// background (`inf_backgrounds`), Install Application (deskins #20), Read .INF
/// (reload + apply), Save desktop (`app_save`). The remaining preferences /
/// resolution items stay disabled by `men_update` (deskrez is config.txt-driven,
/// ADR-017 rev 2).
fn do_optnmenu(item: Item) {
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
        Item::ReadInf => do_read_inf(),
        Item::SaveDesktop => do_save_desktop(),
        _ => {} // remaining Options items stay disabled
    }
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
        // all (see [`launch_native`]) — but the enum must be total. GEM is the
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
            if g.g_screen[child as usize].ob_state & super::win::OB_SELECTED != 0 {
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

/// `READITEM` (`deskmain.c:919-925`) — *Options → Read .INF file*: run
/// [`super::inf::app_read_inf`] (prompt, load, validate, alert on failure) and,
/// if it read a file, put the new settings into effect.
///
/// **Deviation — no desktop restart** (`TASK/DESK14.md`, lead decision
/// 2026-09-23, point 4). EmuTOS answers a successful read with
/// `restart_desktop = TRUE; done = TRUE;`: `deskmain` tears its allocations
/// down (`:2136-2148`) and `_deskstart` immediately re-enters it, so the new
/// file is read exactly the way the boot path reads one. NewTOS's desktop is
/// still a kernel call, not a process `sh_main` can re-enter — `global::init`
/// is idempotent, there is no `win_start()` on re-entry, and the
/// `cnx_put(); app_save(FALSE)` → `shel_get` bridge (`deskmain.c:2152-2153`)
/// does not exist — so the settings are pulled into the *running* desktop
/// instead: the block below is what the restart would otherwise have done.
/// The restart path belongs to the move of the desktop into user scope and is
/// deferred; do not work around its absence here.
fn do_read_inf() {
    if !super::inf::app_read_inf() {
        return;
    }
    // The re-read `#E` bytes land in `G.g_cnxsave`; push their preference half
    // into the live desktop (see [`cnx_get_prefs`] for why not the whole
    // `cnx_get`).
    cnx_get_prefs();
    // The re-read `#K` line landed in `menu_shortcuts[]`; render it into the
    // menu strings. EmuTOS gets this for free — the restart rebuilds the whole
    // desktop from `deskmain`, `install_shortcuts()` included
    // (`deskmain.c:920-926`, `:2023`) — whereas NewTOS re-applies in place (see
    // the deviation above), so it has to say so.
    menu::install_shortcuts();
    global::with(|g| {
        g.win_view();
        g.app_blddesk();
        g.win_srtall();
    });
    win_bdall();
    // Read .INF rebuilt the desktop icon tree + every window's view (background,
    // icon grid, sort may all have changed): a genuine whole-desktop repaint.
    redraw::redraw_desktop();
}

/// Options → Save desktop: snapshot the session context, then serialise it to
/// `NEWDESK.INF` — EmuTOS `SAVEITEM`: `desk_busy_on(); cnx_put(); app_save(TRUE);
/// desk_busy_off();` (`deskmain.c:888-893`). Reports failure through an alert.
fn do_save_desktop() {
    cnx_put();
    if !super::inf::save_desktop() {
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

/// File → Open: open the single selected item of the current window (a folder
/// in a new window, a drive icon at its root). Enabled only when exactly one
/// item is selected (`men_update`).
fn do_open_selected() {
    let picked = global::with(|g| {
        let root = g.g_croot;
        let handle = g.g_cwin;
        let mut child = g.g_screen[root as usize].ob_head;
        while child != NIL && child != root {
            if g.g_screen[child as usize].ob_state & super::win::OB_SELECTED != 0 {
                return Some((handle, child));
            }
            child = g.g_screen[child as usize].ob_next;
        }
        None
    })
    .flatten();
    if let Some((handle, obid)) = picked {
        open_item(handle, obid);
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
        navigate_window(handle, &pspec, &ptitle);
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
        // `do_diropen(pw, TRUE, obid, pws->pth_save, (GRECT *)pws, TRUE)`.
        let (spec, title) = spec_and_title(&path);
        open_dir_window_at(&spec, &title, Some(gr), cvcol, cvrow);
        opened += 1;
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

/// Open a directory window on drive `letter`'s root (`X:\`).
pub fn open_root_window(letter: u8) {
    let mut spec = String::new();
    spec.push(letter as char);
    spec.push_str(":\\*.*");
    let mut title = String::new();
    title.push(letter as char);
    title.push_str(":\\");
    open_dir_window(&spec, &title);
}

/// `do_fopen(pw, curr, pathname, allow_new_win)` (EmuTOS
/// `desksupp.c:1089-1160`) — open the folder `spec` **for the window `handle`**:
/// normally *in that same window*, in a new one only in the cases EmuTOS lists
/// in the function's own header comment (`:1084-1088`).
///
/// ```c
/// if (allow_new_win) {
///     graf_mkstate(&junk, &junk, &junk, &keystate);
///     if ((keystate & MODE_SCA) == MODE_ALT)      /* :1130-1135 */
///         new_win = TRUE;
/// }
/// if (new_win) { pw = win_alloc(curr); rc_copy(…, &t); }  /* :1141-1149 */
/// else         { pn_close(&pw->w_pnode); }                /* :1151-1154 */
/// do_diropen(pw, new_win, curr, app_path, &t, TRUE);      /* :1156 */
/// ```
///
/// * `new_win == FALSE` → `pn_close` frees the window's file list and
///   `do_diropen(pw, FALSE, …)` re-fills **the same** `WNODE`: same frame, new
///   path in the title, no `wind_open`, and a direct `WM_REDRAW` of the work
///   area (`:453-459`). That is [`navigate_window`], which is also what the
///   Close-folder "up one level" navigation uses — the same EmuTOS body
///   (`w_setpath` → `do_fopen(pw, 0, pathname, FALSE)`, `deskfun.c:824`).
/// * `new_win == TRUE` → `win_alloc` + `do_diropen(pw, TRUE, …)`, i.e. a
///   `wind_open` of a freshly allocated node at **its own** default rectangle
///   (`rc_copy` at `:1148` copies the new node's rect *into* `t`, so the old
///   window's geometry is discarded). That is [`open_dir_window`].
///
/// **Two of EmuTOS's three new-window cases reach this function; the third is
/// a separate caller.** A drive icon is `do_dopen` (`:1041-1069`), which always
/// allocates — [`open_root_window`], unchanged. The other two are here: the
/// Alt-held case above, and the `WN_DESKTOP` case (`:1112-1127`,
/// `CONF_WITH_DESKTOP_SHORTCUTS`, DESK30) — a **folder shortcut** opened from
/// the desktop surface, which always gets a new window because the "window" it
/// was opened from is the desktop pseudo-window and has no file list to
/// replace:
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
///
/// Not reproduced: `do_wopen`'s `graf_growbox` zoom from the source icon to the
/// window rect and its `act_chg(…, FALSE, FALSE)` deselect of that icon
/// (`:294-322`). Neither is implemented on any NewTOS window-open path (a
/// pre-existing gap, not one this function introduces); the deselect is
/// unobservable here because the window's item tree is rebuilt from the new
/// directory, which drops every `OB_SELECTED` with the objects that carried it.
fn do_fopen(handle: i16, curr: i16, spec: &str, title: &str, allow_new_win: bool) {
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
        open_dir_window(spec, title);
    } else {
        navigate_window(handle, spec, title);
    }
}

/// Open a new directory window for `spec` (e.g. `C:\SUB\*.*`), titled `title`,
/// scan + sort its contents, build the view, and paint it (EmuTOS `do_diropen`
/// + `win_bldview`). A no-op if all window slots are in use.
fn open_dir_window(spec: &str, title: &str) {
    open_dir_window_at(spec, title, None, 0, 0);
}

/// [`open_dir_window`] with an explicitly supplied frame rectangle and scroll
/// position — the shape EmuTOS's `do_diropen(pw, TRUE, obid, path, pt, TRUE)`
/// has, where `pt` is the caller's rectangle (`deskmain.c:1497`, `cnx_get`
/// passing `(GRECT *)pws`). With `rect == None` the window takes its slot's
/// [`super::inf::window_rect`] (EmuTOS `win_alloc`'s
/// `cs_wnode[G.g_wcnt]` default, `deskwin.c:165`), or a staggered fallback.
fn open_dir_window_at(spec: &str, title: &str, rect: Option<Grect>, cvcol: i16, cvrow: i16) {
    let Some(desk) = global::with(|g| g.g_desk) else {
        return;
    };
    let handle = super::wind_create(WINDOW_STYLE as i16, desk);
    if handle < 1 {
        return;
    }
    let scanned = global::with(|g| {
        let sort = g.sort;
        let slot = g.win_alloc(handle, 0)?;
        {
            let pn = &mut g.windows.slots[slot].w_pnode;
            pn.pn_open(spec);
            pn.pn_active(true);
            pn.pn_sort(sort);
        }
        // `pw->w_cvcol = pws->hsl_save; pw->w_cvrow = pws->vsl_save;`
        // (`cnx_get`, `deskmain.c:1493-1494`) — the saved scroll position, set
        // before the view is built so `win_bldview` lays out at that offset.
        g.windows.slots[slot].w_cvcol = cvcol;
        g.windows.slots[slot].w_cvrow = cvrow;
        let pn = &g.windows.slots[slot].w_pnode;
        Some((slot, pn.p_count, pn.p_size))
    })
    .flatten();
    let Some((slot, count, size)) = scanned else {
        super::wind_delete(handle);
        return;
    };

    // Title (WF_NAME) + info line (WF_INFO), held in the node and passed by
    // address (`win_sname`/`win_sinfo`; the AES keeps the pointer, A83).
    set_window_strings(handle, slot, title, count, size);

    // Open at the window's saved `NEWDESK.INF` rectangle for this slot, or a
    // staggered sub-rectangle of the desktop (later windows offset so they do
    // not exactly overlap) when the INF supplies none.
    let step = (slot as i16) * 24;
    let rw = (desk.w - 96).min(560);
    let rh = (desk.h - 56).min(400);
    let default_r = Grect { x: desk.x + 48 + step, y: desk.y + 24 + step, w: rw, h: rh };
    let r = rect
        .or_else(|| super::inf::window_rect(slot))
        .unwrap_or(default_r);
    super::wind_open(handle, r);

    // Build the view for the actual work rect + push the scrollbar geometry,
    // then make it the current window and paint everything.
    let work = wind_get_work(handle);
    if let Some(sliders) = global::with(|g| g.win_bldview(slot, work)) {
        set_sliders(handle, sliders);
    }
    global::with(|g| g.desk_verify(handle, work, false));
    // No desktop-side repaint: `wind_open` (above) already repainted the AES
    // background + frame over the new window's rect and `send_redraw`d a
    // `WM_REDRAW` for its content — drained by the event loop into
    // `redraw::do_wredraw` once `win_bldview` (above) has laid out the items.
    // Exactly the EmuTOS `do_diropen` → `wind_open` → AES-`WM_REDRAW` path.
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

/// `do_diropen(pw, FALSE, curr, pathname, pt, TRUE)` (EmuTOS
/// `desksupp.c:422-467`) — re-fill window `handle`'s **existing** `WNODE` from
/// the directory `spec`/`title`: same frame, new path. The one body behind both
/// in-place navigations, exactly as in EmuTOS, where both reach it through
/// `do_fopen(…, new_win = FALSE)`:
///
/// * Close-folder / `WM_CLOSED` "up one level" (`w_setpath`, `deskfun.c:817-824`)
/// * opening a sub-folder in the window it lives in ([`do_fopen`], DESK34)
///
/// `pn_close(&pw->w_pnode)` + `pn_open` (`:1153`, `:428`) — our
/// [`super::model::PNode::pn_open`] frees the previous file list itself — then
/// `pn_active`, `win_sname`/`win_sinfo`/`wind_set(WF_NAME)` (`:444-446`),
/// `do_wopen(FALSE, …)` (no `wind_open`: the frame does not move), the
/// `desk_verify` that makes this the current window (`:459`), and the
/// `fun_msg(WM_REDRAW, …)` of `pt` that only the `!new_win` arm sends
/// (`:462-463`).
///
/// The caller resets the vertical scroll before calling — `pw->w_cvrow = 0` at
/// `desksupp.c:1258` and `deskfun.c:823`. `w_cvcol` is **not** reset there and
/// is not reset here either; `win_ocalc` clamps it to the new directory's
/// column count, so a shorter listing cannot scroll off the end.
fn navigate_window(handle: i16, spec: &str, title: &str) {
    let scanned = global::with(|g| {
        let sort = g.sort;
        let slot = g.win_slot(handle)?;
        {
            let pn = &mut g.windows.slots[slot].w_pnode;
            pn.pn_open(spec);
            pn.pn_active(true);
            pn.pn_sort(sort);
        }
        // `pw->w_cvrow = 0` — the caller's step in EmuTOS (`desksupp.c:1258`,
        // `deskfun.c:823`), taken here because both of our callers want it.
        g.windows.slots[slot].w_cvrow = 0;
        let pn = &g.windows.slots[slot].w_pnode;
        Some((slot, pn.p_count, pn.p_size))
    })
    .flatten();
    let Some((slot, count, size)) = scanned else {
        return;
    };

    set_window_strings(handle, slot, title, count, size);

    let work = wind_get_work(handle);
    if let Some(sliders) = global::with(|g| g.win_bldview(slot, work)) {
        set_sliders(handle, sliders);
    }
    // `desk_verify(pw->w_id, TRUE)` (`desksupp.c:459`) — the window whose
    // contents just changed becomes the current one (`g_cwin`/`g_croot`/
    // `g_wlastsel`). `changed` is false because the view rebuild above already
    // is `desk_verify(…, TRUE)`'s `win_bldview` half.
    global::with(|g| g.desk_verify(handle, work, false));
    // Same window, unchanged rect, new content: repaint this window's content
    // directly via `do_wredraw` over its work area — EmuTOS's
    // `fun_msg(WM_REDRAW, pw->w_id, pt)` (`:462-463`), which its event loop
    // turns into exactly this call (`deskmain.c:1272-1278`); no geometry change
    // and so no AES-generated `WM_REDRAW` involved.
    redraw::do_wredraw(handle, work);
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
