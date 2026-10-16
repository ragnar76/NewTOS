//! Kernel-side program launcher — the EmuTOS shell-launch primitive.
//!
//! Loads an external program (registry **or** FAT16 disk) into a process
//! slot, drops to EL0, runs it, and resumes here when it `Pterm`s. This is
//! how the desktop / AUTO folder launch `.PRG` programs — the `NORMAL_APP`
//! / autorun cases of EmuTOS' shell loop.
//!
//! It follows the proven `selftest_pexec` path: `pexec::load` →
//! `enter_el0_with_resume_capture`. Crucially it is **not** a raw `svc`
//! Pexec: the GEMDOS Pexec trap reads its `path` argument as a *user*
//! pointer (`ldtrb`), which faults on a kernel pointer; calling
//! `pexec::load` directly reads the path with `read_file` (an ordinary
//! kernel function), so the caller may pass a kernel string.

use core::sync::atomic::{AtomicI32, AtomicUsize, Ordering};

use tos_api::bdos::process::{self, set_current, slot, State, PROCESS_TABLE};

use crate::kprintln;

/// Load and run the program named `path` (NUL-terminated; a registry key or
/// a FAT16 path like `b"C:\\HELLO.PRG\0"`). Drops to EL0, runs it, and
/// returns when it `Pterm`s. Returns `true` if it launched, `false` on a
/// load/launch failure.
///
/// The desktop / boot caller repaints the screen afterwards (the program
/// owned it while running).
/// Set the mouse cursor to the busy hourglass (`on`) or back to the arrow,
/// so the desktop shows the wait cursor while a program loads from disk
/// (EmuTOS `set_mouse_to_hourglass`). No-op when there is no display.
fn busy_cursor(on: bool) {
    if let Some(b) = tos_api::host::host().and_then(|h| h.raster()) {
        // 2 = HOURGLASS, 0 = ARROW (AES preset shape numbers).
        tos_api::vdi::mouse::set_preset(if on { 2 } else { 0 }, b);
    }
}

// ── The kernel-launched child: its slot and its exit code ───────────────────

/// "No kernel-launched child is running" in [`SHELL_CHILD_SLOT`].
const NO_CHILD: usize = usize::MAX;

/// The process slot of the program [`run_program_with_tail_rc`] started and
/// that has not terminated yet, or [`NO_CHILD`].
///
/// The launcher is the one place that starts a program on behalf of the
/// kernel: the shell loop's `sh_ldapp` (EmuTOS `dos_exec(PE_LOADGO, …)`,
/// `aes/gemshlib.c:645`) and, before the AES, the `\AUTO\` runner. Published
/// right before the drop to EL0 and withdrawn as soon as the program is back,
/// so while it holds a slot, that slot *is* the shell's child — the program
/// identity the kernel's "desktop up" observation keys on
/// ([`crate::shell_loop::observe_desktop_gem_call`], DESK43 U9-0c) and that a
/// measurement injector can arm on (T10, `tools/inject-arm.py`). Programs the
/// child `Pexec`s, and desk accessories, have other slots.
static SHELL_CHILD_SLOT: AtomicUsize = AtomicUsize::new(NO_CHILD);

/// The slot of the program the kernel's launcher started and that is still
/// running — see [`SHELL_CHILD_SLOT`]. `None` while no such program runs.
pub fn shell_child_slot() -> Option<usize> {
    match SHELL_CHILD_SLOT.load(Ordering::Acquire) {
        NO_CHILD => None,
        slot => Some(slot),
    }
}

/// The exit code of the last program that terminated back into a **kernel**
/// resume point (written by the trap handler's terminate tail,
/// [`record_kernel_child_exit`]), read by [`run_program_with_tail_rc`] after
/// the resume.
static KERNEL_CHILD_RC: AtomicI32 = AtomicI32::new(0);

/// Record the exit code of a program whose "parent" is a kernel function — the
/// launcher here, or a boot-time selftest — at the moment its termination
/// resumes that function (`finish_terminate`'s kernel-launch branch in
/// `arch::aarch64::exceptions`). There is no `Pexec` caller whose `x0` could
/// carry it, so it is left here for the resumed function to read.
///
/// The value is stored as EmuTOS's parent sees it
/// ([`process::pterm::parent_exit_code`]: `xterm(UWORD rc)`, `bdos/proc.c:601,
/// 616`, the low 16 bits zero-extended) — `Pterm(-1)` arrives as 65535, the
/// console Ctrl-C's `-32` as 65504. The `Pterm` paths hand over `rc & 0xFFFF`
/// already (D-005); the mask makes the other terminate paths (Ctrl-C, upcall
/// nesting overflow) read the same way, so a child's exit code is never
/// negative here and never mistaken for a load error (PROC05).
pub(crate) fn record_kernel_child_exit(rc: i32) {
    KERNEL_CHILD_RC.store(process::pterm::parent_exit_code(rc), Ordering::Release);
}

/// Load and run the program named `path` (NUL-terminated; a registry key or a
/// FAT16 path like `b"C:\\HELLO.PRG\0"`) with **no command tail**. Drops to EL0,
/// runs it, and returns when it `Pterm`s. Returns `true` if it ran (whatever
/// its exit code), `false` on a load/launch failure. The desktop / boot caller
/// repaints the screen afterwards (the program owned it while running).
pub fn run_program(path: &[u8]) -> bool {
    run_program_with_tail(path, &[])
}

/// Load and run `path` like [`run_program`], but hand the child a **command
/// tail** (`p_cmdlin`). `tail` is the Atari command-tail image copied verbatim
/// into the basepage: `tail[0]` is the length byte, `tail[1..]` the argument
/// characters (the loader appends the terminating NUL). This is the mechanism
/// behind the desktop's document→application association (`deskins`/`do_aopen`):
/// the associated application receives the document path as its command tail.
///
/// `true` means "the program ran" — [`run_program_with_tail_rc`] returned its
/// exit code (`0 … 65535`) — and `false` a load failure (a negative code).
pub fn run_program_with_tail(path: &[u8], tail: &[u8]) -> bool {
    run_program_with_tail_rc(path, tail) >= tos_types::error::E_OK
}

/// [`run_program_with_tail`] reporting the **GEMDOS return code** rather than a
/// bool — EmuTOS's `dos_exec(PE_LOADGO, D.s_cmd, ad_stail, ad_envrn)`
/// (`aes/gemshlib.c:645`), whose negative result `sh_ldapp` returns to
/// `sh_main`, which turns it into an `fm_error` alert (`:722-723`).
///
/// Returns what EmuTOS's `dos_exec` returns (`docs/api/aes/shell-loop.md` §8,
/// "the child's exit code", PROC05):
///
/// * the program ran: **its exit code**, the `UWORD` of its `Pterm`
///   zero-extended, `0 … 65535` (`bdos/proc.c:601,616`) — `Pterm(-1)` (a
///   crashed program, a Rust panic) is `65535`, never negative;
/// * it could not be started: the load failure's negative GEMDOS code
///   ([`tos_api::bdos::process::pexec::PexecError::errno`], e.g. `EFILNF` for
///   a missing file, `EPLFMT` for a non-ELF, `ENSMEM` when the TPA cannot be
///   allocated).
pub fn run_program_with_tail_rc(path: &[u8], tail: &[u8]) -> i32 {
    launch(path, path, tail, Launch::Program)
}

/// Start the desktop **program** — the shell loop's DESKTOP arm, EmuTOS's
/// `aes_run_rom_program(deskstart)` (`aes/gemshlib.c:570-580,606`; DESK43 U9-4).
///
/// `name` is the registry name of the embedded desktop (`DEF_DESKTOP`) — what
/// EmuTOS's `deskstart` entry point is: the desktop the image carries, found
/// without a file. `cmd` is `D.s_cmd`, from which the arm's `p_nameit(rlr,
/// fname)` (`:604`) names the AES descriptor.
///
/// It is [`run_program_with_tail_rc`] with what EmuTOS's ROM-program start does
/// not do left out:
///
/// * **no hourglass** — the DESKTOP arm has no `set_mouse_to_hourglass`
///   (`gemshlib.c:600-612`); the pointer keeps the shape it has;
/// * **no `D.s_cmd`/`ad_stail` rewrite** — `aes_run_rom_program` creates the
///   basepage with an **empty** tail (`Pexec(PE_BASEPAGEFLAGS, PF_STANDARD,
///   "", NULL)`, `:575`) and leaves the shell's command pair alone, so
///   `shel_read` answers during the desktop what it answered before;
/// * no `[acc] launching …` report (it describes an application start after
///   the previous one's `appl_exit`).
///
/// Returns what [`run_program_with_tail_rc`] returns: the desktop's exit code
/// `0 … 65535`, or a negative load error (the desktop never ran).
pub fn run_desktop_program(name: &[u8], cmd: &[u8]) -> i32 {
    launch(name, cmd, &[], Launch::Desktop)
}

/// What kind of start [`launch`] performs — see [`run_desktop_program`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Launch {
    /// An application: `dos_exec(PE_LOADGO, D.s_cmd, ad_stail, ad_envrn)`
    /// (`gemshlib.c:645`) or an `\AUTO\` program.
    Program,
    /// The desktop: `aes_run_rom_program(deskstart)` (`gemshlib.c:606`).
    Desktop,
}

/// Load `path` (a registry key or a FAT path), run it at EL0 to its end and
/// return its exit code or the negative load error. `name` names the AES
/// descriptor (`p_nameit`); for an application it is `path` itself.
fn launch(path: &[u8], name: &[u8], tail: &[u8], kind: Launch) -> i32 {
    let program = kind == Launch::Program;
    // Busy cursor while loading from disk (the desktop is still on screen).
    if program {
        busy_cursor(true);
    }
    // Load from the registry or the FAT16 file system into a fresh slot,
    // TPA and environment owned by the child (the shell's `PE_LOADGO`: its
    // `Pterm` frees them). The environment is `run->p_env`'s copy, as EmuTOS
    // gives it (`alloc_env`, `bdos/proc.c:413-432`): the AUTO runner passes
    // `NULL` while `run` is `initial_basepage` (`bios/bios.c:693`) — `{0, 0}`;
    // `sh_ldapp` passes `ad_envrn`, the AES PD's `p_env` (`aes/gemshlib.c:645`,
    // `aes/gemstart.S:78-96`). Both are `EnvSpec::Inherit` with no running
    // process: the kernel's stand-in `run` (TASK/PPARENT,
    // `tos_api::bdos::process::standin`). The command `tail` is copied into
    // the child's `p_cmdlin` (offset 0x80) by the loader.
    // SAFETY: single-CPU boot/desktop context; the BDOS is not concurrently
    // locked, and the returned `LoadedChild` borrows nothing past this call.
    let child = match unsafe {
        process::pexec::load(path, tail, process::loader::EnvSpec::Inherit, false, true)
    } {
        Ok(c) => c,
        Err(e) => {
            kprintln!("[launch] load failed: {:?}", e);
            if program {
                busy_cursor(false);
            }
            return e.errno();
        }
    };
    if program {
        busy_cursor(false);
    }

    // Desk accessories: by this point the caller's `appl_exit` has posted
    // `AC_CLOSE` to every registered one and blocked until each had seen it
    // (EmuTOS `ap_exit` → `mn_cleanup` + `wait_for_accs(AP_ACCLOSE)`,
    // `gemaplib.c:253-255`). Report the state on the serial console, because
    // an accessory that does nothing with the message — the correct behaviour
    // for one with no state tied to the dying application — leaves no other
    // trace that it arrived.
    let accs = tos_api::aes::acc::count();
    if program && accs > 0 {
        kprintln!(
            "[acc] launching with {} accessory(s); AC_CLOSE seen by all: {}",
            accs,
            tos_api::aes::acc::all_have(tos_api::aes::acc::AP_ACCLOSE)
        );
    }

    let s = match slot(child.slot) {
        Some(s) => s,
        None => return tos_types::error::ENSMEM,
    };
    s.set_state(State::Running);
    // Record the program's own directory so a GEM app's `rsrc_load` finds its
    // `.RSC` beside the executable (lead directive 2026-07-20): QED passes a
    // bare `qed.rsc`, and its resource is staged next to `QED.NAP`. The
    // DESKTOP arm has done its `p_setappdir(rlr, D.s_cmd)` (`gemshlib.c:605`)
    // already.
    if program {
        tos_api::aes::resource::set_app_dir(path);
    }
    // Name the launched program's AES process descriptor — the slot's entry
    // (A62) — from its launch path (EmuTOS `pstart` → `p_nameit`,
    // `aes/gempd.c:110-131`), so a later `appl_find(name)` resolves it; the
    // desktop from `D.s_cmd` (`gemshlib.c:604`).
    tos_api::aes::register_process(child.slot, name);
    // Publish the start command + command-tail image as `D.s_cmd`/`ad_stail`
    // — the one pair `shel_read` answers from and `shel_write(SHW_EXEC)`
    // overwrites (`aes/gemshlib.c:104-105,169-170`; H05). In EmuTOS the
    // desktop's `shel_write` has filled them by the time `sh_ldapp` reaches
    // `dos_exec(PE_LOADGO, D.s_cmd, ad_stail, …)` (`:645`), and since DESK43 U3
    // the NewTOS desktop takes that route too — for the shell loop's own arm
    // this therefore rewrites the identical values. The path that has no
    // `shel_write` in front of it is the `\AUTO\` runner (`run_auto_programs`,
    // EmuTOS `bios/bios.c:713-714`, which precedes the AES entirely), so the
    // launcher publishes them here on every path, one writer for both. `tail`
    // is handed over verbatim: it is the length-prefixed `p_cmdlin` image the
    // loader copied. Not for the desktop (see [`run_desktop_program`]).
    if program {
        tos_api::aes::shell::set_launch_info(path, tail);
    }
    // `proc_go`: `p->p_parent = run` (`bdos/proc.c:541`) — `run` is kernel
    // code here, so the stand-in PD of this moment: `initial_basepage` for an
    // AUTO program (`bios/bios.c:693`), the AES PD for the desktop's and the
    // `#Z` program (`aes/gemshlib.c:645`; TASK/PPARENT).
    // SAFETY: single-CPU kernel context; the slot holds the loaded image.
    unsafe { process::pexec::set_kernel_parent(child.slot) };
    set_current(process::pd_of(child.slot));
    // The child's identity, for the kernel's observers (DESK43 U9-0c).
    SHELL_CHILD_SLOT.store(child.slot, Ordering::Release);
    kprintln!("[launch] EL0: slot {} entry 0x{:x}", child.slot, child.entry_va);

    // SAFETY: single-CPU; `parent_frame` captures this kernel resume point,
    // and the program's `Pterm` restores it via the handler's SPSR.M = EL1t
    // (kernel-launched) branch — exactly as `selftest_pexec` does. The
    // initial `SP_EL0` is the child's TPA top (`child.sp_el0` = `p_hitpa`; the
    // TPA is the largest free block, EmuTOS `alloc_tpa`); the SDK start-up
    // code moves the stack into the part of the TPA it keeps when it shrinks
    // the block (mintlib `_crtinit`).
    unsafe {
        crate::arch::aarch64::user_entry::enter_el0_with_resume_capture(
            child.entry_va,
            child.basepage_va,
            child.l0_pa,
            child.sp_el0,
            &PROCESS_TABLE[child.slot].parent_frame,
        );
    }

    // Back: the terminate tail has released the slot and left the child's
    // exit code in `KERNEL_CHILD_RC` (`record_kernel_child_exit`).
    SHELL_CHILD_SLOT.store(NO_CHILD, Ordering::Release);
    let rc = KERNEL_CHILD_RC.load(Ordering::Acquire);
    kprintln!("[launch] returned (program Pterm'd)");

    // `sh_ldapp`'s `AP_OPEN` cleanup (`aes/gemshlib.c:650-657`), at exactly
    // the point EmuTOS runs it — immediately after `dos_exec(PE_LOADGO, …)`
    // returns. If the program called `appl_init()` without a matching
    // `appl_exit()` (a crashed or sloppy GEM app), do the important parts for
    // it, so the AES descriptor the next program reuses is clean. The slot's
    // release (`pterm::release_slot`) has reset the descriptor already; this
    // is the `sh_ldapp` step, addressed by the slot the descriptor is keyed by.
    tos_api::aes::close_client_if_open(child.slot);

    rc
}

/// `MODE_CTRL` (`references/emutos/include/biosdefs.h:92`) — the Control bit of
/// the `Kbshift` modifier byte.
const MODE_CTRL: u8 = 0x04;

/// EmuTOS' `bootflags` (`references/emutos/bios/bios.c:104`), reduced to the
/// one flag NewTOS has a consumer for: `BOOTFLAG_SKIP_AUTO_ACC`
/// (`include/biosext.h:42`), read by [`run_auto_folder`] below exactly as
/// EmuTOS' `autoexec` reads it (`bios/bios.c:702-704`).
///
/// `BOOTFLAG_SKIP_HDD_BOOT` and `BOOTFLAG_EARLY_CLI` are not represented — see
/// the hint-line comment in [`crate::initinfo::run`] for why neither has a
/// consumer here.
static SKIP_AUTO_ACC: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Turn the modifier keys held on the boot screen into boot flags — EmuTOS
/// `biosmain` (`references/emutos/bios/bios.c:816-820`):
///
/// ```c
/// if (shiftbits & MODE_ALT)  bootflags |= BOOTFLAG_SKIP_HDD_BOOT;
/// if (shiftbits & MODE_CTRL) bootflags |= BOOTFLAG_SKIP_AUTO_ACC;
/// ```
///
/// `shiftbits` is what [`crate::initinfo::run`] returned. Only the Control arm
/// is reproduced (see [`SKIP_AUTO_ACC`]).
///
/// EmuTOS' `BOOTFLAG_SKIP_AUTO_ACC` is broader than the `\AUTO\` folder: it
/// also suppresses loading desk accessories (`aes/geminit.c:268-270`), user
/// cursors (`geminit.c:529-532`), user icons (`desk/deskapp.c:541-543`) and
/// even the `EMUDESK.INF` read itself (`geminit.c:766-769`). NewTOS honours the
/// `\AUTO\` part and, since ACC02, the **desk-accessory** part
/// ([`skip_auto_acc`] → `crate::accs::load_accs`). Loadable cursors/icons are
/// not implemented at all, and our `.INF` read happens in `aes::init::init()`
/// *before* the boot screen is shown, so the flag does not exist yet at that
/// point. Reordering the boot to fix that is a separate change.
pub fn set_bootflags_from_shiftbits(shiftbits: u8) {
    SKIP_AUTO_ACC.store(shiftbits & MODE_CTRL != 0, Ordering::Relaxed);
}

/// `bootflags & BOOTFLAG_SKIP_AUTO_ACC` — read by [`run_auto_folder`] here and
/// by `count_accs` (`crate::accs`, EmuTOS `aes/geminit.c:269-270`), the two
/// consumers EmuTOS has for it that NewTOS also has.
pub fn skip_auto_acc() -> bool {
    SKIP_AUTO_ACC.load(Ordering::Relaxed)
}

/// The drive and directory an `\AUTO\` program runs in — the **root of the
/// boot drive** (DESK28).
///
/// EmuTOS' `autoexec` sets neither: `run_auto_program` builds the full path
/// `"\AUTO\<name>"` and hands it straight to `Pexec(PE_LOADGO, …)`
/// (`bios/bios.c:685-695`), with no `Dsetdrv`/`Dsetpath` anywhere in
/// `autoexec` (`:697-733`). The AUTO child's working directory is therefore
/// purely *inherited*: `Pexec` copies the current drive and the per-drive
/// current-directory table out of the calling process into the child's
/// basepage (`init_pd_files`, `bdos/proc.c:400-410`), and the caller here is
/// the BIOS, whose state is `Dsetdrv(bootdev)` (`bios/bios.c:827`) plus, for
/// every drive it never `Dsetpath`ed, the **root** DND that `ckdrv` installs
/// on first use (`bdos/fsdrive.c:119-131`). An AUTO program that opens
/// `FOO.TXT` by a bare name sees `<bootdrive>:\FOO.TXT`, not `\AUTO\FOO.TXT`.
///
/// NewTOS has no per-process copy to inherit from: the current drive and the
/// per-drive current directories are single globals, the documented stand-in
/// for `p_curdrv`/`p_curdir` (`tos-api/src/bdos/fs/drive.rs`,
/// `…/fs/path.rs`). Left alone, the first AUTO program's `Dsetpath` would be
/// seen by the second and by the desktop after them — which on EmuTOS cannot
/// happen, because each child mutates only its own copy. Re-establishing the
/// BIOS's state here, before every program, is what makes the global behave
/// like the copy: each `\AUTO\` program starts on the boot drive, at its
/// root, whatever the one before it did.
fn set_auto_default_path() {
    // `Dsetdrv(bootdev)` (`bios/bios.c:827`) — the BIOS's current drive, which
    // is what every AUTO child inherits as `p_curdrv`.
    tos_api::bdos::fs::drive::set_boot_drive();
    // …and the root on every drive: the BIOS process never `Dsetpath`ed, so
    // `ckdrv` hands each of its drives the root DND (`bdos/fsdrive.c:119-131`).
    // The empty string is this table's spelling of "root"
    // (`tos-api/src/bdos/fs/path.rs`, `curdir_str`).
    for drive in 0..16 {
        tos_api::bdos::fs::path::set_curdir(drive, "");
    }
}

/// Run every auto-run program in `\AUTO\` on the boot drive (C:) in directory
/// order, before the desktop — EmuTOS' `autoexec` (`bios/bios.c:697`). Each
/// runs to its `Pterm`, then the next. AUTO programs are typically drivers /
/// TSRs that set something up at boot. Absent `\AUTO\` folder → no-op.
///
/// Each program starts on the boot drive at its root — see
/// [`set_auto_default_path`] for why that is the inherited state and not
/// `\AUTO\`.
///
/// Skipped entirely when the user held Control on the boot screen — EmuTOS'
/// first act in `autoexec` (`bios/bios.c:702-704`, "check if the user does not
/// want to run AUTO programs"); see [`set_bootflags_from_shiftbits`].
///
/// Runs **only** native NewTOS *console* programs, `.NCP` (ADR-018), and the
/// filter is the **search pattern**, exactly as EmuTOS's is:
/// `Fsfirst("\\AUTO\\*.PRG", 7)` (`bios/bios.c:713-714`) states the one
/// extension the AUTO folder runs and nothing downstream re-decides it. Ours
/// says `C:\AUTO\*.NCP` for the same reason and in the same place — the boot
/// code owns this test; it does not ask the desktop.
///
/// Why `.NCP` is the one: AUTO runs **before** the AES, so a GEM application
/// (`.NAP`) — which needs `menu_bar`/`objc_draw`/`gl_wchar` — must not start
/// here; it launches from the desktop instead (double-click, or the `#Z`
/// NEWDESK.INF autorun). Classic Atari `.PRG`/`.TOS` are m68k and cannot load
/// without the (unavailable) emulator — attempting one at boot only produced a
/// failed load and an alert (the desktop still reports them as m68k when a
/// user explicitly opens one). `.NTP` is excluded too: it is a console program
/// that takes a command tail, and there is no dialog to ask for one at boot
/// (`desk/ftype.rs`'s `Flavor::Params`, DESK44).
pub fn run_auto_folder() {
    use tos_api::bdos::fs::find::{list_dir_ex, DirInfo};

    // `if (bootflags & BOOTFLAG_SKIP_AUTO_ACC) return;` (`bios/bios.c:702-704`)
    if SKIP_AUTO_ACC.load(Ordering::Relaxed) {
        kprintln!("[auto] skipped (Control held on the boot screen)");
        return;
    }

    // `err = Fsfirst("\\AUTO\\*.PRG", 7)` (`bios/bios.c:713-714`) — the
    // extension is the pattern; see the function doc.
    let mut ents = [DirInfo::default(); 32];
    let n = list_dir_ex("C:\\AUTO\\*.NCP", &mut ents);
    let mut ran = 0;
    for e in &ents[..n] {
        // EmuTOS's search attribute is `7` (`FA_RO|FA_HIDDEN|FA_SYSTEM`), which
        // excludes directories (`bdos/fsdir.c:1840-1844`); `list_dir_ex`
        // reports them, so this test is that attribute's stand-in.
        if e.attrib & 0x10 != 0 {
            continue; // skip "." / ".." / sub-directories
        }
        let end = e.name.iter().position(|&b| b == 0).unwrap_or(14);
        let name = &e.name[..end];
        // Build "C:\AUTO\<name>\0".
        let mut path = [0u8; 40];
        let prefix = b"C:\\AUTO\\";
        path[..prefix.len()].copy_from_slice(prefix);
        path[prefix.len()..prefix.len() + end].copy_from_slice(name);
        path[prefix.len() + end] = 0;
        kprintln!(
            "[auto] running C:\\AUTO\\{}",
            core::str::from_utf8(name).unwrap_or("?")
        );
        // The working directory EmuTOS' AUTO child inherits, re-established
        // for every program because our drive/directory state is global
        // (DESK28, see [`set_auto_default_path`]).
        set_auto_default_path();
        run_program(&path);
        ran += 1;
    }
    if ran > 0 {
        // Whatever the last program did to the global drive/directory, the
        // desktop that follows must see the state EmuTOS' BIOS process still
        // has after `autoexec` returns — its own, untouched by any child.
        set_auto_default_path();
        kprintln!("[auto] {} AUTO program(s) run", ran);
    }
}
