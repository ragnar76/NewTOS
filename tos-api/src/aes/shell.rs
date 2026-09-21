//! AES Scrap Manager (`scrp_*` 80–82) + Shell Manager (`shel_*` 120–127).
//!
//! Spec: [`docs/api/aes/scrap-shell.md`]. The **scrap** manager owns the
//! clipboard *directory path* (the data itself is exchanged as `SCRAP.*`
//! files through GEMDOS). The **shell** manager is the bridge between the
//! desktop/shell and launched programs: which program runs next
//! (`shel_write`), the same command/tail pair read back (`shel_read` — the
//! app's own start command until it `shel_write`s another), a
//! shared settings buffer (`shel_get`/`shel_put`), path resolution
//! (`shel_find`) and the environment (`shel_envrn`).
//!
//! `shel_write` records **which program runs next** in the `SHELL`
//! structure (`gemshlib.c:76-83`) — `sh_doexec`, `sh_nextapp`, `sh_isgem`
//! plus the AES globals `D.s_cmd` (the command) and `ad_stail` (its tail).
//! The actual launch is done by the shell loop, `kernel::shell_loop`, which
//! is EmuTOS `sh_main`/`sh_ldapp` (`gemshlib.c:592-731`); this module owns
//! the state and the policy those two read (`sh_chdef`, `set_default_desktop`,
//! `sh_find`, `sh_draw`/`sh_show`), exactly as `gemshlib.c` does.
//!
//! Spec: [`docs/api/aes/shell-loop.md`] §7-§9, §12.

use super::Aespb;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

/// Built-in environment used when the shell buffer is empty.
const DEFAULT_ENV: &[u8] = b"PATH=C:\\;\0\0";

/// `MAXPATHLEN` (`include/sysconf.h:51-55`: `LEN_ZPATH + LEN_ZFNAME + 1`) —
/// the size EmuTOS gives every fully-qualified path scratch buffer, from
/// `D.s_cmd` (`gemlib.h:122`) to `rs_load`'s `tmprsfname` (`gemrslib.c:46`).
pub const PATHBUF: usize = 128;
/// The shell settings buffer capacity — `SIZE_SHELBUF` (`sysconf.h:74`,
/// "same as TOS 1.04→"). `shel_get`/`shel_put` clamp `len` to this: a
/// safety bound over EmuTOS's unbounded `memcpy` (`gemshlib.c:205,215`,
/// spec edge-case 10) that does not change well-formed behaviour.
const SHELLBUF: usize = 4192;
/// `CMDTAILSIZE` (`aes/gemlib.h:97`, "architectural") — the size of the
/// `ad_stail` command-tail image `sh_write` fills and `sh_ldapp` hands to
/// `dos_exec`.
const CMDTAILSIZE: usize = 128;

/// `shel_write` mode flags (`doex`, `aesdefs.h:618-621`).
const SHW_NOEXEC: i16 = 0; // exit to desktop
/// `SHW_EXEC` — run `pcmd` after the current program terminates.
pub const SHW_EXEC: i16 = 1;
/// `SHW_SHUTDOWN` — shut the system down. Ends the shell loop
/// (`gemshlib.c:730`, `while (psh->sh_doexec != SHW_SHUTDOWN)`).
pub const SHW_SHUTDOWN: i16 = 4;
const SHW_RESCHNG: i16 = 5; // change screen resolution (request recorded; restart = A106)

// ── `sh_nextapp` — the kind of program the shell runs next ────────────────
//
// `gemshlib.c:69-73`. `sh_ldapp` dispatches on this (`:592-689`) and
// `sh_chdef` picks the drive/directory from it (`:537-566`).

/// A normal application, launched by `shel_write(SHW_EXEC, …)`.
pub const NORMAL_APP: i16 = 0;
/// The built-in console, selected by the magic name [`DEF_CONSOLE`]
/// (`gemshlib.c:172`).
pub const CONSOLE_APP: i16 = 1;
/// The autorun program named by the `.INF` file's `#Z` line
/// (`geminit.c:438-460` → [`sh_wdef`]).
pub const AUTORUN_APP: i16 = 2;
/// The built-in desktop (`gemshlib.c:600-612`).
pub const DESKTOP_APP: i16 = 3;

/// `DEF_CONSOLE` (`include/aesdefs.h:627`) — the magic program name that makes
/// `sh_write` select [`CONSOLE_APP`] instead of [`NORMAL_APP`]
/// (`gemshlib.c:172`). **Contract**: an application may pass this literal to
/// `shel_write`, so the spelling is observable (`scrap-shell.md` item 8).
pub const DEF_CONSOLE: &[u8] = b"EMUCON";

/// `DEF_DESKTOP` (`aes/geminit.h:13` = `"EMUDESK"`) — the magic name
/// `set_default_desktop` puts in `sh_desk` to mean "the built-in desktop"
/// (`gemshlib.c:587`). NewTOS's built-in desktop is `kernel::desk`, so the
/// name is `NEWDESK`; it is never typed by a caller (EmuTOS only ever
/// compares `sh_nextapp`, never this string), so the spelling is
/// **reference**, not contract (`shell-loop.md` §12).
pub const DEF_DESKTOP: &[u8] = b"NEWDESK";

/// `PATHSEP` (`include/sysconf.h:58`).
const PATHSEP: u8 = b'\\';
/// `DRIVESEP` (`include/sysconf.h:57`).
const DRIVESEP: u8 = b':';

/// `gl_shgem` (`aes/gemglobe.c`) — TRUE while the screen is in graphics
/// (GEM) mode, FALSE in alpha (character) mode. `sh_chgrf` flips it,
/// `sh_show` and `sh_ldapp`'s `wm_new` call are gated on it
/// (`gemshlib.c:292-297`, `:676-677`).
static GL_SHGEM: AtomicBool = AtomicBool::new(false);

/// Read `gl_shgem`.
pub fn gl_shgem() -> bool {
    GL_SHGEM.load(Ordering::Relaxed)
}

/// Write `gl_shgem`. Only the shell loop's `sh_chgrf`/`sh_main` do this
/// (`gemshlib.c:513-523`, `:705`).
pub fn set_gl_shgem(on: bool) {
    GL_SHGEM.store(on, Ordering::Relaxed);
}

/// FAT `FA_SUBDIR` attribute bit (a directory, not a deletable file).
const FA_SUBDIR: u8 = 0x10;
/// GEMDOS `Fdelete` function id and the GEMDOS trap number, for building the
/// `TrapArgs` `scrp_clear` hands to [`crate::bdos::fs::file::fdelete`].
const FDELETE: u16 = 0x41;
const TRAP_GEMDOS: u16 = 1;
/// GEMDOS function ids the shell's `set_drvdir`/`sh_curdrvdir` drive
/// (`dos_sdrv`/`dos_chdir`/`dos_gdir`, `aes/gemdos.h`).
const DSETDRV: u16 = 0x0E;
const DSETPATH: u16 = 0x3B;
const DGETPATH: u16 = 0x47;

/// The `SHELL sh` structure (`gemshlib.c:76-83`) plus the AES globals the
/// shell manager owns: `D.s_cmd` (the command of the program to run next),
/// `ad_stail` (its command tail) and `sh_apdir` (`:86-87`, the directory a
/// `shel_write`-launched application is to be started in).
///
/// EmuTOS sizes `sh_desk` at `LEN_ZFNAME` (13) and `sh_cdir` at `LEN_ZPATH`
/// (114) (`sysconf.h:51-52`); `D.s_cmd` is a `MAXPATHLEN` (128) buffer. We
/// use one [`PATHBUF`] (= 128 = `MAXPATHLEN`) for all of them, which is >=
/// every EmuTOS bound, so no well-formed value is truncated differently.
struct State {
    scrap: UnsafeCell<[u8; PATHBUF]>, // clipboard dir (empty = never set)
    // ── SHELL sh (gemshlib.c:76-83) ──────────────────────────────────────
    sh_doexec: UnsafeCell<i16>,  // last `shel_write` mode
    sh_nextapp: UnsafeCell<i16>, // NORMAL_APP / CONSOLE_APP / AUTORUN_APP / DESKTOP_APP
    sh_isgem: UnsafeCell<i16>,   // 1 = GEM/graphic, 0 = character mode
    // `isover` (`SH_ISCR`, `int_in[2]`) as last handed to
    // `shel_write(SHW_EXEC)`. Not a member of EmuTOS's `SHELL`: `sh_write`
    // takes it as a parameter and the `SHW_EXEC` arm ignores it
    // ("isover ignored, should currently always be 1", `gemshlib.c:149`;
    // TOS 4 would take `1` as an ARGV request, which EmuTOS does not build).
    // Retained so the binding's third declared word is provably read (A61);
    // no consumer, read back only by [`sh_isover`].
    sh_isover: UnsafeCell<i16>,
    sh_desk: UnsafeCell<[u8; PATHBUF]>, // name of the default startup app
    sh_cdir: UnsafeCell<[u8; PATHBUF]>, // its directory
    // ── AES globals the shell owns ───────────────────────────────────────
    // `D.s_cmd`/`ad_stail` are the **one** command/tail pair of the AES:
    // `shel_write(SHW_EXEC)` writes them, the launcher runs them, and
    // `shel_read` reads them (`gemshlib.c:104-105,169-170,645`; H05). There
    // is no separate "what started this app" copy — see `shel_read`.
    s_cmd: UnsafeCell<[u8; PATHBUF]>,    // D.s_cmd — command to run next
    s_tail: UnsafeCell<[u8; PATHBUF]>,   // ad_stail — its command tail
    sh_apdir: UnsafeCell<[u8; PATHBUF]>, // gemshlib.c:86-87
    buf: UnsafeCell<[u8; SHELLBUF]>,     // shel_get/shel_put settings blob
    env: UnsafeCell<[u8; PATHBUF]>,      // environment (empty = default)
    // ── the pending resolution-change request (gemshlib.c:92-99) ─────────
    // `gl_changerez`: 0 = no change, 1 = ST/TT family, 2 = Falcon family;
    // `gl_nextrez`: the mode word for `Setscreen` (with the caller's `+2`
    // ST/TT offset, D-101). Written by `shel_write(SHW_RESCHNG)` exactly as
    // `sh_write` does (`:187-191`, A48 Tier 1). Not yet consumed: `sh_main`'s
    // `!gl_changerez` exit term and the `gem_main` restart around it are
    // A106 (Tier 2) — until then a request is recorded and readable
    // ([`gl_changerez`]/[`gl_nextrez`]) but the shell loop ignores it.
    gl_changerez: UnsafeCell<i16>,
    gl_nextrez: UnsafeCell<i16>,
}
// SAFETY: single boot CPU only touches the AES.
unsafe impl Sync for State {}

static S: State = State {
    scrap: UnsafeCell::new([0; PATHBUF]),
    sh_doexec: UnsafeCell::new(SHW_EXEC),
    sh_nextapp: UnsafeCell::new(DESKTOP_APP),
    sh_isgem: UnsafeCell::new(1),
    sh_isover: UnsafeCell::new(0),
    sh_desk: UnsafeCell::new([0; PATHBUF]),
    sh_cdir: UnsafeCell::new([0; PATHBUF]),
    s_cmd: UnsafeCell::new([0; PATHBUF]),
    s_tail: UnsafeCell::new([0; PATHBUF]),
    sh_apdir: UnsafeCell::new([0; PATHBUF]),
    buf: UnsafeCell::new([0; SHELLBUF]),
    env: UnsafeCell::new([0; PATHBUF]),
    gl_changerez: UnsafeCell::new(0),
    gl_nextrez: UnsafeCell::new(0),
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

/// `shel_read` (120) — `strcpy(pcmd, D.s_cmd); memcpy(ptail, ad_stail,
/// CMDTAILSIZE)` → `addr_in[0]`/`addr_in[1]` (`sh_read`, `gemshlib.c:102-106`).
///
/// **One buffer, not two** (`docs/api/aes/scrap-shell.md` § `shel_read`, H05).
/// `D.s_cmd`/`ad_stail` are the cells [`shel_write`]`(SHW_EXEC)` writes
/// (`:169-170`) and the launcher runs (`:645`, [`set_launch_info`]). So a
/// program launched by the shell loop or the desktop reads back its own start
/// command and tail — until it calls `shel_write(SHW_EXEC, cmd, tail)` itself,
/// after which `shel_read` returns **that** pair. TeraDesk's Single-TOS chain
/// protocol (`shel_write(SHW_EXEC, child)` → `Pexec(child)` → `shel_read` →
/// `strcmp(cmd, own name)`, `startprg.c:344-355`) depends on exactly this;
/// answering from a separate launch-only buffer made it start a second copy
/// of itself (`TASK/H05-STATUS-DONE.MD`).
///
/// **Accepted LOW deviation (F8).** EmuTOS copies a fixed `CMDTAILSIZE = 128`
/// bytes (`memcpy`, `gemshlib.c:105`); we round-trip the tail image
/// (`tail[0]` = length, characters follow) as a NUL-terminated C string,
/// the mirror of [`tail_in`]. This is byte-exact for conforming text tails
/// and only truncates a tail carrying an embedded NUL (a rare binary tail).
/// Kept deliberately to avoid an unconditional 128-byte over-read of an EL0
/// user pointer for a low-value case (spec edge-case 6).
pub fn shel_read(pb: &Aespb) {
    // SAFETY: single-CPU AES.
    unsafe {
        cstr_out(pb.addrin(0) as *mut u8, &*S.s_cmd.get());
        cstr_out(pb.addrin(1) as *mut u8, &*S.s_tail.get());
    }
    pb.set_ret(1);
}

/// Publish the command + command-tail image the launcher is about to run as
/// `D.s_cmd`/`ad_stail` — the cells [`shel_read`] answers from.
///
/// In EmuTOS those cells are already filled when `sh_ldapp` reaches
/// `dos_exec(PE_LOADGO, D.s_cmd, ad_stail, ad_envrn)` (`gemshlib.c:645`): by
/// the desktop's `shel_write(SHW_EXEC, …)` (`desk/deskpro.c:41` → `:169-170`)
/// or by `sh_chdef` for an autorun entry (`:557-562`). NewTOS's shell loop
/// takes the same route (`kernel/src/shell_loop.rs` reads `D.s_cmd`/`ad_stail`
/// and hands them to the launcher, so this call rewrites the same values),
/// but the built-in desktop's double-click launches through the kernel
/// launcher directly, without a `shel_write` (AES gap G1). The launcher
/// therefore calls this just before the program enters EL0 on **every** path,
/// so both are covered by one writer.
///
/// `cmd` is the full launch path (`C:\APPS\QED\QED.NAP`, NUL-terminated);
/// `tail` is the Atari command-tail **image** exactly as handed to the loader
/// for `p_cmdlin` (`tail[0]` = length, `tail[1..]` = characters; empty slice =
/// empty tail). It is stored by the same rule as [`tail_in`] (`1 + tail[0]`
/// bytes, capped at [`CMDTAILSIZE`]), so `shel_read` returns the identical
/// image either way (F8).
pub fn set_launch_info(cmd: &[u8], tail: &[u8]) {
    // SAFETY: single-CPU launcher context; the AES is not dispatching.
    unsafe {
        store_str(&mut *S.s_cmd.get(), cmd);
        store_tail_image(&mut *S.s_tail.get(), tail);
    }
}

/// Copy a command-tail image slice into `dst` by the [`tail_in`] rule: the
/// length byte plus that many characters, capped at [`CMDTAILSIZE`] and at
/// what the slice actually holds; an empty slice is the empty tail (`[0]`).
fn store_tail_image(dst: &mut [u8], image: &[u8]) {
    dst.fill(0);
    let n = match image.first() {
        Some(&len) => (len as usize + 1)
            .min(CMDTAILSIZE)
            .min(dst.len())
            .min(image.len()),
        None => 0,
    };
    dst[..n].copy_from_slice(&image[..n]);
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
/// level / rez family); `addr_in[0]`/`[1]` = command + tail. The binding
/// declares all three words (`desk/gembind.c:914`, `aesdefs.h:600-602`) and
/// the dispatcher hands all three to `sh_write` (`gemsuper.c:384`). **Always
/// returns `1`** (`gemshlib.c:195`); the actual launch is done by the shell
/// loop ([`crate::aes::shell`] module docs), which reads the state this call
/// leaves behind. Dispatches on the mode:
///
/// * `SHW_EXEC` (1) — `strcpy(D.s_cmd, pcmd)`, copy the tail into `ad_stail`,
///   set `sh_doexec`, classify the program by the magic name
///   ([`DEF_CONSOLE`] → [`CONSOLE_APP`], else [`NORMAL_APP`],
///   `gemshlib.c:172`), record `isgem`, and for a `NORMAL_APP` save the
///   caller's current drive+directory in `sh_apdir` (`:174-175`) so
///   `sh_chdef` can restore it before the child runs. `isover` is read and
///   retained ([`sh_isover`]) but has no effect — EmuTOS's arm ignores it
///   ("should currently always be 1", `:149`; TOS 4's ARGV request, which
///   EmuTOS does not implement).
/// * `SHW_NOEXEC` (0) — exit to the desktop: `D.s_cmd = DEF_DESKTOP`,
///   [`DESKTOP_APP`], forced GEM mode (`gemshlib.c:161-167`).
/// * `SHW_SHUTDOWN` (4) — EmuTOS records it only `if (can_shutdown())`
///   (`gemshlib.c:178-187`, `#if CONF_WITH_SHUTDOWN`), and `can_shutdown()`
///   is `FALSE` on every machine without a native power-off — including the
///   Falcon this build profile targets (`bios/bios.c:754-770`, the `#else
///   return FALSE` arm). NewTOS has no power-off path on RPi4/AArch64
///   (D-101) either, so the request is recorded nowhere and `sh_doexec`
///   keeps its old value — the shell loop keeps running, exactly as on a
///   real Falcon.
/// * `SHW_RESCHNG` (5) — records the restart request **unconditionally**:
///   `gl_changerez = 1 + isover`, `gl_nextrez = isgem` (`gemshlib.c:188-189`;
///   no `check_moderez`, no comparison with the current mode — the
///   "no change" filter is the caller's, `desk/deskrez.c:213-215`) and clears
///   `D.s_cmd`/`ad_stail` (`:190-191`). The `SHELL` members are not touched.
///   Readable through [`gl_changerez`]/[`gl_nextrez`]; the shell loop's
///   `!gl_changerez` exit term and the AES restart it triggers are A106
///   (Tier 2) — until then the request is recorded but has no effect
///   (`docs/analysis/a48-shw-reschng-design-2026-09-21.md` §5).
/// * any other `doex` — silently ignored, still returns `1` (spec edge-case
///   9; EmuTOS's `switch` has no `default`).
pub fn shel_write(pb: &Aespb) {
    let doex = pb.intin(0);
    let isgem = pb.intin(1);
    // `SH_ISCR` — `sh_write`'s `isover` (gemsuper.c:384, gemshlib.c:157).
    let isover = pb.intin(2);
    // SAFETY: single-CPU AES.
    unsafe {
        match doex {
            SHW_NOEXEC => {
                // `strcpy(D.s_cmd, DEF_DESKTOP)` (gemshlib.c:163).
                store_str(&mut *S.s_cmd.get(), DEF_DESKTOP);
                *S.sh_doexec.get() = doex;
                *S.sh_nextapp.get() = DESKTOP_APP;
                *S.sh_isgem.get() = 1;
            }
            SHW_EXEC => {
                cstr_in(&mut *S.s_cmd.get(), pb.addrin(0) as *const u8);
                tail_in(&mut *S.s_tail.get(), pb.addrin(1) as *const u8);
                *S.sh_doexec.get() = doex;
                // `strcmp(pcmd, DEF_CONSOLE) == 0 ? CONSOLE_APP : NORMAL_APP`
                // (gemshlib.c:172) — the one magic name a caller can type.
                let cmd = &*S.s_cmd.get();
                let next = if &cmd[..clen(cmd)] == DEF_CONSOLE {
                    CONSOLE_APP
                } else {
                    NORMAL_APP
                };
                *S.sh_nextapp.get() = next;
                *S.sh_isgem.get() = i16::from(isgem != 0);
                // Retained, not acted on (gemshlib.c:149; A61).
                *S.sh_isover.get() = isover;
                if next == NORMAL_APP {
                    // `sh_curdrvdir(sh_apdir)` — save the app's current
                    // directory (gemshlib.c:174-175).
                    sh_curdrvdir(&mut *S.sh_apdir.get());
                }
            }
            SHW_RESCHNG => {
                // `gl_changerez = 1 + isover; gl_nextrez = isgem;`
                // (gemshlib.c:188-189) — unconditionally: `sh_write` neither
                // validates the mode nor compares it with the current one
                // (that is the caller's job, desk/deskrez.c:213-215). The
                // request is recorded here and consumed by A106.
                *S.gl_changerez.get() = 1i16.wrapping_add(isover);
                *S.gl_nextrez.get() = isgem;
                // `D.s_cmd[0] = '\0'; ad_stail[0] = '\0';` (:190-191).
                (*S.s_cmd.get())[0] = 0;
                (*S.s_tail.get())[0] = 0;
            }
            _ => {}
        }
    }
    pb.set_ret(1);
}

/// `sh_curdrvdir` (`gemshlib.c:107-122`) — write `"<X>:<curdir>"` for the
/// current drive into `ppath`, where an empty current directory becomes a
/// bare `"\"` (`:118-121`).
fn sh_curdrvdir(ppath: &mut [u8]) {
    use crate::trap::TrapArgs;
    let drive = crate::bdos::fs::drive::current_drive();
    ppath.fill(0);
    ppath[0] = b'A' + drive as u8;
    ppath[1] = DRIVESEP;
    // `dos_gdir(drive+1, ppath)` — Dgetpath writes the path (no drive letter)
    // starting at ppath[2].
    let args = TrapArgs {
        regs: [ppath[2..].as_mut_ptr() as u64, drive as u64 + 1, 0, 0, 0, 0, 0, 0],
        func: DGETPATH as u64,
        svc_imm: TRAP_GEMDOS,
    };
    let _ = crate::bdos::fs::dirops::dgetpath(&args);
    if ppath[2] == 0 {
        ppath[2] = PATHSEP;
        ppath[3] = 0;
    }
}

/// The current value of `sh_nextapp` (`gemshlib.c:77`).
pub fn sh_nextapp() -> i16 {
    // SAFETY: single-CPU AES.
    unsafe { *S.sh_nextapp.get() }
}

/// Overwrite `sh_nextapp` — `sh_ldapp`'s "run the desktop after a normal
/// application" preset (`gemshlib.c:641`).
pub fn set_sh_nextapp(v: i16) {
    // SAFETY: single-CPU AES.
    unsafe { *S.sh_nextapp.get() = v };
}

/// The current value of `sh_isgem` (`gemshlib.c:78`).
pub fn sh_isgem() -> bool {
    // SAFETY: single-CPU AES.
    unsafe { *S.sh_isgem.get() != 0 }
}

/// Overwrite `sh_isgem` (`gemshlib.c:642`).
pub fn set_sh_isgem(v: bool) {
    // SAFETY: single-CPU AES.
    unsafe { *S.sh_isgem.get() = i16::from(v) };
}

/// The `isover` word (`SH_ISCR`, `int_in[2]`) the last `shel_write(SHW_EXEC)`
/// handed over — `sh_write`'s third parameter (`gemshlib.c:157`), which its
/// `SHW_EXEC` arm does not use (`:149`, `:168-176`). Test hook (A61): lets a
/// host test show that the binding's third declared word (`gembind.c:914`,
/// `aesdefs.h:602`) reaches the manager. Nothing in the AES reads it.
pub fn sh_isover() -> i16 {
    // SAFETY: single-CPU AES.
    unsafe { *S.sh_isover.get() }
}

/// The current value of `sh_doexec` — the shell loop's termination test
/// (`gemshlib.c:730`).
pub fn sh_doexec() -> i16 {
    // SAFETY: single-CPU AES.
    unsafe { *S.sh_doexec.get() }
}

/// `gl_changerez` (`gemshlib.c:95,98`) — the pending resolution-change
/// request: `0` = none, `1` = ST/TT family, `2` = Falcon family, as
/// `shel_write(SHW_RESCHNG, …, isover)` last recorded it (`1 + isover`,
/// `:188`). Readable state only until A106 wires it into the shell loop's
/// exit test (`:727`) and the restart around `sh_main`.
pub fn gl_changerez() -> i16 {
    // SAFETY: single-CPU AES.
    unsafe { *S.gl_changerez.get() }
}

/// `gl_nextrez` (`gemshlib.c:96,99`) — the mode word `shel_write(SHW_RESCHNG,
/// isgem, …)` last recorded (`gl_nextrez = isgem`, `:189`), verbatim: for the
/// ST/TT family it carries the caller's `+2` offset (D-101). Readable state
/// only until A106 consumes it.
pub fn gl_nextrez() -> i16 {
    // SAFETY: single-CPU AES.
    unsafe { *S.gl_nextrez.get() }
}

/// Copy `D.s_cmd` — the fully-qualified command of the program to run next —
/// into `out` as a NUL-terminated string; returns its length.
pub fn s_cmd(out: &mut [u8]) -> usize {
    // SAFETY: single-CPU AES.
    let src = unsafe { &*S.s_cmd.get() };
    copy_out(out, src)
}

/// `memcpy(ad_stail, ptail, CMDTAILSIZE)` (`gemshlib.c:170`) — copy the Atari
/// command-tail **image** at `src` into `ad_stail`.
///
/// The image is length-prefixed: `ptail[0]` is the character count and the
/// characters follow (`desk/deskpro.c:56-57`, `tail[0] = strlen(tail+1);`), so
/// it may carry embedded NULs and must not be copied as a C string — doing so
/// truncates every non-empty tail at its own length byte.
///
/// We copy `1 + ptail[0]` bytes (capped at [`CMDTAILSIZE`]) rather than a blind
/// `CMDTAILSIZE`: that is exactly the well-formed content of the image, and it
/// does not over-read an EL0 user buffer the caller sized to its own tail —
/// the same reasoning as the [`shel_read`] F8 deviation, applied to the write
/// side.
fn tail_in(dst: &mut [u8], src: *const u8) {
    dst.fill(0);
    if src.is_null() {
        return;
    }
    // SAFETY: trusted GEM pointer to a length-prefixed command-tail image.
    unsafe {
        let n = (*src as usize + 1).min(CMDTAILSIZE).min(dst.len());
        for (i, b) in dst.iter_mut().take(n).enumerate() {
            *b = *src.add(i);
        }
    }
}

/// Copy `ad_stail` — the length-prefixed command-tail image for the program to
/// run next — into `out`; returns its length (`1 + out[0]`). This is the image
/// `sh_ldapp` hands to `dos_exec` verbatim (`gemshlib.c:645`).
pub fn s_tail_image(out: &mut [u8]) -> usize {
    // SAFETY: single-CPU AES.
    let src = unsafe { &*S.s_tail.get() };
    out.fill(0);
    let n = (src[0] as usize + 1).min(out.len()).min(src.len());
    out[..n].copy_from_slice(&src[..n]);
    n
}

/// Overwrite `D.s_cmd`. `sh_find` rewrites it in place when it resolves a
/// bare name to a fully-qualified one (`gemshlib.c:629`, `findfile`).
pub fn set_s_cmd(cmd: &[u8]) {
    // SAFETY: single-CPU AES.
    unsafe { store_str(&mut *S.s_cmd.get(), cmd) };
}

/// Copy a NUL-terminated string out of a fixed state buffer into `out`,
/// truncating to `out.len() - 1`; returns the length written.
fn copy_out(out: &mut [u8], src: &[u8]) -> usize {
    out.fill(0);
    let n = clen(src).min(out.len().saturating_sub(1));
    out[..n].copy_from_slice(&src[..n]);
    n
}

/// `sh_wdef` (`gemshlib.c:504-510`) — the internal writer behind
/// [`shel_wdef`], also called by `process_inf2` for the `.INF` `#Z` line
/// (`geminit.c:455`). Stores the (program-name, directory) **pair**;
/// `sh_chdef` recombines them into `D.s_cmd` for an [`AUTORUN_APP`]
/// (`gemshlib.c:552-563`).
pub fn sh_wdef(lpcmd: &[u8], lpdir: &[u8]) {
    // SAFETY: single-CPU AES.
    unsafe {
        store_str(&mut *S.sh_desk.get(), lpcmd);
        store_str(&mut *S.sh_cdir.get(), lpdir);
    }
}

/// `set_default_desktop` (`gemshlib.c:583-589`) — make the **built-in**
/// desktop the next program: `sh_nextapp = DESKTOP_APP`, `sh_isgem = TRUE`,
/// `sh_desk = DEF_DESKTOP` and `sh_cdir = D.s_cdir` (the AES's current
/// directory, i.e. the root of the current drive as `run_accs_and_desktop`
/// built it, `geminit.c:733`).
pub fn set_default_desktop() {
    // SAFETY: single-CPU AES.
    unsafe {
        *S.sh_nextapp.get() = DESKTOP_APP;
        *S.sh_isgem.get() = 1;
        store_str(&mut *S.sh_desk.get(), DEF_DESKTOP);
        let mut cdir = [0u8; PATHBUF];
        build_root_path(&mut cdir);
        store_str(&mut *S.sh_cdir.get(), &cdir);
    }
}

/// `build_root_path(D.s_cdir, 'A'+dos_gdrv())` (`geminit.c:733`,
/// `util/miscutil.c`) — `"<X>:\"` for the current drive.
fn build_root_path(out: &mut [u8]) {
    out.fill(0);
    out[0] = b'A' + crate::bdos::fs::drive::current_drive() as u8;
    out[1] = DRIVESEP;
    out[2] = PATHSEP;
}

/// `sh_main`'s prologue (`gemshlib.c:694-706`) — seed the `SHELL` for the
/// first pass of the loop. `isauto`/`isgem` come from `process_inf2`
/// ([`super::init::process_inf2`], `geminit.c:734,739`).
pub fn sh_main_init(isauto: bool, isgem: bool) {
    // SAFETY: single-CPU AES.
    unsafe {
        *S.sh_doexec.get() = SHW_EXEC;
        *S.sh_nextapp.get() = if isauto { AUTORUN_APP } else { DESKTOP_APP };
        *S.sh_isgem.get() = i16::from(isgem);
        // `strcpy(sh_apdir, D.s_cdir)` (:702) — the AES's current directory.
        let mut cdir = [0u8; PATHBUF];
        build_root_path(&mut cdir);
        store_str(&mut *S.sh_apdir.get(), &cdir);
    }
    set_gl_shgem(true); // `gl_shgem = TRUE` (:703)
    if !isauto {
        // `if (psh->sh_nextapp == DESKTOP_APP) set_default_desktop(psh);`
        // (:705-706)
        set_default_desktop();
    }
}

/// `set_drvdir` (`gemshlib.c:526-534`) — `dos_sdrv(drive)` for an explicit
/// `X:` prefix, then `dos_chdir(path)`.
fn set_drvdir(path: &[u8]) {
    use crate::trap::TrapArgs;
    let n = clen(path);
    if n >= 2 && path[1] == DRIVESEP {
        let d = path[0].to_ascii_uppercase();
        if d.is_ascii_uppercase() {
            let args = TrapArgs {
                regs: [(d - b'A') as u64, 0, 0, 0, 0, 0, 0, 0],
                func: DSETDRV as u64,
                svc_imm: TRAP_GEMDOS,
            };
            let _ = crate::bdos::fs::drive::dsetdrv(&args);
        }
    }
    // `dos_chdir(path)`: Dsetpath takes a NUL-terminated kernel pointer.
    let mut buf = [0u8; PATHBUF];
    let m = n.min(buf.len() - 1);
    buf[..m].copy_from_slice(&path[..m]);
    let args = TrapArgs {
        regs: [buf.as_ptr() as u64, 0, 0, 0, 0, 0, 0, 0],
        func: DSETPATH as u64,
        svc_imm: TRAP_GEMDOS,
    };
    let _ = crate::bdos::fs::dirops::dsetpath(&args);
}

/// `sh_chdef` (`gemshlib.c:537-566`) — set the drive and directory the next
/// program is to start in, and (for the desktop/autorun arms) build the
/// fully-qualified `D.s_cmd` from the `sh_cdir`/`sh_desk` pair `sh_wdef`
/// stored. Called once per shell-loop iteration, before `sh_chgrf`
/// (`gemshlib.c:712`).
pub fn sh_chdef() {
    // SAFETY: single-CPU AES; the state buffers are stable statics.
    unsafe {
        let nextapp = *S.sh_nextapp.get();
        match nextapp {
            NORMAL_APP => set_drvdir(&*S.sh_apdir.get()),
            // `#if WITH_CLI` — set by the desktop via `shel_wdef()`.
            CONSOLE_APP => set_drvdir(&*S.sh_cdir.get()),
            AUTORUN_APP | DESKTOP_APP => {
                let cdir = &*S.sh_cdir.get();
                set_drvdir(cdir);
                // Build the fully-qualified name, prefixing the directory
                // only for AUTORUN_APP (`:551-563`).
                let mut cmd = [0u8; PATHBUF];
                let mut n = 0usize;
                if nextapp == AUTORUN_APP {
                    // `strcpy(D.s_cmd, psh->sh_cdir); n = strlen(D.s_cmd);
                    //  if (n) D.s_cmd[n++] = PATHSEP;` (`:557-561`).
                    // `sh_wdef`'s directory never carries a trailing
                    // separator: `process_inf2` produces it by overwriting
                    // the `#Z` path's last separator with a NUL
                    // (`geminit.c:449-450`), so `C:\X\P.NAP` splits into
                    // (`C:\X`, `P.NAP`) and `C:\P.NAP` into (`C:`, `P.NAP`).
                    n = clen(cdir).min(cmd.len() - 2);
                    cmd[..n].copy_from_slice(&cdir[..n]);
                    if n > 0 {
                        cmd[n] = PATHSEP;
                        n += 1;
                    }
                }
                let desk = &*S.sh_desk.get();
                let m = clen(desk).min(cmd.len() - n - 1);
                cmd[n..n + m].copy_from_slice(&desk[..m]);
                store_str(&mut *S.s_cmd.get(), &cmd);
            }
            _ => {}
        }
    }
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
    let found = sh_find(&mut name);
    if found {
        cstr_out(p, &name);
    }
    pb.set_ret(i16::from(found));
}

/// `sh_find` (`gemshlib.c:474-484`) — the internal search behind
/// [`shel_find`], rewriting `spec` in place to the fully-qualified name on a
/// hit. Also called by `sh_ldapp` for a [`NORMAL_APP`] launch
/// (`gemshlib.c:629`); an [`AUTORUN_APP`] deliberately skips it.
///
/// `spec` is a NUL-terminated in/out buffer of at least [`PATHBUF`] bytes.
pub fn sh_find(spec: &mut [u8]) -> bool {
    let n = clen(spec);
    let mut name = [0u8; PATHBUF];
    let m = n.min(name.len() - 1);
    name[..m].copy_from_slice(&spec[..m]);
    let n = m;

    let mut probe = [0u8; 1];

    // (1) The application directory (rlr->p_appdir), searched first with the
    //     filename portion only (sh_name, gemshlib.c:412-423).
    let mut appdir = [0u8; crate::aes::resource::APP_DIR_CAP];
    let alen = crate::aes::resource::app_dir(&mut appdir);
    if alen > 0 {
        let fname = sh_name(&name[..n]);
        // `D.g_work` (`WORKAREASIZE >= MAXPATHLEN`, `gemlib.h:98-99`) takes
        // `p_appdir` (<= 116) plus a name (<= 13); size for the sum so a long
        // application directory cannot silently drop the composition.
        let mut full = [0u8; PATHBUF + 16];
        if alen + fname.len() < full.len() {
            full[..alen].copy_from_slice(&appdir[..alen]);
            full[alen..alen + fname.len()].copy_from_slice(fname);
            if let Ok(st) = core::str::from_utf8(&full[..alen + fname.len()]) {
                if crate::bdos::fs::read_file(st, &mut probe).is_some() {
                    copy_out(spec, &full);
                    return true;
                }
            }
        }
    }

    // (2) The path as given (current directory when drive/dir-relative).
    if let Ok(st) = core::str::from_utf8(&name[..n]) {
        if crate::bdos::fs::read_file(st, &mut probe).is_some() {
            return true;
        }
    }
    // (3) Root of the current drive: "<X>:\<name>" (only for a bare name
    //     with no drive/path separator). X = the current drive letter.
    if !name[..n].contains(&PATHSEP) && !name[..n].contains(&DRIVESEP) {
        let drive = crate::bdos::fs::drive::current_drive() as u8;
        let mut full = [0u8; PATHBUF];
        full[0] = b'A' + drive;
        full[1] = DRIVESEP;
        full[2] = PATHSEP;
        let k = n.min(PATHBUF - 4);
        full[3..3 + k].copy_from_slice(&name[..k]);
        if let Ok(st) = core::str::from_utf8(&full[..3 + k]) {
            if crate::bdos::fs::read_file(st, &mut probe).is_some() {
                copy_out(spec, &full);
                return true;
            }
        }
    }
    // (4) AES PATH= — skipped: no loader-populated AES environment (see doc).
    false
}

/// `sh_name` (`gemshlib.c:305-318`) — the filename portion of a path: strip a
/// leading `X:` drive prefix, then return everything after the last path
/// separator. `"C:\APPS\QED.NAP"` → `"QED.NAP"`, `"C:QED.NAP"` →
/// `"QED.NAP"`, `"QED.NAP"` → `"QED.NAP"`. `sh_ldapp` uses it for the name it
/// shows and for `p_nameit` (`gemshlib.c:594`).
pub fn sh_name(name: &[u8]) -> &[u8] {
    let mut s = name;
    if s.len() >= 2 && s[1] == DRIVESEP {
        s = &s[2..];
    }
    match s.iter().rposition(|&c| c == PATHSEP) {
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
/// must **not** touch `D.s_cmd`, or a `shel_wdef` after a
/// `shel_write(SHW_EXEC, …)` would corrupt the program the shell loop is
/// about to run. Same writer as `process_inf2`'s `#Z` handling ([`sh_wdef`]).
pub fn shel_wdef(pb: &Aespb) {
    let mut cmd = [0u8; PATHBUF];
    let mut dir = [0u8; PATHBUF];
    cstr_in(&mut cmd, pb.addrin(0) as *const u8);
    cstr_in(&mut dir, pb.addrin(1) as *const u8);
    sh_wdef(&cmd, &dir);
    pb.set_ret(1);
}

/// `sh_show` (`gemshlib.c:292-297`) — draw the whole built-in `DESKTOP` tree
/// with `lcmd` as the text of its `DTNAME` object, i.e. the program name GEM
/// shows while a program is being started. A no-op in alpha mode
/// (`if (!gl_shgem) return;`, `:294-295`).
///
/// `sh_ldapp` calls it once per launch: `sh_show("")` for the desktop itself
/// ("like TOS, we don't display a name", `:603`) and `sh_show(fname)` for a
/// normal/autorun application (`:632`).
pub fn sh_show(lcmd: &[u8]) {
    if !gl_shgem() {
        return;
    }
    sh_draw(lcmd, false);
}

/// The name buffer `sh_draw(lcmd, FALSE)` points `DTNAME`'s `te_ptext` at.
/// EmuTOS stores the caller's pointer directly (`ted->te_ptext = lcmd`,
/// `gemshlib.c:285`); `lcmd` there is `D.s_cmd`, a static. Ours is a static
/// too, so the pointer stays valid after the call, as EmuTOS's does.
struct ShowName(UnsafeCell<[u8; PATHBUF]>);
// SAFETY: as [`State`] — single boot CPU, cooperative trap path.
unsafe impl Sync for ShowName {}
static SHOW_NAME: ShowName = ShowName(UnsafeCell::new([0; PATHBUF]));

/// `sh_draw` (`gemshlib.c:269-289`) — draw (part of) the built-in `DESKTOP`
/// tree.
///
/// With `clear == true` this is the AES's screen clear before a launched
/// program takes over, called from the shell loop (`gemshlib.c:719`, "clear
/// the screen"). EmuTOS does not clear with a bespoke fill: it temporarily
/// swaps the DESKTOP tree ROOT's `ob_spec` for `CLEAR_SCREEN` — `(WHITE<<12)
/// | (WHITE<<8) | (IP_HOLLOW<<4) | WHITE` = `0x0000` (`gemshlib.c:64`) —
/// draws that one object over the whole screen (`ob_draw(tree, ROOT, 0)`
/// after `gsx_sclip(&gl_rscreen)`) and puts the saved `ob_spec` straight
/// back. The hollow pattern reaches `vr_recfl` in REPLACE mode with an
/// all-zero pattern, so every pixel is written the background pen — an
/// opaque white rectangle.
///
/// The swap-and-restore is what keeps this white-out a **one-shot**: the very
/// next background repaint (`w_drawdesk`, `gemwmlib.c:340-359`) uses the root's
/// real `ob_spec` again — the `0x1143` green dither, or the `.INF` `#Q` colour.
/// That is why this is a separate call and not a state the compositor is left
/// in; see [`super::window`]'s `fallback_root`.
///
/// With `clear == false` ([`sh_show`]) the tree is drawn in full
/// (`MAX_DEPTH`) after pointing `DTNAME`'s `TEDINFO.te_ptext` at `lcmd` —
/// the program name GEM displays while starting a program.
fn sh_draw(lcmd: &[u8], clear: bool) {
    let Some(backend) = crate::host::host().and_then(|h| h.raster()) else {
        return;
    };
    let tree = super::init::desktop_tree();
    // SAFETY: single-CPU AES, cooperative trap path; `desktop_tree()` is the
    // always-initialised three-object static built by `super::init::init`.
    unsafe {
        // The software mouse cursor is a save-under sprite: painting over it
        // while it is shown would leave its backing store stale (the same
        // reason `window::compose` brackets its repaint).
        crate::vdi::cursor_hide(backend);
        if clear {
            let root = &mut *tree;
            let specsave = root.ob_spec;
            root.ob_spec = CLEAR_SCREEN;
            super::object::draw_root(super::object::ObjTree::lp64(tree as *const _), 0, backend);
            root.ob_spec = specsave;
        } else {
            // `ted->te_ptext = (char *)lcmd; ob_draw(tree, ROOT, MAX_DEPTH);`
            // (`gemshlib.c:284-286`).
            let name = &mut *SHOW_NAME.0.get();
            store_str(name, lcmd);
            super::init::set_dtname_text(name.as_ptr() as u64);
            super::object::draw_root(
                super::object::ObjTree::lp64(tree as *const _),
                MAX_DEPTH,
                backend,
            );
        }
        crate::vdi::cursor_show(backend);
    }
}

/// `sh_draw(D.s_cmd, TRUE)` — the shell loop's screen clear between programs
/// (`gemshlib.c:719`). See [`sh_draw`].
pub fn sh_draw_clear() {
    sh_draw(&[], true);
}

/// `MAX_DEPTH` (`include/obdefs.h:30`) — the `ob_draw` depth that means
/// "the whole tree".
const MAX_DEPTH: i16 = 8;

/// `CLEAR_SCREEN` (`gemshlib.c:64`): `(WHITE<<12) | (WHITE<<8) | (IP_HOLLOW<<4)
/// | WHITE`, with `WHITE` = 0 (`obdefs.h`) and `IP_HOLLOW` = 0
/// (`obdefs.h:32`) — the all-zero colour word.
const CLEAR_SCREEN: u64 = 0;
