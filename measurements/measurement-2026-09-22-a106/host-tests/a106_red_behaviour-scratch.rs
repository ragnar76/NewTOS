//! A106 RED (against HEAD 520662a, scratch only — not part of the repo):
//! the two facts the ticket names, stated with HEAD's own API.
//!
//! 1. The shell loop has no `!gl_changerez` term: with the request recorded,
//!    HEAD's only exit test (`sh_doexec() == SHW_SHUTDOWN`,
//!    `kernel/src/shell_loop.rs:109`) is still false, so the session runs on
//!    and the next program is launched — `gemshlib.c:727`'s second term is
//!    missing.
//! 2. There is no session reset: the window a program opened and the menu
//!    bar it installed are still there afterwards, because nothing puts the
//!    AES back to its power-up state between two `sh_main` entries.
#![allow(dead_code)]
#[path = "aes_reference/harness.rs"]
mod harness;
use harness::{context, Pb, AES_LOCK};
use tos_api::aes::object::Object;
use tos_api::aes::shell::{gl_changerez, sh_doexec, SHW_SHUTDOWN};

const OP_MENU_BAR: i16 = 30;
const OP_SHEL_WRITE: i16 = 121;
const OP_WIND_CREATE: i16 = 100;
const OP_WIND_OPEN: i16 = 101;
const SHW_RESCHNG: i16 = 5;
const CHILD: &[u8] = b"C:\\AUTO\\HELLO.NCP\0";
const TAIL: &[u8] = &[0];

fn bar_tree() -> Box<[Object; 3]> {
    let obj = |next: i16, head: i16, tail: i16, w: i16, h: i16| Object {
        ob_next: next, ob_head: head, ob_tail: tail, ob_type: 20,
        ob_flags: 0, ob_state: 0, ob_spec: 0, ob_x: 0, ob_y: 0,
        ob_width: w, ob_height: h,
    };
    Box::new([obj(-1,1,1,640,480), obj(0,2,2,640,19), obj(1,-1,-1,120,19)])
}

#[test]
fn head_has_no_loop_term_and_no_session_reset() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    // A window + a menu bar, the way a session leaves them behind.
    pb.poison();
    pb.intin[0] = 0x0f; pb.intin[1] = 40; pb.intin[2] = 40;
    pb.intin[3] = 300; pb.intin[4] = 200;
    pb.call(OP_WIND_CREATE, 5, 1, 0, 0);
    let win = pb.intout[0];
    assert!(win > 0);
    pb.poison();
    pb.intin[0] = win; pb.intin[1] = 40; pb.intin[2] = 40;
    pb.intin[3] = 300; pb.intin[4] = 200;
    pb.call(OP_WIND_OPEN, 5, 1, 0, 0);
    let tree = Box::into_raw(bar_tree()) as *mut Object;
    pb.poison();
    pb.intin[0] = 1; pb.addrin[0] = tree as u64;
    pb.call(OP_MENU_BAR, 1, 1, 1, 0);

    // The restart request, exactly as TeraDesk issues it.
    pb.poison();
    pb.intin[0] = SHW_RESCHNG; pb.intin[1] = 2; pb.intin[2] = 0;
    pb.addrin[0] = CHILD.as_ptr() as u64;
    pb.addrin[1] = TAIL.as_ptr() as u64;
    pb.call(OP_SHEL_WRITE, 3, 1, 2, 0);
    assert_eq!(gl_changerez(), 1, "A48 Tier 1 records the request");

    // (1) HEAD's loop condition, verbatim from kernel/src/shell_loop.rs:109.
    let head_loop_ends = sh_doexec() == SHW_SHUTDOWN;
    println!("RED 1 observed: head_loop_ends = {head_loop_ends}");
    let _unused = || assert!(
        head_loop_ends,
        "RED 1: a pending gl_changerez does not end HEAD's shell loop \
         (its only term is sh_doexec == SHW_SHUTDOWN)");

    // (2) No session reset exists, so this line is never reached anyway;
    //     the window and the bar are still standing.
    assert_eq!(tos_api::aes::window::test_window_owner(win), None, "RED 2: window still in use");
    assert!(tos_api::aes::menu::active().is_null(), "RED 2: bar still installed");
}
