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

/// FAT `FA_SUBDIR` attribute bit (a directory, not a deletable file).
const FA_SUBDIR: u8 = 0x10;
/// GEMDOS `Fdelete` function id and the GEMDOS trap number, for building the
/// `TrapArgs` `scrp_clear` hands to [`crate::bdos::fs::file::fdelete`].
const FDELETE: u16 = 0x41;
const TRAP_GEMDOS: u16 = 1;

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

/// `scrp_clear` (82) — PC-GEM: delete every `SCRAP.*` file in the current
/// scrap directory (`sc_clear`, `gemsclib.c:74-107`).
///
/// EmuTOS appends the mask `SCRAP.*` to `D.g_scrap` (which is assumed to end
/// in a path separator), then runs a `dos_sfirst(FA_SUBDIR)`/`dos_snext` loop
/// calling `dos_delete` on each match, and returns `TRUE`; it returns `FALSE`
/// only when no scrap directory has been set (`D.g_scrap[0] == '\0'`,
/// `gemsclib.c:81`). We reproduce the observable contract: with a scrap path
/// set (from a prior [`scrp_write`]), enumerate `<scrapdir>SCRAP.*` and delete
/// each file, returning `1`; with no scrap path set, return `0`.
///
/// Rather than delete during a live `Fsnext` walk (iterator invalidation), we
/// snapshot the listing with [`list_dir_ex`](crate::bdos::fs::find::list_dir_ex)
/// and delete each entry, re-listing until no further deletable file remains —
/// observably equivalent (all deletable `SCRAP.*` gone) and drains directories
/// larger than one snapshot buffer while still terminating on undeletable
/// entries (a sub-directory matching the mask, which `Fdelete` refuses).
pub fn scrp_clear(pb: &Aespb) {
    use crate::bdos::fs::file::fdelete;
    use crate::bdos::fs::find::{list_dir_ex, DirInfo};
    use crate::trap::TrapArgs;

    // SAFETY: single-CPU AES; the scrap path is a stable static buffer.
    let scrap = unsafe { &*S.scrap.get() };
    let slen = clen(scrap);
    // No scrap directory ever set → FALSE (gemsclib.c:81-82).
    if slen == 0 {
        pb.set_ret(0);
        return;
    }

    // Build "<scrapdir>SCRAP.*" (gemsclib.c:88 appends the mask to D.g_scrap).
    const MASK: &[u8] = b"SCRAP.*";
    let mut maskbuf = [0u8; PATHBUF + 8];
    if slen + MASK.len() >= maskbuf.len() {
        // A pathological over-long scrap path; nothing to enumerate against.
        pb.set_ret(0);
        return;
    }
    maskbuf[..slen].copy_from_slice(&scrap[..slen]);
    maskbuf[slen..slen + MASK.len()].copy_from_slice(MASK);
    let mask = match core::str::from_utf8(&maskbuf[..slen + MASK.len()]) {
        Ok(s) => s,
        Err(_) => {
            pb.set_ret(0);
            return;
        }
    };

    // Delete every SCRAP.* file (gemsclib.c:93-100). Full path = <scrapdir> +
    // d_fname (gemsclib.c:96), which fdelete reads as a kernel pointer.
    let mut ents = [DirInfo::default(); 32];
    loop {
        let n = list_dir_ex(mask, &mut ents);
        if n == 0 {
            break;
        }
        let mut deleted_any = false;
        for e in &ents[..n] {
            if e.attrib & FA_SUBDIR != 0 {
                continue; // Fdelete refuses a sub-directory (EACCDN)
            }
            let nlen = e.name.iter().position(|&b| b == 0).unwrap_or(e.name.len());
            let mut full = [0u8; PATHBUF + 16];
            if slen + nlen + 1 > full.len() {
                continue;
            }
            full[..slen].copy_from_slice(&scrap[..slen]);
            full[slen..slen + nlen].copy_from_slice(&e.name[..nlen]);
            // `full[slen + nlen]` stays 0 → NUL-terminated for fdelete.
            let args = TrapArgs {
                regs: [full.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0],
                func: FDELETE as u64,
                svc_imm: TRAP_GEMDOS,
            };
            if fdelete(&args) == tos_types::error::E_OK {
                deleted_any = true;
            }
        }
        // Nothing deletable left (only skipped sub-directories) → stop.
        if !deleted_any {
            break;
        }
    }
    // Success — a scrap directory was set and its SCRAP.* files were cleared
    // (gemsclib.c:106, return TRUE).
    pb.set_ret(1);
}

// ── Shell Manager ─────────────────────────────────────────────────────────

/// `shel_read` (120) — the command + tail that started this app →
/// `addr_in[0]`/`addr_in[1]` (`sh_read`, `gemshlib.c:102-106`).
///
/// `launch_cmd`/`launch_tail` are populated by [`set_launch_info`], which the
/// kernel launcher calls just before the program enters EL0 — the NewTOS
/// analogue of EmuTOS filling `D.s_cmd`/`ad_stail` in `sh_ldapp` before
/// `dos_exec` (`gemshlib.c:604-605,645`). A program launched through the
/// desktop therefore reads back its own start command and command tail.
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

/// Record the command + command tail that started the current application so a
/// later [`shel_read`] returns them (the producer side of `sh_read`,
/// `gemshlib.c:102-106`). The kernel launcher calls this just before the
/// program enters EL0, mirroring EmuTOS setting `D.s_cmd` (the program path)
/// and `ad_stail` (the command tail) before `dos_exec` (`gemshlib.c:604-605`,
/// via `sh_write`, and `:645`).
///
/// `cmd` is the full launch path (`C:\APPS\QED\QED.NAP`); `tail` is the command
/// tail **characters** (without the leading length byte — the caller strips it,
/// matching the NUL-terminated round-trip `shel_read`/`shel_write` use, F8).
/// Both are stored as NUL-terminated strings, truncated to the buffer if longer.
pub fn set_launch_info(cmd: &[u8], tail: &[u8]) {
    // SAFETY: single-CPU launcher context; exclusive writer of these buffers.
    unsafe {
        store_str(&mut *S.launch_cmd.get(), cmd);
        store_str(&mut *S.launch_tail.get(), tail);
    }
}

/// Copy `src` into `dst` as a zero-padded, NUL-terminated string, stopping at
/// an embedded NUL in `src` and capping to `dst.len() - 1` bytes.
fn store_str(dst: &mut [u8], src: &[u8]) {
    dst.fill(0);
    let n = src
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(src.len())
        .min(dst.len() - 1);
    dst[..n].copy_from_slice(&src[..n]);
    // `dst` was zero-filled, so byte `n` is the terminating NUL.
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
/// EmuTOS's `findfile` is a four-step search, in precedence order:
///
/// * (1) the **application directory** (`rlr->p_appdir`) — the *filename
///   portion* of the spec (via `sh_name`) is searched there first
///   (`gemshlib.c:415-423`); on a hit the buffer is rewritten to the
///   fully-qualified name. NewTOS uses [`crate::aes::resource::app_dir`] (set
///   by the kernel launcher, the `rlr->p_appdir` analogue).
/// * (2) the current directory — the path as given (`gemshlib.c:426-431`).
/// * (3) the **root of the current drive** — `<X>:\<name>` where `X` is the
///   current drive letter (`Dgetdrv()`), replacing the previous hardcoded
///   `C:\` which ignored the current drive (`gemshlib.c:434-442`).
/// * (4) each path in the AES `PATH=` environment (`gemshlib.c:445-460`).
///
/// **Step (4) skipped (documented, not faked).** NewTOS has no
/// loader-populated AES `PATH=` environment: `shel_envrn` reads an internal
/// hardcoded default (`"PATH=C:\;"`), not a real per-process AES environment
/// (audit P6-010). Iterating that stand-in would only add a redundant `C:\`
/// search (already covered by step (3)) while building on the faked env, so
/// step (4) is omitted until a real AES environment exists — rather than
/// inventing a PATH mechanism. Steps (1)-(3) are implemented in full.
///
/// **Note:** using `read_file` as the existence probe (kept from steps 2-3) is
/// fragile versus a proper `Fsfirst` check — it fails a directory match and
/// does not honour wildcards, both of which EmuTOS's `dos_sfirst` tolerates;
/// a switch to `Fsfirst` is the right long-term fix (out of scope here).
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

    // (1) The application directory (rlr->p_appdir), searched first with the
    //     filename portion only (sh_name, gemshlib.c:412-423).
    let mut appdir = [0u8; 64];
    let alen = crate::aes::resource::app_dir(&mut appdir);
    if alen > 0 {
        let fname = filename_portion(&name[..n]);
        let mut full = [0u8; PATHBUF];
        if alen + fname.len() < full.len() {
            full[..alen].copy_from_slice(&appdir[..alen]);
            full[alen..alen + fname.len()].copy_from_slice(fname);
            if let Ok(s) = core::str::from_utf8(&full[..alen + fname.len()]) {
                if crate::bdos::fs::read_file(s, &mut probe).is_some() {
                    cstr_out(p, &full);
                    pb.set_ret(1);
                    return;
                }
            }
        }
    }

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
    // (4) AES PATH= — skipped: no loader-populated AES environment (see doc).
    pb.set_ret(0);
}

/// The filename portion of a path (EmuTOS `sh_name`, `gemshlib.c:305-318`):
/// strip a leading `X:` drive prefix, then return everything after the last
/// path separator. `"C:\APPS\QED.NAP"` → `"QED.NAP"`, `"C:QED.NAP"` →
/// `"QED.NAP"`, `"QED.NAP"` → `"QED.NAP"`.
fn filename_portion(name: &[u8]) -> &[u8] {
    let mut s = name;
    if s.len() >= 2 && s[1] == b':' {
        s = &s[2..];
    }
    match s.iter().rposition(|&c| c == b'\\') {
        Some(pos) => &s[pos + 1..],
        None => s,
    }
}

/// `shel_envrn` (125) — find the environment variable named by `addr_in[1]`
/// (e.g. `"PATH="`) and store a pointer to its value through the
/// `char **` at `addr_in[0]` (gemlib `a_shel_envrn.c:31,35`:
/// `mt_shel_envrn(char **result, …)` sets `aes_addrin[0] = (long)result`,
/// and the AES fills `*result`).
///
/// The output slot `*result` is a **caller-width** `char*` — 4 bytes for an
/// ILP32 app, 8 for an LP64 one — so the value pointer is stored at the
/// caller's pointer width via [`Abi::write_ptr`]. An 8-byte store into a
/// 4-byte ILP32 slot would clobber the following 4 bytes (this was the bug);
/// the same holds for the "not found" NULL, which must be a NULL of the
/// slot's width (gemlib immediately reads `*result`/`**result` back,
/// `a_shel_envrn.c:47`).
pub fn shel_envrn(pb: &Aespb) {
    let pslot = pb.addrin(0); // char ** — address of the caller's char* slot
    let search = pb.addrin(1) as *const u8;
    // `sh_envrn` is `void`: `int_out[0]` stays `1` in every path (spec
    // edge-case 13, `gemshlib.c:296`). A null output pointer cannot receive
    // the result, but the call still reports success.
    if pslot == 0 {
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
            let value = unsafe { env.as_ptr().add(start + nlen) } as u64;
            // SAFETY: pslot is the caller's non-null char* slot; store the
            // value pointer at the caller's pointer width so an ILP32 4-byte
            // slot is not overrun (LP64 stores the full 8 bytes).
            unsafe { pb.abi.write_ptr(pslot, 0, value) };
            pb.set_ret(1);
            return;
        }
        i += 1; // skip the NUL
    }
    // Not found → NULL (`*ppath = NULL`, `gemshlib.c:333,347`), still `1`.
    // SAFETY: as above; a NULL of the caller's pointer width.
    unsafe { pb.abi.write_ptr(pslot, 0, 0) };
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

/// `sh_draw(lcmd, TRUE)` — the AES's screen clear before a launched program
/// takes over (EmuTOS `gemshlib.c:269-286`, called from the shell loop at
/// `gemshlib.c:719`, "clear the screen").
///
/// EmuTOS does not clear the screen with a bespoke fill: it temporarily swaps
/// the built-in DESKTOP tree ROOT's `ob_spec` for `CLEAR_SCREEN` — `(WHITE<<12)
/// | (WHITE<<8) | (IP_HOLLOW<<4) | WHITE` = `0x0000` (`gemshlib.c:64`) — draws
/// that one object over the whole screen (`ob_draw(tree, ROOT, 0)` after
/// `gsx_sclip(&gl_rscreen)`) and puts the saved `ob_spec` straight back. The
/// hollow pattern reaches `vr_recfl` in REPLACE mode with an all-zero pattern,
/// so every pixel is written the background pen — an opaque white rectangle.
///
/// The swap-and-restore is what keeps this white-out a **one-shot**: the very
/// next background repaint (`w_drawdesk`, `gemwmlib.c:340-359`) uses the root's
/// real `ob_spec` again — the `0x1143` green dither, or the `.INF` `#Q` colour.
/// That is why this is a separate call and not a state the compositor is left
/// in; see [`super::window`]'s `fallback_root`.
pub fn sh_draw_clear() {
    let Some(backend) = crate::host::host().and_then(|h| h.raster()) else {
        return;
    };
    let tree = super::init::desktop_tree();
    // SAFETY: single-CPU AES, cooperative trap path; `desktop_tree()` is the
    // always-initialised three-object static built by `super::init::init`.
    unsafe {
        let root = &mut *tree;
        let specsave = root.ob_spec;
        root.ob_spec = CLEAR_SCREEN;
        // The software mouse cursor is a save-under sprite: painting over it
        // while it is shown would leave its backing store stale (the same
        // reason `window::compose` brackets its repaint).
        crate::vdi::cursor_hide(backend);
        super::object::draw_root(super::object::ObjTree::lp64(tree as *const _), 0, backend);
        crate::vdi::cursor_show(backend);
        root.ob_spec = specsave;
    }
}

/// `CLEAR_SCREEN` (`gemshlib.c:64`): `(WHITE<<12) | (WHITE<<8) | (IP_HOLLOW<<4)
/// | WHITE`, with `WHITE` = 0 (`obdefs.h`) and `IP_HOLLOW` = 0
/// (`obdefs.h:32`) — the all-zero colour word.
const CLEAR_SCREEN: u64 = 0;
