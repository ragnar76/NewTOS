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

/// Default clipboard directory when none has been set.
const SCRAP_DEFAULT: &[u8] = b"C:\\CLIPBRD\\";
/// Built-in environment used when the shell buffer is empty.
const DEFAULT_ENV: &[u8] = b"PATH=C:\\;\0\0";

const PATHBUF: usize = 128;
const SHELLBUF: usize = 1024;

struct State {
    scrap: UnsafeCell<[u8; PATHBUF]>,       // clipboard dir (empty = default)
    next_cmd: UnsafeCell<[u8; PATHBUF]>,    // shel_write program
    next_tail: UnsafeCell<[u8; PATHBUF]>,   // shel_write command tail
    next_doex: UnsafeCell<i16>,             // shel_write: 1 run / 0 desktop
    launch_cmd: UnsafeCell<[u8; PATHBUF]>,  // command that started this app
    launch_tail: UnsafeCell<[u8; PATHBUF]>, // its tail
    buf: UnsafeCell<[u8; SHELLBUF]>,        // shel_get/shel_put settings blob
    env: UnsafeCell<[u8; PATHBUF]>,         // environment (empty = default)
}
// SAFETY: single boot CPU only touches the AES.
unsafe impl Sync for State {}

static S: State = State {
    scrap: UnsafeCell::new([0; PATHBUF]),
    next_cmd: UnsafeCell::new([0; PATHBUF]),
    next_tail: UnsafeCell::new([0; PATHBUF]),
    next_doex: UnsafeCell::new(-1),
    launch_cmd: UnsafeCell::new([0; PATHBUF]),
    launch_tail: UnsafeCell::new([0; PATHBUF]),
    buf: UnsafeCell::new([0; SHELLBUF]),
    env: UnsafeCell::new([0; PATHBUF]),
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
pub fn scrp_read(pb: &Aespb) {
    let dst = pb.addrin(0) as *mut u8;
    // SAFETY: single-CPU AES.
    let scrap = unsafe { &*S.scrap.get() };
    if scrap[0] == 0 {
        cstr_out(dst, SCRAP_DEFAULT);
    } else {
        cstr_out(dst, scrap);
    }
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
/// `addr_in[0]`/`addr_in[1]`.
pub fn shel_read(pb: &Aespb) {
    // SAFETY: single-CPU AES.
    unsafe {
        cstr_out(pb.addrin(0) as *mut u8, &*S.launch_cmd.get());
        cstr_out(pb.addrin(1) as *mut u8, &*S.launch_tail.get());
    }
    pb.set_ret(1);
}

/// `shel_write` (121) — record what the shell should run next. `int_in[0]`
/// = doex (1 launch / 0 desktop), `int_in[1]` = isgr (GEM/TOS), `int_in[2]`
/// = iscr (mode); `addr_in[0]`/`[1]` = command + tail. The shell loop
/// performs the `Pexec` once the desktop drives it.
pub fn shel_write(pb: &Aespb) {
    let doex = pb.intin(0);
    // SAFETY: single-CPU AES.
    unsafe {
        *S.next_doex.get() = doex;
        cstr_in(&mut *S.next_cmd.get(), pb.addrin(0) as *const u8);
        cstr_in(&mut *S.next_tail.get(), pb.addrin(1) as *const u8);
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
/// existing file, rewriting the buffer in place. Tries the path as given,
/// then `C:\<name>`. Returns 1 if found, else 0.
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
    // 1) As given.
    if let Ok(s) = core::str::from_utf8(&name[..n]) {
        if crate::bdos::fs::read_file(s, &mut probe).is_some() {
            pb.set_ret(1);
            return;
        }
    }
    // 2) C:\<name> (only if it had no drive/path separator).
    if !name[..n].contains(&b'\\') && !name[..n].contains(&b':') {
        let mut full = [0u8; PATHBUF];
        full[..3].copy_from_slice(b"C:\\");
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
    if pvalue.is_null() {
        pb.set_ret(0);
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
    // Not found → empty string.
    // SAFETY: as above.
    unsafe { *pvalue = env.as_ptr().add(env.len() - 1) as u64 };
    pb.set_ret(0);
}

/// `shel_rdef` (126) — PC-GEM: read the shell's default command + dir into
/// `addr_in[0]`/`[1]`. We carry no separate default; report the recorded
/// next command (empty if none).
pub fn shel_rdef(pb: &Aespb) {
    // SAFETY: single-CPU AES.
    unsafe {
        cstr_out(pb.addrin(0) as *mut u8, &*S.next_cmd.get());
        cstr_out(pb.addrin(1) as *mut u8, &*S.scrap.get());
    }
    pb.set_ret(1);
}

/// `shel_wdef` (127) — PC-GEM: set the shell's default command + dir.
pub fn shel_wdef(pb: &Aespb) {
    // SAFETY: single-CPU AES.
    unsafe {
        cstr_in(&mut *S.next_cmd.get(), pb.addrin(0) as *const u8);
    }
    pb.set_ret(1);
}

/// The launch recorded by the most recent `shel_write`, for the shell loop
/// (the desktop) to `Pexec` when it regains control. `None` if nothing is
/// pending; `Some((doex, &cmd, &tail))` otherwise (`doex` 1 = run cmd,
/// 0 = return to the desktop).
pub fn pending_launch() -> Option<(i16, &'static [u8], &'static [u8])> {
    // SAFETY: single-CPU AES; the buffers are stable statics.
    unsafe {
        let doex = *S.next_doex.get();
        if doex < 0 {
            None
        } else {
            Some((doex, &*S.next_cmd.get(), &*S.next_tail.get()))
        }
    }
}
