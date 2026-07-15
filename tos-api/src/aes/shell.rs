//! AES Scrap Manager (`scrp_*` 80–82) + Shell Manager (`shel_*` 120–127).
//!
//! Spec: [`docs/api/aes/scrap-shell.md`]. The **scrap** manager owns the
//! clipboard *directory path* (the data itself is exchanged as `SCRAP.*`
//! files through GEMDOS). The **shell** manager is the bridge between the
//! desktop/shell and launched programs: which program runs next
//! (`shel_write`), the command that started this app (`shel_read`), a
//! shared settings buffer (`shel_get`/`shel_put`), path resolution
//! (`shel_find`) and the environment (`shel_envrn`).
//!
//! `shel_write(doex=1)` records the next image to run; the actual `Pexec`
//! is performed by the shell loop once the desktop drives it (the desktop
//! is not built yet, so the request is **recorded** — see
//! [`pending_launch`] — not a stub). `shel_write(doex=0)` records "return
//! to the desktop", which an app issues before `appl_exit`/`Pterm`.

use super::Aespb;
use core::cell::UnsafeCell;

/// Built-in environment used when the shell buffer is empty.
const DEFAULT_ENV: &[u8] = b"PATH=C:\\;\0\0";

const PATHBUF: usize = 128;
/// The shell settings buffer capacity — `SIZE_SHELBUF` (`sysconf.h:74`,
/// "same as TOS 1.04→"). `shel_get`/`shel_put` clamp `len` to this: a
/// safety bound over EmuTOS's unbounded `memcpy` (`gemshlib.c:205,215`,
/// spec edge-case 10) that does not change well-formed behaviour.
const SHELLBUF: usize = 4192;

/// `shel_write` mode flags (`doex`, `aesdefs.h:618-621`).
const SHW_NOEXEC: i16 = 0; // exit to desktop
const SHW_EXEC: i16 = 1; // run pcmd next after this app terminates
const SHW_SHUTDOWN: i16 = 4; // shut the system down (honest-unsupported)
const SHW_RESCHNG: i16 = 5; // change screen resolution (honest-unsupported)

struct State {
    scrap: UnsafeCell<[u8; PATHBUF]>,       // clipboard dir (empty = never set)
    next_cmd: UnsafeCell<[u8; PATHBUF]>,    // shel_write program
    next_tail: UnsafeCell<[u8; PATHBUF]>,   // shel_write command tail
    next_doex: UnsafeCell<i16>,             // shel_write mode: 1 run / 0 desktop / -1 none
    next_isgem: UnsafeCell<i16>,            // shel_write isgem: 1 GEM/PRG / 0 char/TTP
    launch_cmd: UnsafeCell<[u8; PATHBUF]>,  // command that started this app
    launch_tail: UnsafeCell<[u8; PATHBUF]>, // its tail
    buf: UnsafeCell<[u8; SHELLBUF]>,        // shel_get/shel_put settings blob
    env: UnsafeCell<[u8; PATHBUF]>,         // environment (empty = default)
    sh_desk: UnsafeCell<[u8; PATHBUF]>,     // shel_rdef/wdef default startup app
    sh_cdir: UnsafeCell<[u8; PATHBUF]>,     // shel_rdef/wdef default startup dir
}
// SAFETY: single boot CPU only touches the AES.
unsafe impl Sync for State {}

static S: State = State {
    scrap: UnsafeCell::new([0; PATHBUF]),
    next_cmd: UnsafeCell::new([0; PATHBUF]),
    next_tail: UnsafeCell::new([0; PATHBUF]),
    next_doex: UnsafeCell::new(-1),
    next_isgem: UnsafeCell::new(0),
    launch_cmd: UnsafeCell::new([0; PATHBUF]),
    launch_tail: UnsafeCell::new([0; PATHBUF]),
    buf: UnsafeCell::new([0; SHELLBUF]),
    env: UnsafeCell::new([0; PATHBUF]),
    sh_desk: UnsafeCell::new([0; PATHBUF]),
    sh_cdir: UnsafeCell::new([0; PATHBUF]),
};

// ── small string helpers ─────────────────────────────────────────────────

fn cstr_in(dst: &mut [u8], src: *const u8) {
    for b in dst.iter_mut() {
        *b = 0;
    }
    if src.is_null() {
        return;
    }
    // SAFETY: trusted GEM pointer to a NUL-terminated string.
    unsafe {
        let mut i = 0;
        while i < dst.len() - 1 {
            let c = *src.add(i);
            dst[i] = c;
            if c == 0 {
                break;
            }
            i += 1;
        }
    }
}

fn cstr_out(dst: *mut u8, src: &[u8]) {
    if dst.is_null() {
        return;
    }
    let n = src.iter().position(|&b| b == 0).unwrap_or(src.len());
    // SAFETY: caller buffer is large enough for the string (GEM contract).
    unsafe {
        for i in 0..n {
            *dst.add(i) = src[i];
        }
        *dst.add(n) = 0;
    }
}

fn clen(s: &[u8]) -> usize {
    s.iter().position(|&b| b == 0).unwrap_or(s.len())
}

// ── Scrap Manager ─────────────────────────────────────────────────────────

/// `scrp_read` (80) — copy the clipboard directory path into `addr_in[0]`.
///
/// A bare `strcpy(pscrap, D.g_scrap)` (`gemsclib.c:42`): if no path was ever
/// `scrp_write`n, `D.g_scrap` is empty and an **empty string** is returned —
/// **not** a default like `C:\CLIPBRD\` (spec edge-case 1, `gemsclib.c:35-37`).
/// Always returns `1` (`gemsclib.c:44`).
pub fn scrp_read(pb: &Aespb) {
    let dst = pb.addrin(0) as *mut u8;
    // SAFETY: single-CPU AES.
    let scrap = unsafe { &*S.scrap.get() };
    cstr_out(dst, scrap);
    pb.set_ret(1);
}

/// `scrp_write` (81) — set the clipboard directory path from `addr_in[0]`.
pub fn scrp_write(pb: &Aespb) {
    let src = pb.addrin(0) as *const u8;
    // SAFETY: single-CPU AES.
    cstr_in(unsafe { &mut *S.scrap.get() }, src);
    pb.set_ret(1);
}

/// `scrp_clear` (82) — PC-GEM extension to delete the `SCRAP.*` files. Not
/// implemented yet (needs directory-enumerated deletion); honestly reports
/// 0 rather than faking success.
pub fn scrp_clear(pb: &Aespb) {
    pb.set_ret(0);
}

// ── Shell Manager ─────────────────────────────────────────────────────────

/// `shel_read` (120) — the command + tail that started this app →
/// `addr_in[0]`/`addr_in[1]` (`sh_read`, `gemshlib.c:102-106`).
///
/// **Deferred producer (F7).** `launch_cmd`/`launch_tail` are the values the
/// **loader** must populate when a program is launched through the shell loop
/// (EmuTOS fills `D.s_cmd`/`ad_stail` in `sh_ldapp` before `dos_exec`,
/// `gemshlib.c:645`). NewTOS does not yet wire the shell-loop↔`Pexec` launch
/// path, so no producer writes them and `shel_read` returns the defined-empty
/// pair. This is an honest gap, not a stub: the read side is correct and the
/// missing side is the launch wiring (tracked in `docs/STATUS.md` "Blocked").
///
/// **Accepted LOW deviation (F8).** EmuTOS copies a fixed `CMDTAILSIZE = 128`
/// bytes (`memcpy`, `gemshlib.c:105`); we round-trip the tail as a
/// NUL-terminated C string. This is byte-exact for conforming text tails and
/// only truncates a tail carrying an embedded NUL (a rare binary tail). Kept
/// deliberately to avoid an unconditional 128-byte over-read of an EL0 user
/// pointer for a low-value case (spec edge-case 6).
pub fn shel_read(pb: &Aespb) {
    // SAFETY: single-CPU AES.
    unsafe {
        cstr_out(pb.addrin(0) as *mut u8, &*S.launch_cmd.get());
        cstr_out(pb.addrin(1) as *mut u8, &*S.launch_tail.get());
    }
    pb.set_ret(1);
}

/// `shel_write` (121) — the multi-purpose shell selector (`sh_write`,
/// `gemshlib.c:157-196`). `int_in[0]` = `doex` (mode, `SHW_*`), `int_in[1]` =
/// `isgem` (GEM/graphic vs char/TTP), `int_in[2]` = `isover` (shutdown
/// level / rez family); `addr_in[0]`/`[1]` = command + tail. **Always
/// returns `1`** (`gemshlib.c:195`); the actual `Pexec` is deferred to the
/// shell loop. Dispatches on the mode rather than blindly recording a launch:
///
/// * `SHW_EXEC` (1) — record `pcmd`/`ptail` and `isgem` as the next program
///   (`gemshlib.c:168-176`). `isgem` routes the future launch: `1` = GEM
///   (PRG/APP → GUI), `0` = char (TTP → console).
/// * `SHW_NOEXEC` (0) — exit to the desktop, forced GEM-mode; no program
///   name (`gemshlib.c:162-167`).
/// * `SHW_SHUTDOWN` (4) / `SHW_RESCHNG` (5) — **not a program launch**: clear
///   `s_cmd`/tail (`gemshlib.c:190-191`) and record no pending launch. Both
///   stay honest-unsupported on RPi4/AArch64 (no dual-video / power-off,
///   D-101); this only avoids mis-recording them as a launch.
/// * any other `doex` — silently ignored, still returns `1` (spec edge-case
///   9; EmuTOS's `switch` has no `default`).
pub fn shel_write(pb: &Aespb) {
    let doex = pb.intin(0);
    let isgem = pb.intin(1);
    // SAFETY: single-CPU AES.
    unsafe {
        match doex {
            SHW_EXEC => {
                cstr_in(&mut *S.next_cmd.get(), pb.addrin(0) as *const u8);
                cstr_in(&mut *S.next_tail.get(), pb.addrin(1) as *const u8);
                *S.next_isgem.get() = i16::from(isgem != 0);
                *S.next_doex.get() = SHW_EXEC;
            }
            SHW_NOEXEC => {
                // Exit to the desktop: no program, forced GEM (gemshlib.c:166).
                (*S.next_cmd.get()).fill(0);
                (*S.next_tail.get()).fill(0);
                *S.next_isgem.get() = 1;
                *S.next_doex.get() = SHW_NOEXEC;
            }
            SHW_SHUTDOWN | SHW_RESCHNG => {
                // Not a launch: clear the command/tail and record nothing
                // pending (gemshlib.c:190-191). No rez-change/power-off on
                // RPi4 (D-101) — this only stops a mis-recorded launch.
                (*S.next_cmd.get()).fill(0);
                (*S.next_tail.get()).fill(0);
                *S.next_doex.get() = -1;
            }
            _ => {}
        }
    }
    pb.set_ret(1);
}

/// `shel_get` (122) — copy `int_in[0]` bytes of the shell settings buffer
/// into `addr_in[0]`.
pub fn shel_get(pb: &Aespb) {
    let dst = pb.addrin(0) as *mut u8;
    let len = (pb.intin(0) as usize).min(SHELLBUF);
    if !dst.is_null() {
        // SAFETY: single-CPU AES; caller buffer is >= len bytes.
        unsafe {
            let buf = &*S.buf.get();
            for i in 0..len {
                *dst.add(i) = buf[i];
            }
        }
    }
    pb.set_ret(1);
}

/// `shel_put` (123) — write `int_in[0]` bytes from `addr_in[0]` into the
/// shell settings buffer.
pub fn shel_put(pb: &Aespb) {
    let src = pb.addrin(0) as *const u8;
    let len = (pb.intin(0) as usize).min(SHELLBUF);
    if !src.is_null() {
        // SAFETY: single-CPU AES; caller buffer is >= len bytes.
        unsafe {
            let buf = &mut *S.buf.get();
            for i in 0..len {
                buf[i] = *src.add(i);
            }
        }
    }
    pb.set_ret(1);
}

/// `shel_find` (124) — resolve the program name in `addr_in[0]` to an
/// existing file, rewriting the buffer in place. Returns `1` if found, else
/// `0` (`sh_find`/`findfile`, `gemshlib.c:406-484`).
///
/// EmuTOS's `findfile` is a four-step search: (1) the **application
/// directory** (`rlr->p_appdir`), (2) the current directory (spec as given),
/// (3) the **root of the current drive** (`\` prefix), (4) each path in the
/// AES `PATH=` environment. NewTOS implements the two search stages it can
/// today:
///
/// * step (2) — the path as given (covers the current directory when the
///   spec is drive/dir-relative), and
/// * step (3) — the current drive's root, `<X>:\<name>` where `X` is the
///   current drive letter (`Dgetdrv()`), replacing the previous hardcoded
///   `C:\` which ignored the current drive (`gemshlib.c:437-445`).
///
/// **Deferred (F5).** Step (1) needs the process model's `p_appdir` and step
/// (4) needs a real DOS environment; NewTOS exposes neither yet, so those two
/// stages are omitted rather than faked (tracked in `docs/STATUS.md`
/// "Blocked"). **Note:** using `read_file` as an existence probe is fragile
/// versus a proper `Fsfirst` existence check — it fails a directory match and
/// does not honour wildcards, both of which EmuTOS's `dos_sfirst` tolerates;
/// a switch to `Fsfirst` is the right long-term fix.
pub fn shel_find(pb: &Aespb) {
    let p = pb.addrin(0) as *mut u8;
    if p.is_null() {
        pb.set_ret(0);
        return;
    }
    let mut name = [0u8; PATHBUF];
    cstr_in(&mut name, p);
    let n = clen(&name);

    let mut probe = [0u8; 1];
    // (2) The path as given (current directory when drive/dir-relative).
    if let Ok(s) = core::str::from_utf8(&name[..n]) {
        if crate::bdos::fs::read_file(s, &mut probe).is_some() {
            pb.set_ret(1);
            return;
        }
    }
    // (3) Root of the current drive: "<X>:\<name>" (only for a bare name
    //     with no drive/path separator). X = the current drive letter.
    if !name[..n].contains(&b'\\') && !name[..n].contains(&b':') {
        let drive = crate::bdos::fs::drive::current_drive() as u8;
        let mut full = [0u8; PATHBUF];
        full[0] = b'A' + drive;
        full[1] = b':';
        full[2] = b'\\';
        let m = n.min(PATHBUF - 4);
        full[3..3 + m].copy_from_slice(&name[..m]);
        if let Ok(s) = core::str::from_utf8(&full[..3 + m]) {
            if crate::bdos::fs::read_file(s, &mut probe).is_some() {
                cstr_out(p, &full);
                pb.set_ret(1);
                return;
            }
        }
    }
    pb.set_ret(0);
}

/// `shel_envrn` (125) — find the environment variable named by `addr_in[1]`
/// (e.g. `"PATH="`) and store a pointer to its value through the
/// `char **` at `addr_in[0]`.
pub fn shel_envrn(pb: &Aespb) {
    let pvalue = pb.addrin(0) as *mut u64; // char **
    let search = pb.addrin(1) as *const u8;
    // `sh_envrn` is `void`: `int_out[0]` stays `1` in every path (spec
    // edge-case 13, `gemshlib.c:296`). A null output pointer cannot receive
    // the result, but the call still reports success.
    if pvalue.is_null() {
        pb.set_ret(1);
        return;
    }
    let mut needle = [0u8; 64];
    cstr_in(&mut needle, search);
    let nlen = clen(&needle);

    // SAFETY: single-CPU AES; env is a stable static blob.
    let env = unsafe { &mut *S.env.get() };
    if env[0] == 0 {
        env[..DEFAULT_ENV.len()].copy_from_slice(DEFAULT_ENV);
    }
    // The environment is a sequence of NUL-terminated "NAME=VALUE" strings.
    let mut i = 0;
    while i < env.len() && env[i] != 0 {
        let start = i;
        while i < env.len() && env[i] != 0 {
            i += 1;
        }
        let entry = &env[start..i];
        if nlen > 0 && entry.len() >= nlen && &entry[..nlen] == &needle[..nlen] {
            // Point at the value (just past "NAME=").
            // SAFETY: env outlives the call; the pointer is into it.
            unsafe { *pvalue = env.as_ptr().add(start + nlen) as u64 };
            pb.set_ret(1);
            return;
        }
        i += 1; // skip the NUL
    }
    // Not found → NULL (`*ppath = NULL`, `gemshlib.c:333,347`), still `1`.
    // SAFETY: as above.
    unsafe { *pvalue = 0 };
    pb.set_ret(1);
}

/// `shel_rdef` (126) — PC-GEM: read the shell's default startup command
/// (`addr_in[0]`) + directory (`addr_in[1]`) from the `SHELL` slots
/// `sh_desk`/`sh_cdir` (`sh_rdef`, `gemshlib.c:491-497`). These have **their
/// own storage**: they are not the pending `shel_write` launch nor the scrap
/// directory.
pub fn shel_rdef(pb: &Aespb) {
    // SAFETY: single-CPU AES.
    unsafe {
        cstr_out(pb.addrin(0) as *mut u8, &*S.sh_desk.get());
        cstr_out(pb.addrin(1) as *mut u8, &*S.sh_cdir.get());
    }
    pb.set_ret(1);
}

/// `shel_wdef` (127) — PC-GEM: set the shell's default startup command
/// (`addr_in[0]` → `sh_desk`) + directory (`addr_in[1]` → `sh_cdir`)
/// (`sh_wdef`, `gemshlib.c:504-510`). Writes only its own `SHELL` slots — it
/// must **not** touch the pending `shel_write` launch (`next_cmd`), or a
/// `shel_wdef` after a `shel_write(SHW_EXEC, …)` would corrupt the program the
/// shell loop is about to run.
pub fn shel_wdef(pb: &Aespb) {
    // SAFETY: single-CPU AES.
    unsafe {
        cstr_in(&mut *S.sh_desk.get(), pb.addrin(0) as *const u8);
        cstr_in(&mut *S.sh_cdir.get(), pb.addrin(1) as *const u8);
    }
    pb.set_ret(1);
}

/// A launch recorded by `shel_write`, handed to the shell loop (the desktop)
/// to `Pexec` when it regains control.
pub struct PendingLaunch {
    /// The mode: `SHW_EXEC` (1) run [`cmd`](Self::cmd), or `SHW_NOEXEC` (0)
    /// return to the desktop.
    pub doex: i16,
    /// GEM-vs-char routing: `1` = GEM/graphic (PRG/APP → GUI), `0` =
    /// TOS/char (TTP → console). Recorded from `int_in[1]` on `SHW_EXEC`.
    pub isgem: i16,
    /// The program name to run (empty on `SHW_NOEXEC`).
    pub cmd: &'static [u8],
    /// The command tail.
    pub tail: &'static [u8],
}

/// The launch recorded by the most recent `shel_write`, for the shell loop
/// (the desktop) to `Pexec` when it regains control. `None` if nothing is
/// pending (`next_doex < 0`, e.g. after a `SHW_SHUTDOWN`/`SHW_RESCHNG`).
pub fn pending_launch() -> Option<PendingLaunch> {
    // SAFETY: single-CPU AES; the buffers are stable statics.
    unsafe {
        let doex = *S.next_doex.get();
        if doex < 0 {
            None
        } else {
            Some(PendingLaunch {
                doex,
                isgem: *S.next_isgem.get(),
                cmd: &*S.next_cmd.get(),
                tail: &*S.next_tail.get(),
            })
        }
    }
}
