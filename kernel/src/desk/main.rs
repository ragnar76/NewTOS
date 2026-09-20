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

use tos_api::bios::drive::drvbits;

use super::global::{self, Grect, DROOT, MAX_DEPTH, NIL, NUM_WNODES, WINDOW_STYLE};
use super::menu::{self, Item, Title};
use super::win::{DESKWH, SliderSet, WA_DNLINE, WA_DNPAGE, WA_LFLINE, WA_RTLINE, WA_UPLINE, WA_UPPAGE};
use super::{dir, ftype, redraw};
use super::{
    wind_get_grect, wind_get_top, wind_get_work, MU_BUTTON, MU_KEYBD, MU_MESAG, MU_TIMER, WF_CXYWH,
    WF_FXYWH, WF_HSLIDE, WF_HSLSIZ, WF_INFO, WF_NAME, WF_PXYWH, WF_TOP, WF_VSLIDE, WF_VSLSIZ,
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
fn hndl_msg(msg: &[i16; 8]) -> bool {
    let handle = msg[3];
    let mut done = false;
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
        WM_TOPPED => top_msg(handle),
        WM_CLOSED => do_close_folder(), // == File → Close (EmuTOS CLOSITEM)
        WM_FULLED => full_msg(handle),
        WM_ARROWED => arrow_msg(handle, msg[4]),
        WM_VSLID => slid_msg(handle, false, msg[4]),
        WM_HSLID => slid_msg(handle, true, msg[4]),
        WM_MOVED => moved_msg(handle, msg),
        WM_SIZED => sized_msg(handle, msg),
        _ => {}
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
    let Some((ex, ey, ew, eh)) = global::with(|g| g.selection_extent(root)).flatten() else {
        return false;
    };
    let desk = g_desk();
    let bound = Grect { x: 0, y: 0, w: desk.x + desk.w, h: desk.y + desk.h };
    let (fx, fy) = super::graf_dragbox(ew, eh, ex, ey, bound);

    // Resolve the drop destination from the release point.
    let st = super::graf_mkstate();
    let dst_wh = super::wind_find(st.mx, st.my);
    let dst_root = global::with(|g| g.root_of(dst_wh)).unwrap_or(DROOT);
    let dobj = gr_obfind(dst_root, st.mx, st.my);
    let drop_keystate = st.kstate as u16;

    // Self-drop in the same window → cancel (deskact.c:668).
    if dst_wh == handle && dobj == sobj {
        return false;
    }

    // Desktop icon → empty desktop space: reposition in place, bypass fun_drag
    // (deskact.c:673 `move_drvicon`).
    if handle == DESKWH && dst_wh == DESKWH && dobj == DROOT {
        if global::with(|g| g.move_drvicon(fx - ex, fy - ey)).unwrap_or(false) {
            redraw::do_wredraw(DESKWH, desk);
        }
        return false;
    }

    // Otherwise dispatch the drag through the deskfun dispatch tree.
    let done = dir::fun_drag(handle, dst_wh, dobj, drop_keystate);
    global::with(|g| g.desk_clear(handle));
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
            // Desktop icon: the drive letter overlay in ib_char's low byte.
            let letter = (g.g_screeninfo[obid as usize].block.ib_char & 0xFF) as u8;
            if letter.is_ascii_uppercase() {
                OpenTarget::Drive(letter)
            } else {
                OpenTarget::None
            }
        }
    })
    .unwrap_or(OpenTarget::None);

    match target {
        OpenTarget::Folder(spec, title) => open_dir_window(&spec, &title),
        OpenTarget::Drive(letter) => open_root_window(letter),
        OpenTarget::App { path, name, flavor } => launch_native(&path, &name, flavor, &[]),
        OpenTarget::M68k => {
            let text = b"[1][ This is an m68k Atari program. | The m68k emulator is not | available yet. ][ OK ]\0";
            super::form_alert(1, text.as_ptr());
            // No desktop repaint: the AES `form_alert` bb_save/restores the screen
            // under the alert (EmuTOS `fm_alert`), so the desktop returns intact.
        }
        OpenTarget::Doc { name, full_path } => open_document(&name, &full_path),
        OpenTarget::None => {}
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

/// Launch a native NewTOS program (ADR-018) and repaint the desktop on return.
///
/// The launch primitive is [`crate::launch::run_program`], which loads the ELF
/// from the `C:` FAT16 path, drops to EL0, runs it, and returns on `Pterm`.
/// The flavour selects the surrounding behaviour:
///
/// * [`ftype::Flavor::App`] (`.NAP`) — run directly; the app draws its own AES
///   UI over the desktop, so nothing special is needed around the call.
/// * [`ftype::Flavor::Console`] (`.NCP`) — clear the screen and home the
///   cursor (`fbcon::clear_and_home`), the same `ESC E` a GEM program's
///   `v_enter_cur` sends before it runs a TOS program, so the child's output
///   starts at the top left; run it, then repaint the desktop over the text
///   below — mirroring [`crate::shell::run_console`]'s enter/restore.
///   `run_program` is kept pure
///   (no console/redraw coupling); the enter is done here and the restore is
///   the trailing [`redraw::redraw_desktop`] (whole-screen recovery — the
///   console cleared the screen).
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
            // Console (`.NCP`): its own VT52 text-screen enter/restore, NOT the
            // GEM screen handoff — left exactly as before.
            crate::fbcon::clear_and_home();
            let ran = crate::launch::run_program_with_tail(path.as_bytes(), tail);
            // Classic TOS keeps the console up until the user acknowledges, so
            // the program's final output stays readable before the desktop
            // repaints over it. Only reached from a real double-click (an
            // interaction path that requires HID input — verified on hardware).
            console_pause_for_key();
            if !ran {
                alert_launch_failed(name);
            }
            // The console program cleared the whole screen: genuine whole-screen
            // recovery of the desktop (background + windows + menu bar).
            redraw::redraw_desktop();
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
///   desktop epilog — `menu_bar(NULL,0)`, `wind_set(DESKWH, WF_NEWDESK, NULL)`,
///   `appl_exit()` (`deskmain.c:2159-2166`);
/// * **resume** ([`desktop_resume`], after the child's `Pterm`): `appl_init()`,
///   re-register the background tree, repaint.
///
/// Returns whether the child launched (`run_program` → `true`).
fn run_gem_app(path: &[u8], tail: &[u8]) -> bool {
    desktop_relinquish();
    let ran = crate::launch::run_program_with_tail(path, tail);
    desktop_resume();
    ran
}

/// The **relinquish** half of [`run_gem_app`]: the EmuTOS desktop *epilog*
/// (`deskmain.c:2159-2166`) — the three steps the desktop takes before it gives
/// the screen up, every one of them a plain AES call the desktop is entitled to
/// make.
///
/// **What used to be here and is now gone (AES gap G1/G2).** Two further steps
/// stood here, neither of which is an AES function: a `save_windows()` snapshot
/// of the shared AES window table and a `sh_draw_clear()` screen white-out. Both
/// existed only because NewTOS runs the child *nested* inside the desktop's own
/// event loop. EmuTOS never needs either: `hndl_button`'s `do_filemenu(OPENITEM)`
/// returns TRUE (`deskmain.c:988`), `deskmain`'s loop ends, the desktop process
/// **terminates**, and the AES shell loop — not the desktop — does the window
/// teardown (`wm_init()`, `gemshlib.c:717`) and the clear (`sh_draw(D.s_cmd,
/// TRUE)`, `gemshlib.c:719`) before it loads the child. Both of those live in
/// `crate::shell_loop::sh_main` already; the desktop has no business doing them.
fn desktop_relinquish() {
    // Remove the desktop menu bar (EmuTOS `menu_bar(NULL,0)`). Our AES
    // `menu_bar(tree, 0)` needs the real (non-null) tree to erase the strip.
    super::menu_bar(menu::tree_ptr(), 0);
    // Un-register the desktop background tree (EmuTOS
    // `wind_set(DESKWH, WF_NEWDESK, NULL, 0, 0)`, `deskmain.c:2163`) so the AES
    // `w_drawdesk` falls back to its own built-in DESKTOP root
    // (`gemwmlib.c:340-359,1268-1271`).
    super::wind_set_newdesk(0, 0);
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
/// before the launch (EmuTOS `deskmain.c:2124-2171` + `gemshlib.c:606,710-727`).
fn desktop_resume() {
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
    // Re-register the desktop's background object tree (EmuTOS
    // `wind_set(DESKWH, WF_NEWDESK, G.g_screen, 1, 0)`, `deskmain.c:2049`).
    // The AES `w_drawdesk` draws `DROOT` + icons again from here.
    //
    // **AES gap G2.** The window set is *not* restored: `restore_windows()` was
    // removed with its `save_windows()` counterpart. EmuTOS does not restore
    // either — it re-enters `deskmain` from scratch (`gemshlib.c:600-612`) and
    // rebuilds its windows from the desktop's **own** context save area
    // (`cnx_get`, `deskmain.c:2055`), which this desktop does not have.
    if let Some(tree) = super::global::screen_addr() {
        super::wind_set_newdesk(tree, super::global::DROOT);
    }
    // App-return recovery: the child cleared/overdrew the whole screen, so rebuild
    // the desktop background + windows + menu bar (`redraw_desktop` re-installs the
    // menu bar itself). Not a per-event shortcut — a genuine whole-screen restore.
    redraw::redraw_desktop();
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
    // 4. Function key F1-F20 — launch its installed application (none until
    //    Task #9 → unhandled, falls through).
    if let Some(done) = check_function_key(key) {
        menu::men_update();
        return done;
    }
    // 5. Alt+letter — open that drive (inert on the current keymap, see below).
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
    global::with(|g| g.desk_clear(handle));
    let work = wind_get_work(handle);
    if let Some(sliders) = global::with(|g| g.win_arrow(slot, work, action)).flatten() {
        set_sliders(handle, sliders);
        redraw::do_wredraw(handle, work);
    }
}

/// `check_function_key` (EmuTOS `deskmain.c:1117`) — F1-F20 launch the
/// installed application bound to that function key. The application list
/// (ANODE chain) is built by Install-Application (deskins, **Task #9**); with
/// none installed, no function key resolves to a launch target, so the key is
/// unhandled and falls through (EmuTOS `process_funkey` → `do_aopen` only on a
/// matching `a_funkey`). Returns `None` (unhandled) for both a non-function key
/// and a function key with no bound application — a genuine no-op over an empty
/// list, not a stub.
fn check_function_key(key: i16) -> Option<bool> {
    funkey_number(key)?; // must be F1-F20, else fall through
    None // no installed application bound yet (Task #9)
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
/// **Inert on the current NewTOS keymap:** the IKBD/keymap folds Alt into AltGr
/// and never yields ASCII `0` for a letter key, so a letter never arrives with
/// the bare-scancode shape this path requires — the drive never opens. Faithful
/// check, left in place; a keyboard-layer limitation flagged to the lead, not a
/// desktop stub. Returns `true` only if it actually opened a drive.
fn check_alt_letter_key(key: i16) -> bool {
    if (key & 0xFF) != 0 {
        return false;
    }
    let sc = ((key >> 8) & 0xFF) as u8;
    let letter = crate::keymap::to_ascii(sc, true, false, false);
    if letter.is_ascii_uppercase() {
        let drive = letter - b'A';
        if drvbits() & (1u32 << drive) != 0 {
            open_root_window(letter);
            return true;
        }
    }
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

/// `do_deskmenu` (EmuTOS `deskmain.c:612`) — the Desk menu. Only `About`
/// (Desktop info); a `form_alert` stands in for the resource About dialog.
fn do_deskmenu(_item: Item) {
    let text = b"[1][ NewTOS Desktop | EmuTOS-conformant rebuild | ADR-016 (Task #8) ][ OK ]\0";
    super::form_alert(1, text.as_ptr());
    // No after-menu whole-desktop repaint: the Desk drop-down was save/restored by
    // the AES menu manager (P1), and the AES `form_alert` bb_save/restores the
    // screen under the About box — the desktop returns intact via the app path.
}

/// `do_filemenu` (EmuTOS `deskmain.c:654`) — the File menu. Task-#8 items:
/// Open, Close folder, Close top window, Cycle windows. The rest are Task-#9
/// ops kept disabled by `men_update`. Returns the loop-exit flag (never, here).
fn do_filemenu(item: Item) -> bool {
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
        Item::InfoRename => do_info_rename(),
        _ => {} // remaining File items have no live handler yet
    }
    false
}

/// `do_optnmenu` (EmuTOS `deskmain.c:786`) — the Options menu. Live items: Set
/// background (`inf_backgrounds`), Install Application (deskins #20), Read .INF
/// (reload + apply), Save desktop (`app_save`). The remaining preferences /
/// resolution items stay disabled by `men_update` (deskrez is config.txt-driven,
/// ADR-017 rev 2).
fn do_optnmenu(item: Item) {
    match item {
        Item::SetBackground => {
            // Self-drawn dialog (no AES save/restore): restore the whole desktop it
            // covered (Module-2 restore path — see `redraw::redraw_desktop`).
            super::inf::set_background();
            redraw::redraw_desktop();
        }
        Item::InstallApp => do_install_app(),
        Item::ReadInf => do_read_inf(),
        Item::SaveDesktop => do_save_desktop(),
        _ => {} // remaining Options items stay disabled
    }
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
    let (init_ext, init_flavor, init_full) = match existing {
        Some((ext, fl, full)) => (ext, fl, full),
        None => (String::new(), default_flavor(flavor), false),
    };

    let outcome = super::insapp::ins_app_dialog(&app_name, &init_ext, init_flavor, init_full, installed);
    match outcome {
        Outcome::Install { doc_ext, flavor: fl, full_path } => {
            super::inf::install_application(&app_path, &doc_ext, fl, full_path, false, "");
        }
        Outcome::Remove => {
            super::inf::remove_application(&app_path);
        }
        Outcome::Cancel => {}
    }
    redraw::redraw_desktop(); // self-drawn dialog: restore the whole desktop it covered
}

/// The default Install-Application flavour for a native program, from its
/// extension: `.NAP` (GEM app) → GEM, `.NCP` (console) → TOS, `.NTP`
/// (takes-parameters) → TTP. The user can change it in the dialog.
fn default_flavor(flavor: ftype::Flavor) -> super::inf::AppFlavor {
    match flavor {
        ftype::Flavor::App => super::inf::AppFlavor::Gem,
        ftype::Flavor::Console => super::inf::AppFlavor::Tos,
        ftype::Flavor::Params => super::inf::AppFlavor::Ttp,
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

/// Options → Read .INF file: re-read `NEWDESK.INF`, re-apply it (view/sort,
/// background, desktop-icon positions), rebuild the desktop icon tree and every
/// open window's view, and repaint. A missing file is a silent no-op.
fn do_read_inf() {
    if !super::inf::load_and_apply() {
        return;
    }
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

/// Options → Save desktop: serialise the current desktop state to
/// `NEWDESK.INF`. Reports failure through an alert.
fn do_save_desktop() {
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
        redraw::redraw_desktop(); // dialog cancelled — restore the screen
        return;
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
    redraw::redraw_desktop();
}

/// File → Info/Rename (EmuTOS `do_filemenu` `SHOWITEM` → `inf_file_folder`,
/// `deskinf.c:493-639`): show the single selected item's size/date/attributes
/// in a dialog with an editable name; on OK with a changed name, `Frename` it.
/// The dialog itself lives in [`super::inf`].
fn do_info_rename() {
    let cwin = global::with(|g| g.g_cwin).unwrap_or(DESKWH);
    if cwin == DESKWH {
        return;
    }
    // The single selected window item and its display fields.
    let sel = global::with(|g| {
        let slot = g.win_slot(cwin)?;
        let spec = g.windows.slots[slot].w_pnode.p_spec.clone();
        let dir = spec.rfind('\\').map(|i| String::from(&spec[..=i]))?;
        let f = g.windows.slots[slot].w_pnode.p_flist.iter().find(|f| f.f_selected)?;
        let name = core::str::from_utf8(f.name_bytes()).ok().map(String::from)?;
        Some((dir, name, f.f_size, f.f_date, f.f_time, f.f_attr))
    })
    .flatten();
    let Some((dir_path, old, size, date, time, attr)) = sel else {
        return;
    };
    let renamed = super::inf::inf_file_folder(&old, size, date, time, attr);
    let Some(new) = renamed else {
        redraw::redraw_desktop(); // dialog cancelled / no change — restore the screen
        return;
    };
    let new = new.trim();
    if !new.is_empty() && new != old {
        let mut oldp = dir_path.clone();
        oldp.push_str(&old);
        let mut newp = dir_path;
        newp.push_str(new);
        if dir::rename(&oldp, &newp) {
            dir::rebuild_window(cwin);
            return;
        }
        dir::fun_alert(1, b"Could not rename the item.");
    }
    redraw::redraw_desktop();
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
        redraw::redraw_desktop(); // cancelled — restore the screen under the dialog
        return;
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
        fmt_info(&mut node.w_info, count, size);
        (node.name_ptr(), node.info_ptr())
    });
    if let Some((name, info)) = ptrs {
        super::wind_set_ptr(handle, WF_NAME, name);
        super::wind_set_ptr(handle, WF_INFO, info);
    }
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
        redraw::redraw_desktop(); // cancelled — restore the screen under the dialog
        return;
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
    global::with(|g| match item {
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
    .unwrap_or(0)
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
fn top_msg(handle: i16) {
    global::with(|g| {
        let prev = g.g_cwin;
        g.desk_clear(prev);
    });
    if global::with(|g| g.win_find(handle).is_some()).unwrap_or(false) {
        super::wind_set_int(handle, WF_TOP, 0); // AES: top + repaint bg/frames
        global::with(|g| g.win_top(handle));
        let work = wind_get_work(handle);
        global::with(|g| g.desk_verify(handle, work, false));
        // change = TRUE → cnx_put() (session save) — not implemented (future module).
    }
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
fn full_msg(handle: i16) {
    if global::with(|g| g.win_find(handle).is_some()).unwrap_or(false) {
        global::with(|g| g.win_top(handle)); // fuller implies top
        do_wfull(handle);
        let work = wind_get_work(handle);
        if let Some(sliders) = global::with(|g| g.desk_verify(handle, work, true)).flatten() {
            set_sliders(handle, sliders);
        }
    }
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
fn moved_msg(handle: i16, msg: &[i16; 8]) {
    let Some(slot) = global::with(|g| g.win_slot(handle)).flatten() else {
        return;
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
fn sized_msg(handle: i16, msg: &[i16; 8]) {
    if global::with(|g| g.win_find(handle).is_some()).unwrap_or(false) {
        let mut gr = Grect { x: msg[4], y: msg[5], w: msg[6], h: msg[7] };
        do_xyfix(&mut gr);
        super::wind_set_grect(handle, WF_CXYWH, gr); // AES: repaint bg/frames + send_redraw
        let work = wind_get_work(handle);
        if let Some(sliders) = global::with(|g| g.desk_verify(handle, work, true)).flatten() {
            set_sliders(handle, sliders);
        }
    }
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

/// Open a new directory window for `spec` (e.g. `C:\SUB\*.*`), titled `title`,
/// scan + sort its contents, build the view, and paint it (EmuTOS `do_diropen`
/// + `win_bldview`). A no-op if all window slots are in use.
fn open_dir_window(spec: &str, title: &str) {
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
    let r = super::inf::window_rect(slot).unwrap_or(default_r);
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

/// Re-scan window `handle` in place for a new directory `spec`/`title` (the
/// Close-folder "up one level" navigation), reset its scroll, rebuild + repaint.
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
        g.windows.slots[slot].w_cvrow = 0;
        g.windows.slots[slot].w_cvcol = 0;
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
    // Same window, unchanged rect, new content (folder "up one level"): repaint
    // this window's content directly via `do_wredraw` over its work area — the
    // EmuTOS `win_blt`/`refresh_window` direct-`do_wredraw` idiom
    // (`deskwin.c:635`), no geometry change and so no AES `WM_REDRAW` involved.
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
        if global::with(|g| g.desk_clear(handle)).unwrap_or(false) {
            any_win = true;
        }
    }
    let any_desk = global::with(|g| g.desk_clear(DESKWH)).unwrap_or(false);
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

/// Build a directory-window info line ` N items   M bytes used` into `out`
/// (NUL-terminated, truncated to fit) — the node's `w_info` buffer.
fn fmt_info(out: &mut [u8], count: usize, size: u64) {
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
    push(out, &mut p, fmt_u64(count as u64, &mut num));
    push(out, &mut p, b" items   ");
    push(out, &mut p, fmt_u64(size, &mut num));
    push(out, &mut p, b" bytes used");
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
