//! DESK60 — **after a `#Z` program that called `shel_write(SHW_NOEXEC)`, the
//! desktop pass's `D.s_cmd` is the `#Z` program's file name, as in EmuTOS.**
//!
//! The kernel log showed `sh_ldapp: starting DESKTOP.NAP` when the desktop came
//! back after a `#Z`-started TeraDesk (`measurements/measurement-2026-10-15-
//! desk43-acceptance`, `tdqed` runs). EmuTOS's shell does the same:
//!
//! ```c
//! /* geminit.c:455 (process_inf2, #Z) */   sh_wdef(tmpptr2, tmpptr1);
//! /* gemshlib.c:698   sh_main */            psh->sh_nextapp = isauto ? AUTORUN_APP : DESKTOP_APP;
//! /* gemshlib.c:551-565 sh_chdef */         case AUTORUN_APP: case DESKTOP_APP: ...
//!                                           strcpy(D.s_cmd+n, psh->sh_desk);  /* dir only for AUTORUN */
//! /* gemshlib.c:161-166 sh_write */         case SHW_NOEXEC: strcpy(D.s_cmd, DEF_DESKTOP);
//!                                           psh->sh_nextapp = DESKTOP_APP; ...   /* sh_desk untouched */
//! /* gemshlib.c:662-666 sh_ldapp */         if (psh->sh_nextapp == AUTORUN_APP) set_default_desktop(psh);
//! ```
//!
//! TeraDesk calls `shel_write(SHW_NOEXEC, 1, 0, "", "\0\0")` after every
//! program it `Pexec`s (`sdk/apps/teradesk/startprg.c:386`). When it then
//! quits, `sh_nextapp` is `DESKTOP_APP`, so the AUTORUN return skips
//! `set_default_desktop` and `sh_desk` still holds the `#Z` file name; the next
//! `sh_chdef` builds `D.s_cmd = "DESKTOP.NAP"`. `sh_ldapp`'s DESKTOP arm starts
//! the desktop regardless (`deskstart`, `:606`) and names its descriptor
//! `p_nameit(rlr, sh_name(D.s_cmd))` = `DESKTOP` (`:594,604`); `shel_read`
//! answers `DESKTOP.NAP` during that desktop run (`:104`).
//!
//! So only the kernel's log line was misleading (fixed in
//! `kernel/src/shell_loop.rs`); this file pins the shell state machine in
//! `tos_api::aes::shell` to the EmuTOS sequence above, plus the contrast case
//! (the `#Z` program quits without `shel_write`: `set_default_desktop`,
//! `D.s_cmd = DEF_DESKTOP`).

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK};
use tos_api::aes::shell::{
    s_cmd, set_default_desktop, sh_chdef, sh_main_init, sh_name, sh_nextapp, sh_wdef,
    AUTORUN_APP, DEF_DESKTOP, DESKTOP_APP,
};

const OP_SHEL_READ: i16 = 120;
const OP_SHEL_WRITE: i16 = 121;
/// `SHW_NOEXEC` (`include/aesdefs.h`) — "exit to desktop".
const SHW_NOEXEC: i16 = 0;

/// The `#Z` pair `process_inf2` hands `sh_wdef` for
/// `#Z 01 C:\Apps\TERADESK\DESKTOP.NAP@` (`geminit.c:449-455`: the last
/// separator becomes the split point, no trailing separator on the directory).
const Z_PROG: &[u8] = b"DESKTOP.NAP\0";
const Z_DIR: &[u8] = b"C:\\Apps\\TERADESK\0";

fn cstr(b: &[u8]) -> &[u8] {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    &b[..n]
}

fn cur_cmd() -> Vec<u8> {
    let mut b = [0u8; 128];
    let n = s_cmd(&mut b);
    b[..n].to_vec()
}

/// TeraDesk's `shel_write(SHW_NOEXEC, 1, 0, "", "\0\0")` (`startprg.c:386`).
fn shel_write_noexec(pb: &mut Pb) -> i16 {
    static EMPTY: [u8; 1] = [0];
    static TAIL: [u8; 2] = [0, 0];
    pb.poison();
    pb.intin[0] = SHW_NOEXEC;
    pb.intin[1] = 1;
    pb.intin[2] = 0;
    pb.addrin[0] = EMPTY.as_ptr() as u64;
    pb.addrin[1] = TAIL.as_ptr() as u64;
    pb.call(OP_SHEL_WRITE, 3, 1, 2, 0);
    pb.intout[0]
}

fn shel_read_cmd(pb: &mut Pb) -> Vec<u8> {
    let mut cmd = Box::new([0xEEu8; 256]);
    let mut tail = Box::new([0xEEu8; 256]);
    pb.poison();
    pb.addrin[0] = cmd.as_mut_ptr() as u64;
    pb.addrin[1] = tail.as_mut_ptr() as u64;
    pb.call(OP_SHEL_READ, 0, 1, 2, 0);
    assert_eq!(pb.intout[0], 1, "shel_read returns 1");
    cstr(&cmd[..]).to_vec()
}

/// The kernel's post-run step of `sh_ldapp` (`gemshlib.c:662-666`,
/// `kernel/src/shell_loop.rs`): read `sh_nextapp` live after the child ran.
fn autorun_return() {
    if sh_nextapp() == AUTORUN_APP {
        set_default_desktop();
    }
}

/// The `#Z` boot: first pass runs the autorun program with its full path.
fn boot_autorun() {
    sh_wdef(Z_PROG, Z_DIR);
    sh_main_init(true, true);
    assert_eq!(sh_nextapp(), AUTORUN_APP, "sh_main: isauto -> AUTORUN_APP (:698)");
    sh_chdef();
    assert_eq!(
        cur_cmd(),
        b"C:\\Apps\\TERADESK\\DESKTOP.NAP".to_vec(),
        "sh_chdef AUTORUN_APP: sh_cdir + PATHSEP + sh_desk (:557-564)"
    );
}

/// The `#Z` program called `shel_write(SHW_NOEXEC)` and quit: the next pass
/// starts the desktop with `D.s_cmd` = the `#Z` file name, unqualified.
#[test]
fn noexec_from_autorun_program_leaves_its_name_in_s_cmd() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    boot_autorun();
    assert_eq!(shel_write_noexec(&mut pb), 1, "shel_write always returns 1");
    assert_eq!(sh_nextapp(), DESKTOP_APP, "SHW_NOEXEC -> sh_nextapp = DESKTOP_APP (:165)");
    assert_eq!(cur_cmd(), cstr(DEF_DESKTOP).to_vec(), "SHW_NOEXEC -> D.s_cmd = DEF_DESKTOP (:163)");

    autorun_return();
    assert_eq!(sh_nextapp(), DESKTOP_APP, "no set_default_desktop: sh_nextapp was not AUTORUN_APP");

    // Second pass of sh_main.
    sh_chdef();
    assert_eq!(
        cur_cmd(),
        b"DESKTOP.NAP".to_vec(),
        "sh_chdef DESKTOP_APP: D.s_cmd = sh_desk, still the #Z program's name (:564) -- \
         EmuTOS behaves the same; the DESKTOP arm starts the desktop regardless"
    );
    assert_eq!(sh_name(&cur_cmd()), b"DESKTOP.NAP", "p_nameit's input (:594)");
    assert_eq!(
        shel_read_cmd(&mut pb),
        b"DESKTOP.NAP".to_vec(),
        "shel_read answers D.s_cmd during the desktop run (:104)"
    );
}

/// Contrast: the `#Z` program quits without `shel_write` — the AUTORUN
/// return resets to the default desktop and `D.s_cmd` = `DEF_DESKTOP`.
#[test]
fn autorun_program_quitting_plainly_returns_to_def_desktop() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();

    boot_autorun();
    autorun_return();
    assert_eq!(sh_nextapp(), DESKTOP_APP, "set_default_desktop (:585)");
    sh_chdef();
    assert_eq!(
        cur_cmd(),
        cstr(DEF_DESKTOP).to_vec(),
        "set_default_desktop: sh_desk = DEF_DESKTOP (:587), so D.s_cmd = DEF_DESKTOP"
    );
}
