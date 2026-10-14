//! PROC05 (DESK43 U9-0b) — **the shell gets the child's exit code, and only a
//! load error raises an alert.**
//!
//! EmuTOS:
//!
//! ```c
//! void xterm(UWORD rc) { … run->p_dreg[0] = rc; … }       /* bdos/proc.c:601,616 */
//!
//! ret = dos_exec(PE_LOADGO, D.s_cmd, ad_stail, ad_envrn);  /* aes/gemshlib.c:645 */
//! …
//! return (ret < 0) ? ret : 0;                              /* :681-684 */
//!
//! if (rc && !IS_BIOS_ERROR(rc)) fm_error(-rc-31);          /* sh_main, :722-723 */
//! ```
//!
//! A child's exit code reaches the parent as a `UWORD` **zero-extended** into
//! the LONG `dos_exec` returns, so it is `0 … 65535` and never negative —
//! `Pterm(-1)` (a crashed program, a Rust panic) arrives as 65535, not −1 — and
//! `sh_ldapp` turns every non-negative value into 0: no alert for any exit
//! code. Only a load error (negative `xexec` result) reaches `fm_error`.
//! Spec: `docs/api/aes/shell-loop.md` §8 ("the child's exit code"),
//! `docs/analysis/desk43-u9-prep-2026-10-14.md` §2.
//!
//! The kernel's launcher and shell loop (`kernel/src/launch.rs`,
//! `kernel/src/shell_loop.rs`) cannot be host-tested (the `kernel` crate is
//! `aarch64-unknown-none-softfloat` only); they call the three functions
//! tested here for every value they hand on.

use tos_api::aes::shell::{is_bios_error, sh_ldapp_result, sh_main_alert};
use tos_api::bdos::process::pterm::parent_exit_code;
use tos_types::error::{
    E_OK, EACCDN, EDRIVE, EDRVNR, EFILNF, EINVFN, ENHNDL, ENSMEM, EPLFMT, EPTHNF, ERROR,
};

/// What `sh_main` shows for a child that ended with `Pterm(code)`: the alert
/// number `fm_error` would get, or `None`.
fn alert_for_exit(code: i32) -> Option<i16> {
    sh_main_alert(sh_ldapp_result(parent_exit_code(code)))
}

#[test]
fn exit_code_is_the_uword_zero_extended() {
    assert_eq!(parent_exit_code(0), 0);
    assert_eq!(parent_exit_code(1), 1);
    assert_eq!(parent_exit_code(-1), 65535);
    assert_eq!(parent_exit_code(0x12345), 0x2345);
    assert_eq!(parent_exit_code(0xFFFF), 65535);
    // The console Ctrl-C's `xterm(-32)` (`bdos/console.c:101`) and FreeMiNT's
    // `-998` nesting overflow, masked the same way.
    assert_eq!(parent_exit_code(EINVFN), 65504);
    assert_eq!(parent_exit_code(-998), 64538);
    for code in [i32::MIN, -66, -33, -1, 0, 1, 255, 65535, 65536, i32::MAX] {
        let r = parent_exit_code(code);
        assert!((0..=65535).contains(&r), "{code} -> {r}");
    }
}

#[test]
fn no_exit_code_raises_an_alert() {
    for code in [0, 1, 2, 33, 255, 0x7FFF, 0x8000, 65535, -1, -33, -66, 0x12345] {
        assert_eq!(sh_ldapp_result(parent_exit_code(code)), 0, "Pterm({code})");
        assert_eq!(alert_for_exit(code), None, "Pterm({code})");
    }
}

#[test]
fn sh_ldapp_passes_load_errors_on_and_zeroes_the_rest() {
    assert_eq!(sh_ldapp_result(EFILNF), EFILNF);
    assert_eq!(sh_ldapp_result(EPLFMT), EPLFMT);
    assert_eq!(sh_ldapp_result(ENSMEM), ENSMEM);
    assert_eq!(sh_ldapp_result(E_OK), 0);
    assert_eq!(sh_ldapp_result(65535), 0);
}

/// The alert table of `docs/analysis/desk43-u9-prep-2026-10-14.md` §2.1: the
/// `fm_error(-rc-31)` numbers for the load errors, none for the BIOS range.
#[test]
fn load_errors_map_to_their_fm_error_number() {
    let table: [(i32, i16); 7] = [
        (EFILNF, 2),  // AL18ERR
        (EPTHNF, 3),  // AL18ERR
        (ENHNDL, 4),  // AL04ERR
        (EACCDN, 5),  // AL05ERR
        (ENSMEM, 8),  // AL08ERR
        (EDRIVE, 15), // AL15ERR
        (EPLFMT, 35), // ALXXERR "TOS error #35"
    ];
    for (rc, n) in table {
        assert_eq!(sh_main_alert(sh_ldapp_result(rc)), Some(n), "rc {rc}");
    }
    // `IS_BIOS_ERROR`: -1 … -31 raise nothing; EINVFN (-32) is the first
    // GEMDOS code and does.
    for rc in [ERROR, EDRVNR, -17, -31] {
        assert!(is_bios_error(rc), "{rc}");
        assert_eq!(sh_main_alert(rc), None, "rc {rc}");
    }
    assert!(!is_bios_error(EINVFN));
    assert!(!is_bios_error(E_OK));
    assert_eq!(sh_main_alert(EINVFN), Some(1));
    assert_eq!(sh_main_alert(E_OK), None);
}
