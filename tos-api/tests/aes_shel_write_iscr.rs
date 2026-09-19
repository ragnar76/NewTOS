//! A61 — **`shel_write` reads the third declared word, `SH_ISCR` = `int_in[2]`.**
//!
//! The binding declares three `int_in` words (`desk/gembind.c:907-915`,
//! `AES_CTRL_CODE(SHEL_WRITE, 3, 1, 2)`; `include/aesdefs.h:600-602`,
//! `SH_DOEX`/`SH_ISGR`/`SH_ISCR`), and the dispatcher hands all three on:
//!
//! ```c
//! case SHEL_WRITE:                                  /* aes/gemsuper.c:383 */
//!     ret = sh_write(SH_DOEX, SH_ISGR, SH_ISCR, (const char *)SH_PCMD, (const char *)SH_PTAIL);
//! ```
//!
//! `sh_write` takes it as `isover` (`aes/gemshlib.c:157`). In the `SHW_EXEC`
//! arm it is ignored — "`isover` ignored, should currently always be 1"
//! (`:149`); only `SHW_RESCHNG` consumes it (`gl_changerez = 1 + isover`,
//! `:188`, the A-5 gap). NewTOS read `int_in[0..1]` and left the third word
//! lying; this probe pins down that it now arrives, through the very path the
//! A19 clamp governs (`min(control[IN_LEN], I_SIZE)` words are copied into
//! the dispatch-local `int_in`, the rest read as 0).
//!
//! The retained value is inspectable through
//! [`tos_api::aes::shell::sh_isover`], a test hook — nothing in the AES
//! consumes it, exactly as in EmuTOS's `SHW_EXEC` arm.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK};
use tos_api::aes::shell::{sh_isover, SHW_EXEC};

const OP_SHEL_WRITE: i16 = 121;

const CHILD: &[u8] = b"C:\\AUTO\\HELLO.NCP\0";
const CHILD_TAIL: &[u8] = &[5, b'h', b'e', b'l', b'l', b'o'];

/// `shel_write(SHW_EXEC, isgem, isover, cmd, tail)` with `control[IN_LEN]` =
/// `in_len` — the binding's 3 (`gembind.c:914`), or a caller-declared 2 to
/// show the clamp.
fn shel_write_exec(pb: &mut Pb, in_len: i16, isgem: i16, isover: i16) -> i16 {
    pb.poison();
    pb.intin[0] = SHW_EXEC;
    pb.intin[1] = isgem;
    pb.intin[2] = isover;
    pb.addrin[0] = CHILD.as_ptr() as u64;
    pb.addrin[1] = CHILD_TAIL.as_ptr() as u64;
    pb.call(OP_SHEL_WRITE, in_len, 1, 2, 0);
    pb.intout[0]
}

/// The third declared word reaches the shell manager: `isover = 1` is
/// retained as 1, `isover = 0` as 0 — the value tracks the word, it is not a
/// constant.
#[test]
fn shel_write_exec_retains_the_third_declared_word() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    assert_eq!(shel_write_exec(&mut pb, 3, 1, 1), 1, "shel_write always returns 1");
    assert_eq!(
        sh_isover(),
        1,
        "shel_write(SHW_EXEC, isgem, isover = 1, …) must read SH_ISCR = int_in[2] \
         (gemsuper.c:384 hands it to sh_write as `isover`, gemshlib.c:157)"
    );

    assert_eq!(shel_write_exec(&mut pb, 3, 1, 0), 1);
    assert_eq!(sh_isover(), 0, "isover = 0 must be retained as 0 (the value tracks the word)");

    // TOS 4's ARGV request (`sh_wiscr = 1`) and any other value are all just
    // retained — EmuTOS's SHW_EXEC arm gives none of them an effect.
    assert_eq!(shel_write_exec(&mut pb, 3, 0, 7), 1);
    assert_eq!(sh_isover(), 7);
}

/// A caller that declares only two words (`control[IN_LEN] = 2`) hands over no
/// third one: the dispatcher copies `min(IN_LEN, I_SIZE)` words and the rest of
/// the dispatch-local `int_in` is cleared (`gemsuper.c:431-434`; A19), so the
/// manager reads 0 — not the poison the caller's array holds at `[2]`.
#[test]
fn shel_write_exec_with_two_declared_words_reads_zero() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    // Leave a known non-zero value behind first …
    assert_eq!(shel_write_exec(&mut pb, 3, 1, 1), 1);
    assert_eq!(sh_isover(), 1);
    // … then declare two words with a poisoned third: the manager sees 0.
    assert_eq!(shel_write_exec(&mut pb, 2, 1, 0x5A5A), 1);
    assert_eq!(
        sh_isover(),
        0,
        "with control[IN_LEN] = 2 the third word is outside the declaration and \
         must read as the cleared dispatch-local 0 (A19 clamp), not the caller's memory"
    );
}
