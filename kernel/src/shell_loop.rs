//! The AES shell loop — EmuTOS `sh_main` / `sh_ldapp` (`aes/gemshlib.c`).
//!
//! Spec: `docs/api/aes/shell-loop.md` §7 (`sh_main`), §8 (`sh_ldapp`), §9
//! (`sh_chdef`/`sh_chgrf`/`sh_tographic`/`sh_toalpha`).
//! Stage **S3** of `docs/analysis/desktop-userspace-plan.md`.
//!
//! This is what makes the machine boot into *whichever* program the
//! configuration names instead of a program compiled into the kernel: the
//! `.INF` file's `#Z` line is read by `process_inf2`
//! ([`tos_api::aes::init::process_inf2`], EmuTOS `geminit.c:407-464`), split
//! into a (program, directory) pair by `sh_wdef`, recombined into a
//! fully-qualified command by `sh_chdef`, and launched here.
//!
//! **Layering.** EmuTOS puts the state, the policy *and* the loop in one file
//! (`aes/gemshlib.c`) because there the AES **is** the kernel. In NewTOS the
//! state and the policy live in [`tos_api::aes::shell`] (they are pure AES
//! bookkeeping and GEMDOS calls) and only the loop plus the actual program
//! start live here, because dropping to EL0
//! ([`crate::launch::run_program_with_tail_rc`]) is architecture-specific
//! kernel code. No behaviour is split — every function below is one EmuTOS
//! function, cited.
//!
//! **The one deliberate deviation, and where it is.** EmuTOS decides at
//! *build* time which user interface the machine has: `endrom.c:28-35` sets
//! `exec_os` to the AES entry `#if WITH_AES`, to EmuCON `#elif WITH_CLI`, and
//! `#error You must provide a main UI` otherwise. On an Atari that is a
//! build-time fact because every Atari has a screen. On our boards it is a
//! *runtime* fact (a board may come up without a raster backend), so the same
//! choice is made at runtime, in exactly one place: [`sh_ldapp`]'s
//! `DESKTOP_APP` arm. That is the hardware-layer carve-out of
//! `docs/COMPATIBILITY.md`; everything else follows `gemshlib.c` literally.

use tos_api::aes::shell::{
    self, ShExit, AUTORUN_APP, CONSOLE_APP, DESKTOP_APP, NORMAL_APP,
};

use crate::kprintln;

/// `LEN_ZPATH + LEN_ZFNAME + 1` = `MAXPATHLEN` (`include/sysconf.h:51-55`) —
/// the size of `D.s_cmd`.
const MAXPATHLEN: usize = 128;

/// `IS_BIOS_ERROR(n)` (`include/gemerror.h:64`): `(n < E_OK) && (n > EINVFN)`
/// — the BIOS error range `-1 … -31`, which `sh_main` does **not** raise an
/// alert for (`gemshlib.c:722`).
fn is_bios_error(n: i32) -> bool {
    n < tos_types::error::E_OK && n > tos_types::error::EINVFN
}

/// `sh_main(isauto, isgem)` (`gemshlib.c:692-731`) — the AES shell loop.
///
/// ```c
/// psh->sh_doexec  = SHW_EXEC;
/// psh->sh_nextapp = isauto ? AUTORUN_APP : DESKTOP_APP;
/// psh->sh_isgem   = isgem;
/// strcpy(sh_apdir, D.s_cdir);
/// gl_shgem = TRUE;
/// if (psh->sh_nextapp == DESKTOP_APP) set_default_desktop(psh);
/// do {
///     sh_chdef(psh);
///     sh_chgrf(psh);
///     if (gl_shgem) { wm_init(); ratinit(); sh_draw(D.s_cmd, TRUE); }
///     if (rc && !IS_BIOS_ERROR(rc)) fm_error(-rc-31);
///     rc = sh_ldapp(psh);
/// } while ((psh->sh_doexec != SHW_SHUTDOWN) && !gl_changerez);
/// if (gl_shgem) sh_toalpha();
/// ```
///
/// **Exit condition — both terms (A106).** `gl_changerez` has two producers
/// in EmuTOS. `process_inf1`'s boot-time `#E` parse (`geminit.c:343-392`)
/// goes through `check_moderez`, which reports no change on a board with
/// exactly one fixed mode (ADR-017 rev 2, D-101) — that producer is
/// legitimately FALSE here. `sh_write`'s `SHW_RESCHNG` arm
/// (`gemshlib.c:187-191`) is pure AES software state, set unconditionally by
/// `shel_write(SHW_RESCHNG, …)` (A48), and *that* is what ends this loop:
/// when the running program terminates with the request pending, the next
/// program is not started ([`tos_api::aes::shell::sh_main_continue`],
/// `gemshlib.c:727`) and the caller gets [`ShExit::RezChange`].
///
/// Returning instead of halting is the caller's job: the kernel's session
/// loop (`kernel/src/lib.rs`) is NewTOS's `gemstart.S:86-124` `aes_restart`
/// — it re-initialises the AES ([`tos_api::aes::reinit`]) and enters
/// `sh_main` again, and only [`ShExit::Shutdown`] falls through to `halt()`.
pub fn sh_main(isauto: bool, isgem: bool) -> ShExit {
    // `gemshlib.c:698-706` — seed the SHELL and, without an autorun entry,
    // make the built-in desktop the first program.
    shell::sh_main_init(isauto, isgem);

    let mut rc: i32 = 0;
    loop {
        shell::sh_chdef(); // :712
        sh_chgrf(); // :713

        if shell::gl_shgem() {
            // :715-720
            tos_api::aes::window::wm_init();
            ratinit();
            shell::sh_draw_clear();
        }

        // Alert for the *previous* iteration's error (`:722-723`). GEMDOS
        // codes (`rc <= EINVFN`) become a positive DOS error number for
        // `fm_error`; BIOS-level codes are suppressed.
        if rc != 0 && !is_bios_error(rc) {
            tos_api::aes::form::fm_error((-rc - 31) as i16);
        }

        rc = sh_ldapp(); // :725

        // `while ((psh->sh_doexec != SHW_SHUTDOWN) && !gl_changerez)` (:727)
        if !shell::sh_main_continue() {
            break;
        }
    }

    if shell::gl_shgem() {
        sh_toalpha(); // :733
    }

    let exit = shell::sh_main_exit();
    kprintln!(
        "[shell] sh_main: session over — {}",
        match exit {
            ShExit::Shutdown => "shutdown",
            ShExit::RezChange =>
                "shel_write(SHW_RESCHNG) pending, restarting the AES",
        }
    );
    exit
}

/// `sh_chgrf` (`gemshlib.c:513-523`) — switch the screen between graphics
/// (GEM) and alpha (character) mode when the program about to run needs the
/// other one.
///
/// ```c
/// if (psh->sh_isgem != gl_shgem) {
///     gl_shgem = psh->sh_isgem;
///     if (gl_shgem) sh_tographic(); else sh_toalpha();
/// }
/// ```
///
/// **Hardware-layer clamp:** `sh_tographic` cannot succeed without a raster
/// backend, so the desired mode is `sh_isgem && have_graphics()`. On every
/// board that has a screen this is exactly `sh_isgem` and the function is
/// EmuTOS's verbatim.
fn sh_chgrf() {
    let want = shell::sh_isgem() && have_graphics();
    if want != shell::gl_shgem() {
        shell::set_gl_shgem(want);
        if want {
            sh_tographic();
        } else {
            sh_toalpha();
        }
    }
}

/// Whether this machine has a screen — the runtime form of EmuTOS's
/// build-time `WITH_AES` (`bios/endrom.c:28-35`). See the module note.
fn have_graphics() -> bool {
    tos_api::host::host().and_then(|h| h.raster()).is_some()
}

/// `sh_tographic` (`gemshlib.c:223-239`) — convert the screen to graphics
/// mode for a GEM application: retake the vectors a character application may
/// have stepped on, open the graphics workstation, set the initial clip
/// rectangle, allocate screen space, start the mouse and show the hourglass.
///
/// On NewTOS the "workstation" is the framebuffer the VDI draws into and it is
/// never closed, so `gsx_graphic(TRUE)` has no counterpart to perform.
/// `gsx_malloc()` (:236) has one: the AES menu/alert screen buffer
/// ([`tos_api::aes::gsxif`]), taken from the BDOS pool the first time this
/// runs — before the first GEM program, so the block is kernel-owned and
/// survives every `Pterm` — and kept: `sh_toalpha` does **not** free it
/// (EmuTOS `gsx_mfree`, `:258`), so its address stays stable across an alpha
/// interlude (unobservable superset; `gsxif` module docs,
/// `docs/COMPATIBILITY.md`). What else is observable — an active mouse and
/// the hourglass shape — is done here. `retake()` restores the critical-error
/// handler and the AES trap vector, neither of which a NewTOS program can
/// displace (the trap vector is the exception table, not a writable
/// low-memory long).
///
/// Going graphic does **not** silence the character console: EmuTOS'
/// `bconout2` writes into screen memory whichever mode the AES thinks it is
/// in, so a program that prints here paints over the GEM screen, on TOS as on
/// NewTOS. See [`crate::fbcon`].
fn sh_tographic() {
    ratinit(); // `ratinit()` (:236)
    // `gsx_malloc()` (:236) — once; a later pass finds the buffer in place.
    let had_buffer = tos_api::aes::gsxif::gsx_mret().is_some();
    match tos_api::aes::gsxif::gsx_malloc() {
        Some(b) if !had_buffer => kprintln!(
            "[aes] gsx_malloc: screen buffer pa=0x{:08x} len={} (kernel-owned, WF_SCREEN)",
            b.pa, b.len,
        ),
        Some(_) => {}
        None => kprintln!("[aes] gsx_malloc: no screen buffer (pool short) -- WF_SCREEN answers FALSE"),
    }
    if let Some(b) = tos_api::host::host().and_then(|h| h.raster()) {
        // `set_mouse_to_hourglass()` (:237) — AES preset shape 2.
        tos_api::vdi::mouse::set_preset(2, b);
    }
}

/// `sh_toalpha` (`gemshlib.c:246-260`) — convert the screen back to alpha
/// mode for a character-mode program: arrow cursor, give back the error
/// handler, turn the mouse off, free screen space, close the workstation.
///
/// The observable half here is the arrow shape, the mouse going away
/// (`ratexit`) and the VT52 text console taking the screen.
fn sh_toalpha() {
    if let Some(b) = tos_api::host::host().and_then(|h| h.raster()) {
        // `set_mouse_to_arrow()` (:248) — AES preset shape 0.
        tos_api::vdi::mouse::set_preset(0, b);
        // `ratexit()` (:256) — `gsx_moff()`, hide the cursor.
        tos_api::vdi::mouse::hide(b);
    }
    crate::fbcon::clear_and_home();
}

/// `ratinit` (`aes/gemgsxif.c:205-209`) — `gsx_1code(SHOW_CUR, 0); gl_moff = 0;`
/// i.e. show the mouse cursor at its current position. The `0` is `v_show_c`'s
/// `reset` argument, which forces the cursor visible regardless of how many
/// `v_hide_c` calls preceded it — [`tos_api::vdi::cursor_show_reset`], not the
/// counted [`tos_api::vdi::cursor_show`]. It does **not** move the pointer; the
/// VDI centres it once when the workstation opens (`vdi/vdi_mouse.c:582`,
/// `GCURX = xres / 2`).
fn ratinit() {
    if let Some(b) = tos_api::host::host().and_then(|h| h.raster()) {
        tos_api::vdi::cursor_show_reset(b);
    }
}

/// `run_accs_and_desktop`'s `sh_tographic()` (`aes/geminit.c:700`, "go into
/// graphic mode") — the one-time switch out of the character screen the BIOS
/// and the `\AUTO\` programs used, done **before** `process_inf2` and
/// `sh_main`. `sh_main` then records the resulting state as `gl_shgem = TRUE`
/// (`gemshlib.c:703`).
///
/// A machine without a raster backend cannot make this switch; the loop's
/// first `sh_chgrf` notices (`gl_shgem` true, desired false) and drops back to
/// alpha mode, which routes the boot into the console arm — see the module
/// note.
pub fn go_graphic() {
    if have_graphics() {
        sh_tographic();
    }
}

/// `sh_ldapp(psh)` (`gemshlib.c:592-689`) — run the next program and block
/// until it terminates. Three top-level arms, four `sh_nextapp` values.
///
/// Returns the GEMDOS code `sh_main` may turn into an alert: `0` on success,
/// the child's negative `dos_exec` result, or `EFILNF` when `sh_find` could
/// not locate a [`NORMAL_APP`].
fn sh_ldapp() -> i32 {
    let mut cmd = [0u8; MAXPATHLEN];
    shell::s_cmd(&mut cmd);
    let nextapp = shell::sh_nextapp();

    kprintln!(
        "[shell] sh_ldapp: starting {}, sh_nextapp={}, sh_isgem={}",
        as_str(&cmd),
        nextapp,
        i32::from(shell::sh_isgem()),
    );

    // ── Arm 1 — DESKTOP_APP (`:600-612`) ─────────────────────────────────
    if nextapp == DESKTOP_APP {
        if !shell::gl_shgem() {
            // The machine has no screen, so the built-in desktop is not the
            // main UI this boot — `endrom.c:28-35`'s choice, made at runtime.
            // Fall through to the console arm, which is the other main UI
            // EmuTOS provides (`endrom.c:33`, `coma_start`).
            report_no_desktop();
            return run_console_arm();
        }
        shell::sh_show(&[]); // `sh_show("")` — no name for the desktop (:603)
        p_setappdir(&cmd); // (:605)
        crate::desk::run(); // `aes_run_rom_program(deskstart)` (:606)
        // EmuTOS calls `wm_new()` only when the desktop terminated
        // *abnormally* (`:606-610`); the normal exit path already cleaned up
        // in `deskmain`'s epilog. Our built-in desktop has no abnormal-exit
        // channel (it is a kernel call, not a `dos_exec`), so this is the
        // normal path and, as in EmuTOS, no `wm_new` runs here.
        return 0;
    }

    // ── Arm 2 — CONSOLE_APP (`:614-622`, `#if WITH_CLI`) ─────────────────
    if nextapp == CONSOLE_APP {
        return run_console_arm();
    }

    // ── Arm 3 — NORMAL_APP / AUTORUN_APP (`:624-689`) ────────────────────
    //
    // `ret = (psh->sh_nextapp == AUTORUN_APP) ? 1 : sh_find(D.s_cmd);` (:629)
    // — an autorun entry is trusted, a normal application is searched for.
    let found = if nextapp == AUTORUN_APP {
        true
    } else {
        let ok = shell::sh_find(&mut cmd);
        if ok {
            // `sh_find` rewrites `D.s_cmd` in place when it qualifies the name.
            shell::set_s_cmd(&cmd);
        }
        ok
    };

    if !found {
        // `set_default_desktop(psh); return EFILNF;` (:687-688)
        kprintln!("[shell] sh_find: '{}' not found", as_str(&cmd));
        shell::set_default_desktop();
        return tos_types::error::EFILNF;
    }

    let fname = shell::sh_name(&cmd[..clen(&cmd)]);
    shell::sh_show(fname); // (:632)
    p_nameit_and_appdir(&cmd); // (:633-635)

    if nextapp == NORMAL_APP {
        // "by default, run the desktop after a normal application" — set
        // *before* the child runs, so a `shel_write` from inside it wins
        // (`:639-643`).
        shell::set_sh_nextapp(DESKTOP_APP);
        shell::set_sh_isgem(true);
    }

    // `ret = dos_exec(PE_LOADGO, D.s_cmd, ad_stail, ad_envrn);` (:645).
    // `ad_stail` is already the Atari command-tail image (length byte first),
    // which is exactly what the loader copies into the child's `p_cmdlin`.
    let mut tail = [0u8; MAXPATHLEN];
    let tlen = shell::s_tail_image(&mut tail);
    let ret = crate::launch::run_program_with_tail_rc(&cmd, &tail[..tlen]);

    // `if (rlr->p_flags & AP_OPEN) { … }` (:650-657) — the program called
    // `appl_init` but never `appl_exit`. Done inside
    // `run_program_with_tail_rc`, at the same point in the sequence EmuTOS
    // runs it (right after `dos_exec` returns), because that is where the
    // child's AES id is known.

    // "if the user ran an autorun application and quitted it, return now to
    // the default desktop" (:662-666). EmuTOS reads `psh->sh_nextapp` **live**
    // here, after the child has run: an autorun program that called
    // `shel_write(SHW_EXEC, …)` on its way out has overwritten it, and its
    // choice must win over the return-to-desktop default.
    if shell::sh_nextapp() == AUTORUN_APP {
        shell::set_default_desktop();
    }

    // `if (gl_shgem) wm_new();` (:676-677) — general cleanup after a
    // graphics-mode program; a character-mode one has no windows to sweep.
    if shell::gl_shgem() {
        tos_api::aes::window::wm_new();
    }

    kprintln!("[shell] sh_ldapp: '{}' exited with rc={}", as_str(&cmd), ret);

    if ret < 0 {
        ret
    } else {
        0
    }
}

/// The `CONSOLE_APP` arm body (`gemshlib.c:614-622`):
/// `aes_run_rom_program(coma_start); set_default_desktop(psh); return 0;`
///
/// Note EmuTOS does **not** call `p_nameit`/`p_setappdir` here — EmuCON runs
/// under whatever name the previous occupant left behind
/// (`docs/api/aes/shell-loop.md` §5).
fn run_console_arm() -> i32 {
    crate::shell::run_console();
    shell::set_default_desktop();
    0
}

/// Run the console a `shel_write(SHW_EXEC, 0, 1, DEF_CONSOLE, …)` has made
/// pending, **nested** in the caller, and come back with the screen in
/// graphics mode again. Returns `false` — without doing anything — when the
/// pending program is not the console.
///
/// This is one pass of `sh_main`'s body (`gemshlib.c:710-727`) for the
/// [`CONSOLE_APP`] case:
///
/// ```c
/// sh_chdef(psh);          /* :712 — CONSOLE_APP: set_drvdir(psh->sh_cdir) */
/// sh_chgrf(psh);          /* :713 — sh_isgem == 0 → sh_toalpha()          */
/// rc = sh_ldapp(psh);     /* :725 — the CONSOLE_APP arm, :614-622         */
/// /* next pass */
/// sh_chdef(psh);          /* :712 — DESKTOP_APP after set_default_desktop */
/// sh_chgrf(psh);          /* :713 — sh_isgem == 1 → sh_tographic()        */
/// ```
///
/// **Why it exists (AES gap G1).** In EmuTOS the desktop does not need this:
/// `do_filemenu(CLIITEM)` returns TRUE (`deskmain.c:753`), `deskmain`'s loop
/// ends, the desktop **process terminates**, and the loop above runs in
/// `sh_main` where it belongs. NewTOS's built-in desktop is an in-kernel call
/// ([`crate::desk::run`], `sh_ldapp`'s DESKTOP arm) that cannot terminate and
/// be started again: its state singleton is built once
/// ([`crate::desk::global::init`] is idempotent) and its window list has no
/// `win_start()` re-entry, so returning here would hand the restarted desktop
/// a window list full of handles the intervening `wm_init()` had already
/// deleted. Until that restart path exists (`TASK/DESK14.md` §4, the same
/// decision the `Read .INF file` item waits on), the desktop calls this
/// instead of returning, and its event loop resumes afterwards.
///
/// **The two steps of the pass that are deliberately left out**, and why:
/// `wm_init()` and `sh_draw(D.s_cmd, TRUE)` (`:717-719`). Both prepare the
/// screen and the AES window table for a desktop that is about to be built
/// from scratch — `wm_init` deletes every window, and the restarted
/// `deskmain` reopens them from `cnx_get` (`deskmain.c:2055`). The desktop
/// that called this one is still standing with its windows registered, so
/// taking either would destroy exactly the state it is about to draw. That is
/// gap G2, named the same way on the GEM-app launch path
/// (`crate::desk::main::desktop_resume`). `ratinit()` (`:718`) is not skipped:
/// it is inside `sh_tographic`, which does run.
pub fn sh_run_pending_console() -> bool {
    if shell::sh_nextapp() != CONSOLE_APP {
        return false;
    }

    shell::sh_chdef(); // :712 — the directory `shel_wdef` named
    sh_chgrf(); // :713 — into alpha mode

    run_console_arm(); // :725 → sh_ldapp's CONSOLE_APP arm (:614-622)

    // The next pass's first two statements: `set_default_desktop` (run by the
    // arm above, `:619`) has put DESKTOP_APP/`sh_isgem = TRUE` back, so these
    // restore the AES's start directory and the graphics screen.
    shell::sh_chdef(); // :712
    sh_chgrf(); // :713 — back into graphics mode
    true
}

/// `p_setappdir(rlr, D.s_cmd)` (`gempd.c:90-107`) — record the directory the
/// program was started from, which `sh_find` step (1) searches first.
fn p_setappdir(cmd: &[u8]) {
    tos_api::aes::resource::set_app_dir(cmd);
}

/// `p_nameit(rlr, fname); p_setappdir(rlr, D.s_cmd); rlr->p_flags = 0;`
/// (`gemshlib.c:633-636`).
///
/// All three run, split over two places by *when* each is needed. The
/// app-directory half is set here because `sh_find` searches it before the
/// launch; `p_nameit` **and** `rlr->p_flags = 0` are applied together by
/// [`crate::launch::run_program_with_tail_rc`] at the moment the child gets its
/// process slot ([`tos_api::aes::register_process`] →
/// `tos_api::aes::pd::register`), because NewTOS's AES process descriptors are
/// keyed on that slot (A62: one entry per GEMDOS slot plus one for this
/// kernel-context desktop, `tos_api::aes::pd::DESKTOP`), so the descriptor to
/// rename and clear is only identified there. Both launch paths go through it:
/// this shell loop's NORMAL/AUTORUN arm and the desktop's double-click
/// ([`crate::desk`]).
///
/// Item 10 of the plan's gap list ("AESPD numbering + `p_nameit`/`p_setappdir`
/// on `rlr`") is closed: like EmuTOS's `AESPD`s (`aes/gempd.c:32-35`),
/// NewTOS's exist per index from boot, `appl_init` cannot fail, and the
/// descriptor a program is renamed into is the one its slot has always had
/// (the *identity*, A62); and the **reported value** is EmuTOS's — 0 for the
/// whole application chain, whichever slot (`tos_api::aes::appl::reported_pid`,
/// A56, spec § "AES process identity and pid", contract (b)). The behaviour
/// EmuTOS gets from its model at
/// this boundary — a program never starting on the previous occupant's flags
/// or messages — is in place (A28): the flag reset in `pd::register`, the queue
/// drain in the desktop's own `appl_exit()` (`deskmain.c:2166`,
/// `crate::desk::main::desktop_relinquish`), and since A62 the slot's release
/// (`pterm::release_slot` → `pd::release`) for every way a process ends.
fn p_nameit_and_appdir(cmd: &[u8]) {
    p_setappdir(cmd);
}

/// Say what was tried and why the desktop is not there, on the console the
/// user is about to land in. With no raster backend the framebuffer console
/// is inert, so this goes to the same serial device the console REPL's own
/// `Cconout` output reaches — the difference between a fallback and a mystery.
fn report_no_desktop() {
    let mut desk = [0u8; MAXPATHLEN];
    shell::s_cmd(&mut desk);
    kprintln!(
        "[shell] no screen: the built-in desktop ('{}') cannot be started \
         — falling back to the console (EmuCON)",
        as_str(&desk)
    );
}

/// Length of a NUL-terminated byte string.
fn clen(s: &[u8]) -> usize {
    s.iter().position(|&b| b == 0).unwrap_or(s.len())
}

/// A NUL-terminated byte string as `&str` for logging (`"?"` if not UTF-8).
fn as_str(s: &[u8]) -> &str {
    core::str::from_utf8(&s[..clen(s)]).unwrap_or("?")
}
