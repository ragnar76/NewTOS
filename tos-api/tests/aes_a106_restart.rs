//! A106 — **the AES session ends on a pending `SHW_RESCHNG`, and the next
//! session starts from a clean AES.**
//!
//! ```c
//! do {
//!     sh_chdef(psh); sh_chgrf(psh);
//!     if (gl_shgem) { wm_init(); ratinit(); sh_draw(D.s_cmd, TRUE); }
//!     if (rc && !IS_BIOS_ERROR(rc)) fm_error(-rc-31);
//!     rc = sh_ldapp(psh);
//! } while ((psh->sh_doexec != SHW_SHUTDOWN) && !gl_changerez);  /* :727 */
//! ```
//! (`references/emutos/aes/gemshlib.c:708-731`.) A `shel_write(SHW_RESCHNG,
//! …)` does not interrupt the running program (A48): it records
//! `gl_changerez`/`gl_nextrez` and returns. The loop notices the next time it
//! re-checks its condition — i.e. once `sh_ldapp` returns, which is when the
//! running program terminated — and leaves **without launching the next
//! program**. `gem_main` then returns to `gemstart.S:86-124`, which re-zeroes
//! the whole `theglo` AES global area and re-enters `gem_main`: every AES
//! structure is rebuilt and the accessories + desktop are run again.
//!
//! NewTOS's AES state is a set of module statics rather than one `D` struct,
//! so "re-zero `theglo`" is [`tos_api::aes::reinit`], and the outer loop is
//! the kernel's session loop around `sh_main` (`kernel/src/lib.rs`), which
//! the `kernel` crate cannot host-test (it is `forced-target`
//! `aarch64-unknown-none-softfloat`). What *is* host-testable, and is what
//! this file tests, are the two halves that decide and perform the restart:
//! the loop's exit term ([`tos_api::aes::shell::sh_main_continue`] /
//! [`sh_main_exit`], the literal `:727` condition the loop calls) and the
//! session reset itself.
//!
//! Design: `docs/analysis/a48-shw-reschng-design-2026-09-21.md` §5 tier 2,
//! §9 "Tier 2 (host + QEMU)". Spec: `docs/api/aes/shell-loop.md` §7.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK};
use tos_api::aes::object::Object;
use tos_api::aes::shell::{
    gl_changerez, gl_nextrez, sh_doexec, sh_main_continue, sh_main_exit, ShExit, SHW_EXEC,
    SHW_SHUTDOWN,
};

const OP_APPL_INIT: i16 = 10;
const OP_APPL_WRITE: i16 = 12;
const OP_EVNT_DCLICK: i16 = 26;
const OP_MENU_BAR: i16 = 30;
const OP_MENU_SETTINGS: i16 = 39;
const OP_OBJC_SYSVAR: i16 = 48;
const OP_RSRC_GADDR: i16 = 112;
const OP_SHEL_WRITE: i16 = 121;
const OP_WIND_CREATE: i16 = 100;
const OP_WIND_OPEN: i16 = 101;

/// `SHW_RESCHNG` (`include/aesdefs.h:621`).
const SHW_RESCHNG: i16 = 5;

/// `NAME | CLOSER | FULLER | MOVER` — an ordinary window's gadgets.
const WINDOW_KIND: i16 = 0x0f;

/// The embedded `GEM.RSC`, the one NRSC blob this crate ships — a resource
/// to put in a process's list so the restart has something to drop.
const RSC: &[u8] = include_bytes!("../src/aes/gem_fsel.nrsc");

const CHILD: &[u8] = b"C:\\AUTO\\HELLO.NCP\0";
const CHILD_TAIL: &[u8] = &[0];

// ── AES calls through the real dispatcher ─────────────────────────────────

/// `shel_write(doex, isgem, isover, cmd, tail)` (`gembind.c:914`).
fn shel_write(pb: &mut Pb, doex: i16, isgem: i16, isover: i16) -> i16 {
    pb.poison();
    pb.intin[0] = doex;
    pb.intin[1] = isgem;
    pb.intin[2] = isover;
    pb.addrin[0] = CHILD.as_ptr() as u64;
    pb.addrin[1] = CHILD_TAIL.as_ptr() as u64;
    pb.call(OP_SHEL_WRITE, 3, 1, 2, 0);
    pb.intout[0]
}

fn appl_init(pb: &mut Pb) -> i16 {
    pb.poison();
    pb.call(OP_APPL_INIT, 0, 1, 0, 0);
    pb.intout[0]
}

/// `appl_write(id, length, pbuff)` — put one 16-byte GEM message into the
/// descriptor's FIFO, so the reset has a queue to drain.
fn appl_write(pb: &mut Pb, id: i16, msg: &[i16; 8]) -> i16 {
    pb.poison();
    pb.intin[0] = id;
    pb.intin[1] = 16;
    pb.addrin[0] = msg.as_ptr() as u64;
    pb.call(OP_APPL_WRITE, 2, 1, 1, 0);
    pb.intout[0]
}

fn wind_create(pb: &mut Pb, x: i16, y: i16, w: i16, h: i16) -> i16 {
    pb.poison();
    pb.intin[0] = WINDOW_KIND;
    pb.intin[1] = x;
    pb.intin[2] = y;
    pb.intin[3] = w;
    pb.intin[4] = h;
    pb.call(OP_WIND_CREATE, 5, 1, 0, 0);
    pb.intout[0]
}

fn wind_open(pb: &mut Pb, handle: i16, x: i16, y: i16, w: i16, h: i16) -> i16 {
    pb.poison();
    pb.intin[0] = handle;
    pb.intin[1] = x;
    pb.intin[2] = y;
    pb.intin[3] = w;
    pb.intin[4] = h;
    pb.call(OP_WIND_OPEN, 5, 1, 0, 0);
    pb.intout[0]
}

fn menu_bar(pb: &mut Pb, tree: *mut Object, show: i16) -> i16 {
    pb.poison();
    pb.intin[0] = show;
    pb.addrin[0] = tree as u64;
    pb.call(OP_MENU_BAR, 1, 1, 1, 0);
    pb.intout[0]
}

/// `evnt_dclick(rate, setit)` — get (`setit == 0`) or set the click rate.
fn evnt_dclick(pb: &mut Pb, rate: i16, setit: i16) -> i16 {
    pb.poison();
    pb.intin[0] = rate;
    pb.intin[1] = setit;
    pb.call(OP_EVNT_DCLICK, 2, 1, 0, 0);
    pb.intout[0]
}

/// `objc_sysvar(mode, which, in1, in2)`; a GET answers in `int_out[1..3]`.
fn objc_sysvar(pb: &mut Pb, mode: i16, which: i16, in1: i16, in2: i16) -> (i16, i16, i16) {
    pb.poison();
    pb.intin[0] = mode;
    pb.intin[1] = which;
    pb.intin[2] = in1;
    pb.intin[3] = in2;
    pb.call(OP_OBJC_SYSVAR, 4, 3, 0, 0);
    (pb.intout[0], pb.intout[1], pb.intout[2])
}

/// `rsrc_gaddr(type, index, &addr)` — `R_TREE` (0), tree 0.
fn rsrc_gaddr_tree0(pb: &mut Pb) -> (i16, u64) {
    pb.poison();
    pb.intin[0] = 0;
    pb.intin[1] = 0;
    pb.call(OP_RSRC_GADDR, 2, 1, 0, 1);
    (pb.intout[0], pb.addrout[0])
}

/// `MN_SET` (`menu_settings`' behaviour block).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct MnSet {
    display: i32,
    drag: i32,
    delay: i32,
    speed: i32,
    height: i16,
}

fn menu_settings(pb: &mut Pb, flag: i16, block: &mut MnSet) -> i16 {
    pb.poison();
    pb.intin[0] = flag;
    pb.addrin[0] = block as *mut MnSet as u64;
    pb.call(OP_MENU_SETTINGS, 1, 1, 1, 0);
    pb.intout[0]
}

/// The smallest tree `mn_bar(tree, TRUE)` accepts: `THESCREEN` (0) with
/// `THEBAR` (1) and `THEACTIVE` (2) under it — `install_bar` takes
/// `THEACTIVE`'s absolute rectangle (`gemmnlib.c:717`).
fn bar_tree() -> Box<[Object; 3]> {
    let obj = |next: i16, head: i16, tail: i16, w: i16, h: i16| Object {
        ob_next: next,
        ob_head: head,
        ob_tail: tail,
        ob_type: 20, // G_BOX
        ob_flags: 0,
        ob_state: 0,
        ob_spec: 0,
        ob_x: 0,
        ob_y: 0,
        ob_width: w,
        ob_height: h,
    };
    Box::new([
        obj(-1, 1, 1, 640, 480),
        obj(0, 2, 2, 640, 19),
        obj(1, -1, -1, 120, 19),
    ])
}

// ── 1. the loop's exit term (`gemshlib.c:727`) ────────────────────────────

/// **Red before A106:** `sh_main_continue`/`sh_main_exit` did not exist and
/// the loop's only term was `sh_doexec != SHW_SHUTDOWN`, so a pending
/// `SHW_RESCHNG` left the session running.
///
/// `while ((psh->sh_doexec != SHW_SHUTDOWN) && !gl_changerez)`: with the
/// request recorded the condition is false and the reason is the resolution
/// change, which is the term `gemstart.S:122-124` acts on.
#[test]
fn a_pending_reschng_ends_the_shell_loop_with_the_restart_reason() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    // A session with nothing pending keeps running.
    tos_api::aes::reinit();
    assert!(sh_main_continue(), "no request pending: the loop runs on");

    // TeraDesk's request on NewTOS (`video.c:363-365`: ST-low + 2, isover 0).
    assert_eq!(shel_write(&mut pb, SHW_RESCHNG, 2, 0), 1);
    assert_eq!((gl_changerez(), gl_nextrez()), (1, 2));
    assert!(
        !sh_main_continue(),
        "`!gl_changerez` is the second term of gemshlib.c:727"
    );
    assert_eq!(
        sh_main_exit(),
        ShExit::RezChange,
        "gl_changerez is the term gemstart.S:122-124 branches on"
    );

    // The restart consumes it (`gl_changerez = FALSE`, geminit.c:806) and the
    // new session's loop runs again.
    tos_api::aes::reinit();
    assert_eq!(gl_changerez(), 0, "gem_main clears the request (geminit.c:806)");
    assert_eq!(gl_nextrez(), 0);
    assert!(sh_main_continue(), "the restarted session's loop runs");
}

/// The shutdown term is the other half of `:727`, and on this build profile
/// it is the one nothing can raise: `sh_write`'s `SHW_SHUTDOWN` arm records
/// the request only `if (can_shutdown())` (`gemshlib.c:178-186`), which is
/// `FALSE` for every machine without a native power-off, the Falcon this
/// profile targets included (`bios/bios.c:754-770`, the `#else return FALSE`
/// arm) — so `sh_doexec` keeps its value and the loop keeps running, exactly
/// as on a real Falcon. When the loop *does* end without a pending
/// resolution change, the reason reported is the shutdown one.
#[test]
fn the_shutdown_term_is_unreachable_on_this_profile_and_is_the_other_reason() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    tos_api::aes::reinit();

    assert_eq!(shel_write(&mut pb, SHW_SHUTDOWN, 0, 2), 1);
    assert_ne!(sh_doexec(), SHW_SHUTDOWN, "can_shutdown() is FALSE");
    assert!(sh_main_continue(), "the loop keeps running (bios.c:768)");
    assert_eq!(
        sh_main_exit(),
        ShExit::Shutdown,
        "with no rez change pending the exit reason is the shutdown one"
    );

    // And a `SHW_SHUTDOWN` does not disturb a pending restart request.
    assert_eq!(shel_write(&mut pb, SHW_RESCHNG, 6, 0), 1);
    assert_eq!(shel_write(&mut pb, SHW_SHUTDOWN, 0, 2), 1);
    assert_eq!((gl_changerez(), gl_nextrez()), (1, 6));
    assert_eq!(sh_main_exit(), ShExit::RezChange);
    tos_api::aes::reinit();
}

// ── 2. the session reset (`gem_main` + `run_accs_and_desktop`) ────────────

/// **Red before A106:** nothing reset these statics between two `sh_main`
/// entries — there was no second entry.
///
/// Every structure a session leaves behind is gone when the next one starts,
/// the way `aes_restart`'s `theglo` re-zero plus `gem_main`'s init leave it:
/// no window, no `WF_NEWDESK` tree, no menu bar, no queued message, no
/// loaded resource, no application directory, and the `menu_settings` /
/// `objc_sysvar` / click-rate blocks back at their power-up values.
#[test]
fn a_restart_leaves_the_aes_as_a_cold_boot_does() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    tos_api::aes::reinit();

    // ── seed a session's worth of state ───────────────────────────────────
    let ap_id = appl_init(&mut pb);
    assert!(ap_id >= 0, "appl_init failed ({ap_id})");

    let win = wind_create(&mut pb, 40, 40, 300, 200);
    assert!(win > 0, "wind_create failed ({win})");
    assert_eq!(wind_open(&mut pb, win, 40, 40, 300, 200), 1);
    assert!(
        tos_api::aes::window::test_window_owner(win).is_some(),
        "seed: the window is in use"
    );

    let tree = bar_tree();
    let tree_ptr = Box::into_raw(tree) as *mut Object;
    assert_eq!(menu_bar(&mut pb, tree_ptr, 1), 1);
    assert_eq!(
        tos_api::aes::menu::active(),
        tree_ptr,
        "seed: the bar is installed"
    );

    assert_eq!(appl_write(&mut pb, ap_id, &[0x4711, ap_id, 0, 1, 2, 3, 4, 5]), 1);
    assert!(
        tos_api::aes::window::test_msg_count(ap_id) > 0,
        "seed: the descriptor's FIFO holds messages (this one, plus whatever \
         the window open posted)"
    );

    // A resource in this identity's list, registered exactly as
    // `rsrc_load`'s in-kernel branch registers it.
    let mut blob = vec![0u64; RSC.len().div_ceil(8) + 1];
    // SAFETY: `blob` holds at least `RSC.len()` writable bytes, and outlives
    // every `rsrc_gaddr` below.
    unsafe {
        std::ptr::copy_nonoverlapping(RSC.as_ptr(), blob.as_mut_ptr() as *mut u8, RSC.len());
        assert!(
            tos_api::aes::resource::relocate_and_register(
                blob.as_mut_ptr() as *mut u8,
                RSC.len()
            ),
            "seed: the blob must relocate and register"
        );
    }
    assert_eq!(rsrc_gaddr_tree0(&mut pb).0, 1, "seed: rsrc_gaddr finds tree 0");

    tos_api::aes::resource::set_app_dir(b"C:\\APPS\\QED\\QED.NAP\0");
    let mut dir = [0u8; 128];
    assert!(tos_api::aes::resource::app_dir(&mut dir) > 0, "seed: app dir set");

    // Behaviour blocks moved away from their power-up values.
    assert_eq!(evnt_dclick(&mut pb, 0, 1), 0, "seed: click rate 0");
    assert_eq!(objc_sysvar(&mut pb, 1, 1, 1, 0), (1, 0, 0), "seed: LK3DIND set");
    let mut set = MnSet { display: 42, drag: 43, delay: 44, speed: 45, height: 9 };
    assert_eq!(menu_settings(&mut pb, 1, &mut set), 1, "seed: menu_settings set");

    // ── the restart ───────────────────────────────────────────────────────
    assert_eq!(shel_write(&mut pb, SHW_RESCHNG, 2, 0), 1);
    assert!(!sh_main_continue());
    tos_api::aes::reinit();
    // SAFETY: nothing holds the tree any more — `menu::reinit` dropped the
    // pointer, which is what the assertion below states.
    unsafe { drop(Box::from_raw(tree_ptr as *mut [Object; 3])) };

    // ── the new session starts on a clean AES ─────────────────────────────
    assert_eq!(
        tos_api::aes::window::test_window_owner(win),
        None,
        "wm_init: every window is gone (gemwmlib.c:1247-1310)"
    );
    assert_eq!(
        tos_api::aes::window::test_newdesk_tree(),
        0,
        "wm_init: gl_newdesk = NULL (gemwmlib.c:1300)"
    );
    assert!(
        tos_api::aes::menu::active().is_null(),
        "mn_bar(…, FALSE)'s state: no menu tree (gemmnlib.c:726-727)"
    );
    assert_eq!(
        tos_api::aes::window::test_msg_count(ap_id),
        0,
        "the process table and its queues are rebuilt (geminit.c:846-880)"
    );
    assert_eq!(
        rsrc_gaddr_tree0(&mut pb).0,
        0,
        "the resource list is empty again"
    );
    assert_eq!(
        tos_api::aes::resource::app_dir(&mut dir),
        0,
        "p_appdir is empty again (geminit.c:871)"
    );
    assert_eq!(
        evnt_dclick(&mut pb, 0, 0),
        3,
        "ev_dclick(3, TRUE) at session start (geminit.c:701)"
    );
    assert_eq!(
        objc_sysvar(&mut pb, 0, 1, 0, 0),
        (1, 0, 1),
        "init_3d: indtxtmove = 0, indcolchange = 1 (gemoblib.c:1327-1328)"
    );
    let mut got = MnSet { display: -1, drag: -1, delay: -1, speed: -1, height: -1 };
    assert_eq!(menu_settings(&mut pb, 0, &mut got), 1);
    assert_eq!(
        got,
        MnSet { display: 200, drag: 10000, delay: 250, speed: 0, height: 16 },
        "mnext_init: the INIT_*_VALUE block (gemmnext.c:103-110)"
    );

    // The shell state is the power-up one, so `sh_main_init` can seed it
    // exactly as it does at cold boot.
    assert_eq!(sh_doexec(), SHW_EXEC);
    assert_eq!(gl_changerez(), 0);

    // A second restart of the same session works the same way — the QEMU
    // acceptance runs the sequence twice.
    let win2 = wind_create(&mut pb, 40, 40, 300, 200);
    assert_eq!(win2, win, "a clean table hands out the same first handle");
    assert_eq!(shel_write(&mut pb, SHW_RESCHNG, 2, 0), 1);
    assert_eq!(sh_main_exit(), ShExit::RezChange);
    tos_api::aes::reinit();
    assert_eq!(tos_api::aes::window::test_window_owner(win2), None);
    assert!(sh_main_continue());

    // Leave the binary's shared context usable for whatever runs next.
    appl_init(&mut pb);
}
