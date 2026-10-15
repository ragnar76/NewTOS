//! `desktop::selftest` — the runner and the output channel of the desktop's four
//! built-in self-tests (DESK45; DESK43 U8a).
//!
//! The four checks live beside the code they exercise —
//! [`super::ins::selftest`] (Install devices / Remove icon on the model),
//! [`super::dir::selftest`] (the file-operation backend on `C:\DESKT9\`),
//! [`super::inf::selftest`] (`NEWDESK.INF` save→read round trip) and
//! [`super::inf::selftest_install`] (Install-application association) —
//! plus the `act_chg` `DISABLED`-guard check [`super::act::selftest`]
//! (DESK59, state only, nothing on `C:`) — and they are always compiled, so
//! a signature break fails the build. They are **run** only in a build with the kernel feature `selftest-desk`:
//! [`super::deskmain`] calls [`run_once`] once, after its prolog, where the
//! desktop state, the cell metrics and the `NEWDESK.INF` model exist (DESK43
//! U7 open point 4: before `deskmain` every metric is 0). Three of the four
//! write to `C:` — build the feature only for a test card.
//!
//! **Output.** The product desktop prints nothing (EmuTOS leaves `KDEBUG`
//! compiled out of every `desk/*.c`, `deskact.c:19`, `deskapp.c:20`; DESK43
//! decision D4), so these lines are the only text the desktop ever writes, and
//! they go where a test program's go: to the GEMDOS console, through the trap
//! ([`newtos_gem::gemdos::cconout`]). The console is the serial line and the
//! framebuffer; on the screen the lines land over the desktop, which a
//! self-test build accepts. There is no kernel path in here — no `crate::`,
//! no `kprintln` — so the runner moves with the desktop when it becomes a
//! crate (U8b) and becomes the separate EL0 test program DESK45 B asks for
//! without a rewrite.

use core::fmt::{self, Write};

use newtos_gem::gemdos::cconout;

/// The console as a [`fmt::Write`] sink: every byte through `Cconout`, `\n`
/// expanded to `\r\n` (the console is a VT52 terminal).
struct ConOut;

impl Write for ConOut {
    // NEWTOS: self-test output; EmuTOS's desktop has no self-tests.
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                cconout(b'\r');
            }
            cconout(b);
        }
        Ok(())
    }
}

/// Print one line (`args` + CR LF) on the console. The body of [`tprintln`].
// NEWTOS: self-test output; EmuTOS's desktop has no self-tests.
pub(super) fn line(args: fmt::Arguments) {
    let _ = ConOut.write_fmt(args);
    cconout(b'\r');
    cconout(b'\n');
}

/// `println!` for the self-tests — formats into [`line`].
macro_rules! tprintln {
    ($($arg:tt)*) => {
        super::selftest::line(format_args!($($arg)*))
    };
}
pub(super) use tprintln;

/// Run the four self-tests, the first time a `deskmain` reaches its event loop
/// in this boot (a desktop that restarts after a program, or after *Read .INF
/// file*, does not repeat them: they rewrite `C:\NEWDESK.INF`).
///
/// Order: the `act_chg` guard check first (it changes one icon's
/// `ob_flags`/`ob_state` and puts them back), then the model test (it needs
/// the untouched `G.g_ahead` it restores afterwards), then the file backend, then the two `NEWDESK.INF` tests (they
/// replace the session model). The last line, `[desk-selftest] done`, marks
/// the end for a harness.
// NEWTOS: self-test runner (DESK45); EmuTOS's desktop has no self-tests.
#[cfg(feature = "selftest-desk")]
pub(super) fn run_once() {
    use core::sync::atomic::{AtomicBool, Ordering};
    static RAN: AtomicBool = AtomicBool::new(false);
    if RAN.swap(true, Ordering::AcqRel) {
        return;
    }
    tprintln!("[desk-selftest] begin (selftest-desk build; mutates C:)");
    super::act::selftest();
    super::ins::selftest();
    super::dir::selftest();
    super::inf::selftest();
    super::inf::selftest_install();
    tprintln!("[desk-selftest] done");
}
