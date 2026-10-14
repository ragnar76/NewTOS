//! `desktop` — the EmuTOS-conformant NewTOS desktop (ADR-016 ground-up rebuild).
//!
//! This is the home of the desktop *application* (EmuTOS `desk/`),
//! reimplemented in idiomatic `#![no_std]` Rust. It supersedes the legacy
//! `crate::desktop` / `crate::desktop_icons` kernel modules (kept in the tree,
//! out of the build, until feature parity — see
//! `docs/api/desktop/_rebuild-plan.md`).
//!
//! **A crate of its own (DESK43 U8b).** Until U8b this was the kernel module
//! `kernel/src/desk/`; it is now the library crate `desktop`
//! (`programs/desktop/`), linked by the kernel, which still calls [`run`] and
//! [`reset`] in EL1 from `sh_ldapp`'s DESKTOP arm — and, from DESK43 U9, by the
//! `NEWDESK` program as well: one source, two link targets
//! (`docs/analysis/desk43-u5-u10-review-2026-10-14.md` §3.1). Its only
//! dependencies are `newtos-gem` (the trap bindings) and `tos-types` (the
//! on-wire records and constants), so a path into the kernel or into
//! `tos_api` does not compile here (guard G1, §4.2).
//!
//! Scope so far: the data model ([`global`]/[`model`]/[`win`]), the boot
//! one-time setup, the window/icon view builder ([`app`]/[`win`], Task #6), the
//! damage-rectangle redraw ([`redraw`], Task #7), and the **interaction layer**
//! ([`main`]/[`menu`], Task #8): the `evnt_multi` loop + message drain, mouse
//! button dispatch (`hndl_button` via `wind_find` — fixes D3), keyboard
//! (`hndl_kbd` — fixes D4), the menu bar + `hndl_menu` + `men_update`
//! (enable/disable/check — fixes D6), and window move/size/top/close wiring.
//! File/INF ops and the drag/rubber-band selection machine are Task #9.
//!
//! The desktop reaches the system **only** through traps, never by calling a
//! `tos_api` function directly. The trap bindings themselves are not here
//! (DESK43 U6b): they are `newtos_gem` — [`newtos_gem::aes`] (EmuTOS
//! `desk/gembind.c`), [`newtos_gem::gemdos`] (`include/gemdos.h`),
//! [`newtos_gem::vdi`] and [`newtos_gem::xbios`] (`include/xbiosbind.h`) — the
//! runtime crate every native Rust program links, so this desktop and a
//! program call the AES the same way. This crate root imports the AES
//! bindings the desktop modules use (they keep calling `super::form_alert`,
//! as EmuTOS's `desk/*.c` call the global `aesbind` functions), adds the few
//! desktop-shaped helpers over them, and holds the boot entry point [`run`].

#![no_std]
// The lint set the desktop had as a module of the kernel crate
// (`kernel/src/lib.rs`), kept so the move changes no diagnostic.
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]
// The module `main` is the desktop's `deskmain.c` (event loop and dispatch),
// not a binary entry point: Cargo's `src/main.rs` convention is switched off
// in Cargo.toml (`autobins = false`) and the name is kept from the kernel
// module. (The lint fires on the `mod` item before item-level attributes
// apply, hence crate level.)
#![allow(special_module_name)]

extern crate alloc;

pub mod act;
pub mod app;
pub mod dir;
pub mod dlg;
pub mod ftype;
pub mod global;
pub mod icons;
pub mod inf;
pub mod ins;
pub mod insapp;
pub mod main;
pub mod menu;
pub mod model;
pub mod pro;
pub mod redraw;
pub mod selftest;
pub mod gsx;
pub mod win;

// No diagnostic channel. EmuTOS's desktop has one only on paper — `KDEBUG`
// (`bios/kprint.h`), left compiled out in every `desk/*.c`
// (`/* #define ENABLE_KDEBUG */`, `deskact.c:19`, `deskapp.c:20`) — and the
// NewTOS desktop prints nothing either (DESK43 decision D4, 2026-10-14). The
// one line the harnesses key on, `[desk] desktop up`, is the kernel's
// observation of this desktop reaching its first `evnt_multi`
// (`kernel/src/shell_loop.rs`, not a desktop call); the self-tests have their own
// console writer ([`selftest`]).

use global::{MAX_DEPTH, WINDOW_STYLE};
use newtos_gem::aes::{EvntMulti, Events};
use tos_types::aes::{
    Grect, BEG_UPDATE, DESKWH, END_UPDATE, FMD_FINISH, FMD_START, MU_BUTTON, MU_M1, WC_WORK, WF_TOP,
    WF_WXYWH,
};

// ── aesbind: the AES trap bindings (`newtos_gem::aes`) ─────────────────────
//
// The desktop is a client of the AES: it drives it through the GEM trap
// (`svc #2`, selector 200) with a parameter block of six array pointers,
// exactly like EmuTOS `desk/` calls the AES over `aesbind`
// (`desk/gembind.c`). The bindings are `newtos_gem::aes` (DESK43 U6b moved
// them out of this file); importing them here keeps every desktop module's
// `super::<call>` unchanged. Opcodes, `control[]` counts and argument layout
// are documented there, per function.
use newtos_gem::aes::{
    appl_exit, appl_init, evnt_button, evnt_dclick, form_alert, form_center, form_dial, form_do,
    fsel_exinput, graf_growbox, graf_handle, graf_mkstate, graf_mouse, graf_rubbox, menu_bar,
    menu_icheck, menu_ienable, menu_tnormal, objc_add, objc_draw, objc_find, objc_offset, shel_get,
    shel_put, shel_wdef, shel_write, wind_calc, wind_close, wind_create, wind_delete, wind_find,
    wind_get_grect, wind_get_int, wind_new, wind_open, wind_set_grect, wind_set_int, wind_set_newdesk,
    wind_set_ptr, wind_update,
};

// The AES constants (`WF_*`, `WC_*`, `MU_*`, `M_ON`/`M_OFF`, `ARROW`, …) are
// `tos_types::aes` (one public set, DESK43 U5b). The two `graf_mouse` shapes
// below keep the desktop's own `deskbind.h` names.
/// `graf_mouse` shape number `FLATHAND` (`desk/deskbind.h:34`, = `FLAT_HAND`,
/// `aesdefs.h:428`): the open hand `gr_drgplns` shows for the duration of a
/// drag (`deskact.c:264`).
pub(crate) const FLATHAND: i16 = 4;
/// `graf_mouse` shape number `HGLASS` (`desk/deskbind.h:33`, = `HOURGLASS`,
/// `aesdefs.h:426`): the busy indicator `desk_busy_on` sets
/// (`desk/desksupp.c:1840-1843`).
pub(crate) const HGLASS: i16 = 2;

// ── Desktop-shaped helpers over the bindings ───────────────────────────────
//
// Each is one binding call with the arguments this desktop always passes —
// the shapes EmuTOS's `desk/*.c` write out at their call sites.

/// `wind_get(handle, WF_WXYWH)` — the window's current work-area rectangle.
pub(crate) fn wind_get_work(handle: i16) -> Grect {
    wind_get_grect(handle, WF_WXYWH)
}

/// `wind_get(DESKWH, WF_TOP)` — the handle of the top-most open window, or
/// [`DESKWH`] when only the desktop is showing (EmuTOS
/// `wind_get(DESKWH, WF_TOP, &wh, …)`, `deskmain.c:1049`; this desktop also
/// answers `win_ontop` with it, see [`close_desktop_windows`]).
pub(crate) fn wind_get_top() -> i16 {
    wind_get_int(DESKWH, WF_TOP)
}

/// `objc_draw` (42) the subtree rooted at `root` of the desktop object arena
/// `tree`, clipped to `clip`, at the desktop's `MAX_DEPTH`. Used by
/// `do_wredraw` to paint a window's item tree **and** the desktop's own
/// surface (`DROOT` background box + icons) — the "two paths, one tree" of
/// EmuTOS `do_wredraw` (`objc_draw(G.g_screen, root, MAX_DEPTH, …)`,
/// `desksupp.c:227`).
pub(crate) fn objc_draw_tree(tree: u64, root: i16, clip: Grect) {
    objc_draw(tree, root, MAX_DEPTH, clip);
}

/// `objc_draw` (42) the whole tree `tree` from `ROOT` (0) — used to paint a
/// self-built dialog, clipped to the rectangle `form_center` reported
/// (EmuTOS `start_dialog`, `objc_draw(tree, ROOT, MAX_DEPTH, …)`,
/// `deskinf.c:356`).
pub(crate) fn objc_draw_dialog(tree: u64, clip: Grect) {
    objc_draw_tree(tree, 0, clip);
}

/// `form_dial(FMD_START, 0,0,0,0, x,y,w,h)` (51) — reserve the screen area
/// `(x, y, w, h)` for a dialog, immediately before its first `objc_draw`
/// (EmuTOS `start_dialog`, `deskinf.c:349-357`).
///
/// EmuTOS's `fm_dial` does **not** save the pixels under `pt` for `FMD_START`
/// (`gemfmlib.c:365-369` is a bare `break` with the comment "grab screen sync
/// or some other mutual exclusion method"); the restore is a *repaint* driven
/// by [`form_dial_finish`]. The call is still made — it is the Form Manager's
/// documented ownership handshake and the AES is free to give it meaning. The
/// small rectangle is unused by `FMD_START`/`FMD_FINISH`; EmuTOS passes zeros,
/// so we do too.
pub(crate) fn form_dial_start(r: Grect) {
    form_dial(FMD_START, Grect::default(), r);
}

/// `form_dial(FMD_FINISH, 0,0,0,0, x,y,w,h)` (51) — release the screen area
/// `(x, y, w, h)` and restore what was under it: the AES repaints the desktop
/// background plus every window frame over that rectangle and fans a
/// `WM_REDRAW` out to each window it touches (`aes/gemfmlib.c:375-379`).
///
/// This is EmuTOS's whole-screen restore idiom after something has taken the
/// screen over — `desksupp.c:826` uses exactly
/// `form_dial(FMD_FINISH, 0,0,0,0, 0,0,scr_width,scr_height)` when the
/// file viewer hands the screen back — and, with the dialog's own rectangle,
/// the second half of `end_dialog` (`deskinf.c:363-369`).
pub(crate) fn form_dial_finish(r: Grect) {
    form_dial(FMD_FINISH, Grect::default(), r);
}

/// `wind_calc(WC_WORK, WINDOW_STYLE, desk)` — the work-area width of a
/// maximum-size window, feeding `win_view`'s `g_icols` (EmuTOS
/// `wind_calc_grect(1, WINDOW_STYLE, &G.g_desk, &t)`, `deskwin.c:95`).
pub(crate) fn wind_calc_work_width(desk: Grect) -> i16 {
    wind_calc(WC_WORK, WINDOW_STYLE, desk).w
}

/// `evnt_multi(flags, 0x02, 0x01, 0x01, 0,0,0,0,0, 0,0,0,0,0, msg, 0, …)` (25)
/// — block until any requested event fires and return which, plus the mouse /
/// key / click state. The button parameters are the desktop's fixed
/// `bclk = 2` (report up to a double click), `bmask = 1` (left button),
/// `bstate = 1` (down) — the EmuTOS `deskmain.c:2078` values. The MOBLK and
/// timer slots stay zero; a `flags` that includes [`MU_TIMER`] therefore
/// returns immediately once no other event is pending (the message-drain
/// "quick-out"), while the blocking outer loop omits it.
///
/// [`MU_TIMER`]: tos_types::aes::MU_TIMER
pub(crate) fn evnt_multi_msg(flags: u16, msg: &mut [i16; 8]) -> Events {
    let p = EvntMulti { flags, bclicks: 2, bmask: 1, bstate: 1, ..EvntMulti::default() };
    newtos_gem::aes::evnt_multi(&p, msg.as_mut_ptr())
}

/// `evnt_multi(MU_BUTTON | MU_M1, 0x01, 0xff, 0x00, out, x, y, w, h, …)` (25)
/// — the single shape `gr_isdown` calls (`deskact.c:70-77`): return as soon as
/// **all** buttons are up (`bmask = 0xff`, `bstate = 0x00`) or the pointer
/// crosses the `w`×`h` rectangle at `(x, y)` in the direction `out` asks for
/// (`m1flags`: leave = `TRUE`, enter = `FALSE`). No message buffer.
///
/// Returns the `RET_CODE` event mask, which is all `gr_isdown` inspects.
pub(crate) fn evnt_multi_m1(out: bool, x: i16, y: i16, w: i16, h: i16) -> u16 {
    let p = EvntMulti {
        flags: MU_BUTTON | MU_M1,
        bclicks: 1,
        bmask: 0xff,
        bstate: 0x00,
        m1flags: i16::from(out),
        m1: Grect { x, y, w, h },
        ..EvntMulti::default()
    };
    newtos_gem::aes::evnt_multi(&p, core::ptr::null_mut()).which
}

// ── Screen geometry, from the AES only ─────────────────────────────────────

/// `G.g_desk` — the desktop work area (the screen minus the menu-bar row).
/// Cached in [`global`] from `wind_get(DESKWH, WF_WXYWH)` at boot, exactly as
/// EmuTOS caches it (`deskmain.c:1959`); before the desktop state exists the
/// AES is asked directly. Nothing here consults the framebuffer.
pub(crate) fn g_desk() -> Grect {
    match global::with(|g| g.g_desk) {
        Some(d) => d,
        None => wind_get_grect(DESKWH, WF_WXYWH),
    }
}

/// The full screen as a `GRECT`, derived from [`g_desk`] the way EmuTOS derives
/// it when it needs the whole screen: `scr_width = G.g_desk.g_w;
/// scr_height = G.g_desk.g_y + G.g_desk.g_h;` (`desksupp.c:786-787`).
pub(crate) fn screen_rect() -> Grect {
    let d = g_desk();
    Grect { x: 0, y: 0, w: d.w, h: d.y + d.h }
}

// ── Boot entry ─────────────────────────────────────────────────────────────

/// `_deskstart` (`desk/deskstart.S:52-56`) — the desktop program's entry point,
/// which is what `sh_ldapp`'s DESKTOP arm starts
/// (`aes_run_rom_program(deskstart)`, `gemshlib.c:606`):
///
/// ```asm
/// restart:
///         jsr     _deskmain               // Run the desktop
///         tst.w   d0
///         jeq     restart                 // handle read of .INF file
///         clr.w   -(sp)
///         trap    #1                      // Pterm
/// ```
///
/// [`deskmain`] answers `false` for the one case that re-enters it in the same
/// process — a successful *Options → Read .INF file* — and `true` when the
/// desktop is done, which on an Atari is a `Pterm` and here is this function
/// returning into the kernel's `sh_ldapp` (`kernel/src/shell_loop.rs`). The
/// `Mshrink` and the stack switch the rest of `deskstart.S` performs are a
/// process's own setup; the NewTOS desktop is still called on the kernel
/// stack (DESK43 U9 moves it).
pub fn run() {
    while !deskmain() {
        // The allocations `deskmain`'s restart arm gave back
        // (`deskmain.c:2140-2147`, eight `dos_free`s of `G.g_wlist` …
        // `desk_rs_strings`) are ours as one singleton, and they are dropped
        // here rather than inside `deskmain` for a reason EmuTOS does not have:
        // `dos_free` leaves the bytes readable, `drop` does not — and at
        // `:2141` the AES still holds `g_screen` as its background tree, which
        // `menu_bar(NULL, 0)` (`:2160`) could still make it read before
        // `wind_set(DESKWH, WF_NEWDESK, NULL)` (`:2163`) takes it away. Freeing
        // after the un-registration is the same state transition without that
        // window.
        reset();
    }
}

/// `deskmain` (`desk/deskmain.c:1938-2172`) — one run of the desktop: the
/// library/state prolog (metrics, the desktop icon tree, the menu bar, the
/// windows the context asks for), the event loop, and the epilog that hands the
/// screen back. Returns `deskmain`'s `rc`: `true` = "the desktop process
/// terminates", `false` = "restart me immediately" (*Read .INF file*).
fn deskmain() -> bool {
    // `restart_desktop = FALSE;` (`:1946`) — this run's answer, until
    // `do_optnmenu`'s READITEM arm says otherwise.
    main::clear_restart_desktop();

    // No raster check here: `sh_ldapp`'s DESKTOP arm only reaches this function
    // when `gl_shgem` is set (`gemshlib.c:600-601`, `kernel/src/shell_loop.rs`), which
    // is where EmuTOS makes that decision too.

    // The desktop is a normal AES application. Register it FIRST (EmuTOS
    // `deskmain.c:1953`: `gl_apid = appl_init();` — the first library-init step,
    // before window creation and the event loop). `appl_init` opens the AES
    // process descriptor — and its FIFO message queue — for the desktop's pid
    // (=1 in the kernel boot context). Without it `pd::by_pid(1)` is `None`, so
    // every `send_redraw` → `post_msg(1, WM_REDRAW)` from a MOVE/SIZE/TOP/CLOSE/
    // open-new-window drops silently and the window content only appears on the
    // direct-draw boot / view-switch paths (the empty directory-window bug).
    //
    // EmuTOS keeps the result in `gl_apid` and does not test it; neither does
    // this desktop (the id is only ever 0, `tos-api/src/aes/appl.rs:200-224`).
    appl_init();

    // `gl_handle = graf_handle(&gl_wchar, &gl_hchar, &gl_wbox, &gl_hbox);`
    // (`deskmain.c:1956`) — the AES's workstation and the normal font's cell.
    // The small font's cell is an AES global the ROM desktop reads directly
    // (`deskapp.c:519-520`); here it is asked of that same workstation the way
    // the AES computes it (`gsx_start`, `aes/gemgraf.c:405-410`). Both before
    // `global::init`, so every layout below sees them.
    let gh = graf_handle();
    gsx::set_handle(gh.handle);
    let (gl_wschar, gl_hschar) = gsx::small_font_cell();
    global::set_cell_metrics(gh.wchar, gh.hchar, gl_wschar, gl_hschar);

    // `wind_get_grect(DESKWH, WF_WXYWH, &G.g_desk)` — the desktop work area,
    // straight after `appl_init` and `graf_handle` exactly as EmuTOS orders it
    // (`deskmain.c:1953-1959`). This is where the desktop learns the screen
    // geometry; it does not ask the framebuffer.
    let desk = wind_get_grect(DESKWH, WF_WXYWH);

    // One-time setup: DeskState (view/sort defaults, window list, object arena).
    global::init(desk);

    // Seed `G.g_idt` before anything formats a date. EmuTOS gets away with
    // refreshing it only inside `do_wredraw` (`desksupp.c:219`) because it
    // formats a text-view line at *draw* time (`dr_fnode`, `deskinf.c:320`);
    // NewTOS builds the line when the view is built, which happens earlier.
    // `do_wredraw` refreshes it after this, exactly as EmuTOS does.
    redraw::refresh_idt();

    // Grid metrics: app_start (icon cell + desktop pitch) → win_view (window
    // pitch; needs the full-window work width from the AES) → app_blddesk
    // (desktop drive/trash/printer icon tree).
    let full_work_w = wind_calc_work_width(desk);
    global::with(|g| {
        g.full_work_w = full_work_w;
        g.app_start();
    });

    // Read NEWDESK.INF and apply it (view/sort, g_patcol background, desktop-icon
    // grid positions, window rectangles) — after `app_start` so the grid pitch
    // (`g_icw`/`g_ich`) exists for the icon-position rounding, before `win_view`/
    // `app_blddesk` so the view and positions take effect. A missing file is not
    // an error (the desktop keeps its built-in defaults).
    if !inf::load_and_apply() {
        // "If there's still no EMUDESK.INF data, build one now"
        // (`deskapp.c:833-834`) — the default environment plus the desktop
        // icons the drive map implies. A file that *is* INF data but carries no
        // `#M`/`#T`/`#O` line is **not** this case: it describes a desktop with
        // no icons, which is what *Remove desktop icon* + *Save desktop*
        // writes, and it is honoured.
        inf::build_inf();
    }

    global::with(|g| {
        g.win_view();
        g.app_blddesk();
    });

    // Register the desktop's own object tree as the AES screen background
    // (EmuTOS `wind_set(DESKWH, WF_NEWDESK, G.g_screen, 1, 0)`, `deskmain.c:2049`):
    // from now on the AES's `w_drawdesk` draws `DROOT` (the background box) + the
    // icon children whenever the desktop area is exposed — the desktop no longer
    // paints its own background. Then install the menu bar (drawn once — it is
    // never covered, windows live below it).
    if let Some(tree) = global::screen_addr() {
        wind_set_newdesk(tree, global::DROOT);
    }
    menu::install_menu_bar();

    // `cnx_get()` (EmuTOS `deskmain.c:2053-2057`, between `wind_update
    // (BEG_UPDATE)` and `END_UPDATE`) — establish the desktop's state from what
    // `app_start` put in `G.g_cnxsave`: the saved view/sort/size-to-fit, then
    // the saved windows, bottom-most first so the z-order comes back.
    wind_update(BEG_UPDATE);
    main::cnx_get();
    wind_update(END_UPDATE);

    // EmuTOS opens whatever the context says and nothing more — its built-in
    // `EMUDESK.INF` (`deskapp.c:171-180`) carries four `#W` lines with *empty*
    // paths, so a machine with no saved session starts with a bare desktop.
    // NewTOS used to open the boot drive's root when nothing was restored;
    // that crutch pre-dated the context save (DESK09) and is gone (user
    // decision 2026-09-23): a window at boot comes from `NEWDESK.INF` or not
    // at all.

    // Set the menu state to reflect the current window (men_update — the D6
    // machinery), then paint everything (desktop bg + icons + frames via the
    // AES compositor, each window's content occlusion-clipped via
    // `do_wredraw`). Boot is a genuine first full paint — `redraw_desktop`
    // (whole-screen recovery), not a per-event shortcut.
    menu::men_update();
    redraw::redraw_desktop();

    // **AES gap G3.** A `tos_api::vdi::mouse::display_at(w/2, h/2, backend)`
    // stood here — a direct VDI call on the raster backend, parking the pointer
    // at the screen centre so the AES hover-menu would not open the Desk
    // drop-down at boot with the cursor resting at (0,0) over the menu bar.
    // EmuTOS's desktop never positions the pointer: `ratinit`
    // (`gemshlib.c:716`, via `sh_main`) centres it when the AES enters graphics
    // mode, and there is no AES binding that would let an application do it.
    // Removed — the pointer now starts wherever the AES left it.

    // `desk_busy_off()` (`deskmain.c:2071`, `desksupp.c:1849-1852` =
    // `graf_mouse(ARROW, NULL)`) — "menu is initialised - display menu bar &
    // set mouse to arrow". The AES handed the desktop an hourglass when it
    // switched into graphics mode (`sh_tographic`'s
    // `set_mouse_to_hourglass()`, `aes/gemshlib.c:237`); the desktop drops it
    // once it is ready for input.
    pro::desk_busy_off();

    // The desktop is up. It says so to nobody (D4): the kernel reports
    // `[desk] desktop up` when it sees the `evnt_multi` below arrive
    // (`observe_desktop_gem_call`, `kernel/src/shell_loop.rs`).

    // A `selftest-desk` build runs the four self-tests here, once per boot,
    // with the desktop fully set up (DESK45; [`selftest`]).
    #[cfg(feature = "selftest-desk")]
    selftest::run_once();

    // The event loop (`deskmain.c:2077-2123`). It returns when a handler set
    // `done` — a program start (`hndl_button`'s double click, `:988`), *File →
    // Execute EmuCON* (`:750-753`) or a successful *Read .INF file*
    // (`:919-925`) — each of which has already told the AES shell what is to
    // happen next with `shel_write(SHW_EXEC, …)`.
    main::event_loop();

    // ── The epilog (`deskmain.c:2124-2171`) ──────────────────────────────
    //
    // "if we are exiting because of a successful 'Read INF file', we must close
    // all the windows & free all the memory we allocated above, since we will
    // re-enter deskmain() immediately. in all other cases, the desktop process
    // will terminate and the allocated memory will be automatically freed. we
    // must save the desktop state in memory for when the desktop process
    // restarts." (`:2124-2134`)
    let rc = if main::restart_desktop() {
        // `:2136-2150`
        pro::desk_busy_on();
        close_desktop_windows();
        pro::desk_busy_off();
        false // "_deskstart will call us again immediately" (`:2149`)
    } else {
        // `:2153-2157`. `cnx_put()` snapshots the window layout into
        // `G.g_cnxsave`; `app_save(FALSE)` serialises that snapshot into the
        // **AES shell buffer** (`shel_put`, `deskapp.c:1293`) — the channel that
        // carries the desktop's state across its own death, which the next
        // `app_start` reads back (`deskapp.c:832`, DESK43 U2).
        main::cnx_put();
        inf::app_save(false);
        true // "_deskstart will terminate the process" (`:2156`)
    };

    // "turn off the menu bar" (`:2159-2160`, `menu_bar(NULL, 0)`). Our AES
    // `menu_bar(tree, 0)` needs the real (non-null) tree to erase the strip.
    menu_bar(menu::tree_ptr(), 0);

    // "give up the desktop" (`:2162-2163`, `wind_set(DESKWH, WF_NEWDESK, NULL,
    // 0, 0)`) — the AES `w_drawdesk` falls back to its own built-in DESKTOP
    // root from here (`gemwmlib.c:340-359,1268-1271`).
    wind_set_newdesk(0, 0);

    // "exit the gem AES" (`:2165-2166`). `ap_exit` (`gemaplib.c:251-261`)
    // drains the message queue and clears `AP_OPEN`; the drain is what stops
    // the next program inheriting the desktop's unread `WM_*` (A28), which
    // matters here because desktop and child share `rlr` (`gemshlib.c:633`).
    // Last, as in EmuTOS: nothing that can queue a message may follow it.
    appl_exit();

    // `if (rc == FALSE) wind_new();` (`:2168-2169`) — "restarting: clean up AES
    // windows", i.e. delete the handles `close_desktop_windows` only closed.
    if !rc {
        wind_new();
    }

    rc
}

/// `close_desktop_windows` (`desk/deskmain.c:1921-1930`, `#if
/// CONF_WITH_READ_INF`) — "close desktop windows":
///
/// ```c
/// while( (pw=win_ontop()) )
/// {
///     wind_close(pw->w_id);           /* close the window */
///     dos_free(pw->w_pnode.p_fbase);  /* free the fnodes */
///     win_free(pw);                   /* free the wnode */
/// }
/// ```
///
/// `win_ontop()` is `wind_get(DESKWH, WF_TOP)` here as everywhere else in the
/// desktop, and the `FNODE` vector lives inside the `WNODE`, so
/// [`global::DeskState::win_free`] performs both of the last two steps. The
/// loop is bounded by the window table because our top handle comes from the
/// AES rather than from the list being emptied: EmuTOS's terminates when
/// `win_free` unlinks the node, ours when the table runs out — same set, no way
/// to spin.
fn close_desktop_windows() {
    for _ in 0..global::NUM_WNODES {
        let pw = wind_get_top();
        if pw == DESKWH {
            break;
        }
        wind_close(pw);
        if global::with(|g| g.win_free(pw)) != Some(true) {
            break;
        }
    }
}

/// Give the dead desktop's memory back — what GEMDOS does for EmuTOS when the
/// desktop *process* ends (`desk/deskstart.S:57-59`, `deskmain.c:2131-2133`
/// *"the desktop process will terminate and the allocated memory will be
/// automatically freed"*), and what `deskmain`'s own restart arm does by hand
/// for the *Read .INF file* re-entry (`:2140-2147`).
///
/// Three callers, all of them a desktop that has just ended:
/// the kernel's `sh_ldapp` DESKTOP arm (`kernel/src/shell_loop.rs`) before
/// the next start, [`run`]'s restart loop, and the kernel's AES-restart path (A106,
/// `kernel/src/lib.rs`).
///
/// Must run **after** whatever clears the AES's `gl_newdesk` registration
/// ([`wind_set_newdesk`]) — [`deskmain`]'s epilog on the desktop's own way out,
/// `tos_api::aes::reinit` on the A106 path. That pointer addresses the object
/// arena this call frees.
pub fn reset() {
    global::reset();
    inf::reset();
    // The VDI handle table does not survive an AES restart either, so the
    // desktop's cached virtual workstation goes with the rest of the state.
    gsx::reset();
}
