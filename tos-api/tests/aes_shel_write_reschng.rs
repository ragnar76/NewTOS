//! A48 — **`shel_write(SHW_RESCHNG, isgem, isover, …)` records the restart
//! request exactly as `sh_write` does.**
//!
//! ```c
//! case SHW_RESCHNG:   /* change resolution */          /* aes/gemshlib.c:187 */
//!     gl_changerez = 1 + isover;                       /* :188 */
//!     gl_nextrez = isgem;                              /* :189 */
//!     D.s_cmd[0] = '\0';                               /* :190 */
//!     ad_stail[0] = '\0';                              /* :191 */
//!     break;
//! }
//! return TRUE;                                         /* :195 */
//! ```
//!
//! The arm is **unconditional**: no `check_moderez`, no comparison with the
//! current mode, no rejection path — that filtering lives in the *caller*
//! (`desk/deskrez.c:213-215`), not in `sh_write`
//! (`docs/analysis/a48-shw-reschng-design-2026-09-21.md` §1). It touches
//! none of the `SHELL` members (`sh_doexec`/`sh_nextapp`/`sh_isgem` keep
//! whatever the previous `shel_write` left). NewTOS reproduced only the
//! buffer clear (`:190-191`) and recorded no restart intent at all
//! (`_full-function-inventory.md` Teil II, A-5).
//!
//! Tier 1 of A48: the two globals are recorded and readable through
//! [`tos_api::aes::shell::gl_changerez`]/[`gl_nextrez`]; the shell loop does
//! not consume them yet (Tier 2 = `TASK/A106.md`, the re-enterable `sh_main`
//! wrapper), so `sh_main`'s exit condition is unchanged by this probe.
//!
//! `SHW_SHUTDOWN` is the guard: `can_shutdown()` is `FALSE` for the Falcon
//! profile (`bios/bios.c:754-770`), so the arm records nothing — in
//! particular it must leave `gl_changerez` alone — and still returns `1`.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK};
use tos_api::aes::shell::{
    gl_changerez, gl_nextrez, s_cmd, s_tail_image, sh_doexec, sh_isgem, sh_nextapp, NORMAL_APP,
    SHW_EXEC, SHW_SHUTDOWN,
};

const OP_SHEL_WRITE: i16 = 121;
/// `SHW_RESCHNG` (`include/aesdefs.h:621`).
const SHW_RESCHNG: i16 = 5;

const CHILD: &[u8] = b"C:\\AUTO\\HELLO.NCP\0";
const CHILD_TAIL: &[u8] = &[5, b'h', b'e', b'l', b'l', b'o'];

/// `shel_write(doex, isgem, isover, cmd, tail)` through the binding's
/// declaration (`gembind.c:914`: 3 words in, 1 out, 2 addresses in).
fn shel_write(pb: &mut Pb, doex: i16, isgem: i16, isover: i16, cmd: &[u8], tail: &[u8]) -> i16 {
    pb.poison();
    pb.intin[0] = doex;
    pb.intin[1] = isgem;
    pb.intin[2] = isover;
    pb.addrin[0] = cmd.as_ptr() as u64;
    pb.addrin[1] = tail.as_ptr() as u64;
    pb.call(OP_SHEL_WRITE, 3, 1, 2, 0);
    pb.intout[0]
}

/// Leave a known, non-empty `SHW_EXEC` state behind so the `SHW_RESCHNG`
/// probe can show what it clears and what it leaves alone.
fn seed_exec(pb: &mut Pb) {
    assert_eq!(shel_write(pb, SHW_EXEC, 1, 1, CHILD, CHILD_TAIL), 1);
    let mut cmd = [0u8; 128];
    assert_eq!(s_cmd(&mut cmd), CHILD.len() - 1, "seed: D.s_cmd holds the child");
    let mut tail = [0u8; 128];
    assert_eq!(s_tail_image(&mut tail), CHILD_TAIL.len(), "seed: ad_stail holds the tail");
    assert_eq!(sh_doexec(), SHW_EXEC);
    assert_eq!(sh_nextapp(), NORMAL_APP);
    assert!(sh_isgem());
}

/// The ST/TT-family request TeraDesk issues on NewTOS (`video.c:363-365`:
/// `wisgr = currez + 2; wiscr = 0`): `gl_changerez = 1 + 0`, `gl_nextrez` =
/// the word verbatim (the `+2` is the caller's, D-101), the command/tail
/// cleared, the `SHELL` members untouched, return `1`.
#[test]
fn shel_write_reschng_records_the_st_tt_family_request() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    seed_exec(&mut pb);

    // TT-medium (4) + 2, isover = 0 — exactly what TeraDesk hands over.
    assert_eq!(
        shel_write(&mut pb, SHW_RESCHNG, 6, 0, CHILD, CHILD_TAIL),
        1,
        "sh_write returns TRUE for every mode (gemshlib.c:195)"
    );
    assert_eq!(gl_changerez(), 1, "gl_changerez = 1 + isover with isover = 0 (gemshlib.c:188)");
    assert_eq!(gl_nextrez(), 6, "gl_nextrez = isgem verbatim (gemshlib.c:189)");

    let mut cmd = [0xEEu8; 128];
    assert_eq!(s_cmd(&mut cmd), 0, "D.s_cmd[0] = '\\0' (gemshlib.c:190)");
    let mut tail = [0xEEu8; 128];
    assert_eq!(s_tail_image(&mut tail), 1, "ad_stail[0] = '\\0' (gemshlib.c:191): an empty image");
    assert_eq!(tail[0], 0);

    // The arm does not touch the SHELL structure (gemshlib.c:187-192).
    assert_eq!(sh_doexec(), SHW_EXEC, "sh_doexec is not written by the SHW_RESCHNG arm");
    assert_eq!(sh_nextapp(), NORMAL_APP, "sh_nextapp is not written by the SHW_RESCHNG arm");
    assert!(sh_isgem(), "sh_isgem is not written by the SHW_RESCHNG arm");
}

/// The Falcon-family request (`isover = 1`): `gl_changerez = 2`, and the
/// mode word is stored without any validation — `sh_write` never compares it
/// with the current mode (design §1), so an arbitrary value is recorded as
/// given. A second request simply overwrites the first.
#[test]
fn shel_write_reschng_records_the_falcon_family_request_unvalidated() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    assert_eq!(shel_write(&mut pb, SHW_RESCHNG, 3, 1, CHILD, CHILD_TAIL), 1);
    assert_eq!(gl_changerez(), 2, "gl_changerez = 1 + isover with isover = 1 (gemshlib.c:188)");
    assert_eq!(gl_nextrez(), 3);

    // No validation: a mode word no Falcon has is recorded all the same.
    assert_eq!(shel_write(&mut pb, SHW_RESCHNG, 0x7ABC, 1, CHILD, CHILD_TAIL), 1);
    assert_eq!(gl_changerez(), 2);
    assert_eq!(gl_nextrez(), 0x7ABC, "sh_write validates nothing — the word is stored verbatim");

    // Back to the ST/TT family: the family code follows isover each time.
    assert_eq!(shel_write(&mut pb, SHW_RESCHNG, 2, 0, CHILD, CHILD_TAIL), 1);
    assert_eq!(gl_changerez(), 1);
    assert_eq!(gl_nextrez(), 2);
}

/// Guard: `SHW_SHUTDOWN` records nothing on the Falcon profile
/// (`can_shutdown()` is FALSE, `bios/bios.c:768`; `gemshlib.c:178-186`) — it
/// neither sets nor clears `gl_changerez`/`gl_nextrez`, leaves `sh_doexec`
/// alone, and still returns `1`.
#[test]
fn shel_write_shutdown_records_nothing_and_leaves_the_rez_request_alone() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();
    seed_exec(&mut pb);

    assert_eq!(shel_write(&mut pb, SHW_RESCHNG, 6, 0, CHILD, CHILD_TAIL), 1);
    assert_eq!((gl_changerez(), gl_nextrez()), (1, 6));

    assert_eq!(shel_write(&mut pb, SHW_SHUTDOWN, 0, 2, CHILD, CHILD_TAIL), 1);
    assert_eq!((gl_changerez(), gl_nextrez()), (1, 6), "SHW_SHUTDOWN must not touch the rez request");
    assert_ne!(sh_doexec(), SHW_SHUTDOWN, "SHW_SHUTDOWN is not recorded (can_shutdown() is FALSE)");
    assert_eq!(sh_doexec(), SHW_EXEC);
}
