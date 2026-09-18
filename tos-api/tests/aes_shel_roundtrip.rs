//! H05 — **`shel_read` answers from the buffer `shel_write(SHW_EXEC)` wrote.**
//!
//! EmuTOS keeps *one* command/tail pair, `D.s_cmd`/`ad_stail`:
//!
//! ```c
//! void sh_read(char *pcmd, char *ptail)              /* aes/gemshlib.c:102 */
//! {
//!     strcpy(pcmd, D.s_cmd);
//!     memcpy(ptail, ad_stail, CMDTAILSIZE);
//! }
//! …
//! case SHW_EXEC:                                     /* aes/gemshlib.c:168 */
//!     strcpy(D.s_cmd, pcmd);
//!     memcpy(ad_stail, ptail, CMDTAILSIZE);
//! …
//! ret = dos_exec(PE_LOADGO, D.s_cmd, ad_stail, ad_envrn);   /* :645 */
//! ```
//!
//! `sh_ldapp` runs whatever is in those cells, so the program that starts
//! reads its own start command back (`docs/api/aes/scrap-shell.md`
//! § `shel_read`) — *until* it overwrites the cells with its own
//! `shel_write(SHW_EXEC, …)`, after which `shel_read` returns **that**. TeraDesk's
//! Single-TOS chain protocol is built on the second half: `shel_write(SHW_EXEC,
//! child)` → `Pexec(child)` → `shel_read` → `strcmp(cmd, own name)` decides
//! whether the child asked for a successor (`sdk/apps/teradesk/startprg.c:344-355`).
//! NewTOS answered `shel_read` from a second, launcher-only buffer, so TeraDesk
//! read its *own* start command, took it for a chain request, and started a
//! second TeraDesk (`TASK/H05-STATUS-DONE.MD` § 4-5).
//!
//! Three probes, each under [`AES_LOCK`] because the shell state is a
//! process-wide static:
//!
//! * [`shel_write_exec_then_shel_read_returns_the_written_pair`] — the
//!   round trip, `gemshlib.c:104-105` against `:169-170`;
//! * [`launcher_fills_the_cells_shel_read_returns`] — the launcher's side of
//!   `sh_ldapp` (`kernel/src/launch.rs` → `set_launch_info`), which must fill the
//!   *same* cells so a program launched by the desktop still reads its start
//!   command (cflib `appinit.c:104` derives the application directory from it);
//! * [`shel_write_after_launch_wins_shel_read`] — the H05 order: start command
//!   set by the launcher, then the app's own `shel_write(SHW_EXEC)`, then
//!   `shel_read` — the written value wins, and TeraDesk's `strcmp` is zero.
//!
//! The tail is the Atari command-tail *image* (`tail[0]` = length, characters
//! follow — TeraDesk's `COMMAND`, `xfilesys.h:119-122`), round-tripped as a
//! NUL-terminated string (accepted deviation F8, `scrap-shell.md`); for a text
//! tail that is byte-identical to the 128-byte `memcpy`.

#![allow(dead_code)]

#[path = "aes_reference/harness.rs"]
mod harness;

use harness::{context, Pb, AES_LOCK};
use tos_api::aes::shell::{set_launch_info, SHW_EXEC};

const OP_SHEL_READ: i16 = 120;
const OP_SHEL_WRITE: i16 = 121;

/// The program TeraDesk was launched as (`#Z` autorun in the H05 measurement).
const OWN_NAME: &[u8] = b"C:\\APPS\\TERADESK\\DESKTOP.NAP\0";
/// The child TeraDesk hands to `shel_write(SHW_EXEC)` before its `Pexec`.
const CHILD: &[u8] = b"C:\\AUTO\\HELLO.NCP\0";
/// A command-tail image: length byte, then the characters (no NUL needed —
/// the length says where it ends; `sh_write` copies `CMDTAILSIZE` bytes).
const CHILD_TAIL: &[u8] = &[5, b'h', b'e', b'l', b'l', b'o'];
/// The tail image the desktop hands the launcher for a plain double-click.
const EMPTY_TAIL: &[u8] = &[];

/// A caller-owned string buffer for the two `shel_read` out-pointers, sized as
/// TeraDesk sizes them (`VLNAME cmd; char tail[sizeof(COMMAND)]`).
fn out_buf() -> Box<[u8; 256]> {
    Box::new([0xEE; 256])
}

/// The bytes up to (not including) the first NUL.
fn cstr(b: &[u8]) -> &[u8] {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    &b[..n]
}

/// `shel_write(SHW_EXEC, isgem, 0, cmd, tail)` — `int_in` = `doex`, `isgem`,
/// `isover`; `addr_in[0]` = command, `addr_in[1]` = tail
/// (`scrap-shell.md` slot table, opcode 121: 3/1/2/0).
fn shel_write_exec(pb: &mut Pb, isgem: i16, cmd: &[u8], tail: &[u8]) -> i16 {
    pb.poison();
    pb.intin[0] = SHW_EXEC;
    pb.intin[1] = isgem;
    pb.intin[2] = 0;
    pb.addrin[0] = cmd.as_ptr() as u64;
    pb.addrin[1] = tail.as_ptr() as u64;
    pb.call(OP_SHEL_WRITE, 3, 1, 2, 0);
    pb.intout[0]
}

/// `shel_read(cmd, tail)` — `addr_in[0]`/`[1]` are the two out buffers
/// (opcode 120: 0/1/2/0). Returns the AES result and the two buffers.
fn shel_read(pb: &mut Pb) -> (i16, Box<[u8; 256]>, Box<[u8; 256]>) {
    let mut cmd = out_buf();
    let mut tail = out_buf();
    pb.poison();
    pb.addrin[0] = cmd.as_mut_ptr() as u64;
    pb.addrin[1] = tail.as_mut_ptr() as u64;
    pb.call(OP_SHEL_READ, 0, 1, 2, 0);
    (pb.intout[0], cmd, tail)
}

/// What a well-formed tail image looks like after the NUL-terminated
/// round trip: the image bytes, then a NUL.
fn expect_tail(image: &[u8]) -> Vec<u8> {
    let mut v = image.to_vec();
    v.push(0);
    v
}

/// (a) `shel_write(SHW_EXEC, c, t)` then `shel_read(b1, b2)` yields `c` in `b1`
/// and `t` in `b2` — the same buffer both ways (`gemshlib.c:104,169`;
/// `scrap-shell.md` § test expectations).
#[test]
fn shel_write_exec_then_shel_read_returns_the_written_pair() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    assert_eq!(shel_write_exec(&mut pb, 0, CHILD, CHILD_TAIL), 1, "shel_write always returns 1");

    let (ret, cmd, tail) = shel_read(&mut pb);
    assert_eq!(ret, 1, "shel_read returns 1");
    assert_eq!(
        cstr(&cmd[..]),
        cstr(CHILD),
        "shel_read must return the command shel_write(SHW_EXEC) stored in D.s_cmd \
         (gemshlib.c:104 reads the cell :169 wrote); got {:?}",
        String::from_utf8_lossy(cstr(&cmd[..]))
    );
    assert_eq!(
        &tail[..CHILD_TAIL.len() + 1],
        &expect_tail(CHILD_TAIL)[..],
        "shel_read must return the tail image shel_write(SHW_EXEC) stored in ad_stail \
         (gemshlib.c:105 reads the cell :170 wrote)"
    );
}

/// (b) The launcher's side of `sh_ldapp`: after the kernel launcher has
/// published the start command and tail (`kernel/src/launch.rs` →
/// `set_launch_info`), the program reads them back — the contract cflib's
/// `appinit` relies on to find its own directory (`sdk/cflib/cflib-21/appinit.c:104`).
#[test]
fn launcher_fills_the_cells_shel_read_returns() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    set_launch_info(OWN_NAME, CHILD_TAIL);

    let (ret, cmd, tail) = shel_read(&mut pb);
    assert_eq!(ret, 1);
    assert_eq!(
        cstr(&cmd[..]),
        cstr(OWN_NAME),
        "a launched program must read its own start command (D.s_cmd as sh_ldapp ran it, \
         gemshlib.c:645); got {:?}",
        String::from_utf8_lossy(cstr(&cmd[..]))
    );
    assert_eq!(
        &tail[..CHILD_TAIL.len() + 1],
        &expect_tail(CHILD_TAIL)[..],
        "a launched program must read the tail image it was started with (ad_stail)"
    );

    // An empty image (plain double-click) reads back as the empty tail `[0]`.
    set_launch_info(OWN_NAME, EMPTY_TAIL);
    let (_, _, tail) = shel_read(&mut pb);
    assert_eq!(tail[0], 0, "an empty command tail reads back as length 0");
}

/// (c) H05 in one sequence: launcher sets the start command; the app itself
/// calls `shel_write(SHW_EXEC, child)`; `shel_read` now returns the child, not
/// the start command — so TeraDesk's chain test `strcmp(cmd, pinfo.name)`
/// (`startprg.c:346`) is zero and no second TeraDesk is started.
#[test]
fn shel_write_after_launch_wins_shel_read() {
    let _g = AES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    context();
    let mut pb = Pb::new();

    // The shell/launcher started TeraDesk: D.s_cmd = its own name.
    set_launch_info(OWN_NAME, EMPTY_TAIL);
    // TeraDesk, before its Pexec (startprg.c:260): shel_write(SHW_EXEC, 0, 0, name, &cml).
    assert_eq!(shel_write_exec(&mut pb, 0, CHILD, CHILD_TAIL), 1);
    // The child ran and Pterm'd; TeraDesk's chain check (startprg.c:344-346).
    let (ret, cmd, tail) = shel_read(&mut pb);
    assert_eq!(ret, 1);
    assert_eq!(
        cstr(&cmd[..]),
        cstr(CHILD),
        "after the app's own shel_write(SHW_EXEC), shel_read must return that command \
         (one buffer, gemshlib.c:104/:169) — returning the start command {:?} is the \
         H05 defect (TeraDesk chains itself); got {:?}",
        String::from_utf8_lossy(cstr(OWN_NAME)),
        String::from_utf8_lossy(cstr(&cmd[..]))
    );
    assert_ne!(
        cstr(&cmd[..]),
        cstr(OWN_NAME),
        "shel_read must not answer from a launcher-only buffer"
    );
    assert_eq!(&tail[..CHILD_TAIL.len() + 1], &expect_tail(CHILD_TAIL)[..]);
}
