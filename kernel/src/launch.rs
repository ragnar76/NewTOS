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

use core::sync::atomic::Ordering;

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

/// Load and run the program named `path` (NUL-terminated; a registry key or a
/// FAT16 path like `b"C:\\HELLO.PRG\0"`) with **no command tail**. Drops to EL0,
/// runs it, and returns when it `Pterm`s. Returns `true` if it launched, `false`
/// on a load/launch failure. The desktop / boot caller repaints the screen
/// afterwards (the program owned it while running).
pub fn run_program(path: &[u8]) -> bool {
    run_program_with_tail(path, &[])
}

/// Load and run `path` like [`run_program`], but hand the child a **command
/// tail** (`p_cmdlin`). `tail` is the Atari command-tail image copied verbatim
/// into the basepage: `tail[0]` is the length byte, `tail[1..]` the argument
/// characters (the loader appends the terminating NUL). This is the mechanism
/// behind the desktop's document→application association (`deskins`/`do_aopen`):
/// the associated application receives the document path as its command tail.
pub fn run_program_with_tail(path: &[u8], tail: &[u8]) -> bool {
    run_program_with_tail_rc(path, tail) == tos_types::error::E_OK
}

/// [`run_program_with_tail`] reporting the **GEMDOS return code** rather than a
/// bool — EmuTOS's `dos_exec(PE_LOADGO, D.s_cmd, ad_stail, ad_envrn)`
/// (`aes/gemshlib.c:645`), whose negative result `sh_ldapp` returns to
/// `sh_main`, which turns it into an `fm_error` alert (`:722-723`).
///
/// Returns `E_OK` (0) when the program ran to its `Pterm`, otherwise the load
/// failure's GEMDOS code ([`tos_api::bdos::process::pexec::PexecError::errno`],
/// e.g. `EFILNF` for a missing file, `EPLFMT` for a non-ELF, `ENSMEM` when the
/// TPA cannot be allocated).
pub fn run_program_with_tail_rc(path: &[u8], tail: &[u8]) -> i32 {
    // Busy cursor while loading from disk (the desktop is still on screen).
    busy_cursor(true);
    // Load from the registry or the FAT16 file system into a fresh slot.
    // No environment (`EnvSpec::None` → `p_env = 0`) — the desktop/AUTO launcher
    // hands none down, preserving the pre-env-plumbing behaviour on this
    // boot-critical path. Wiring an inherited env here waits on the
    // process-ownership model (Phase 09) so the block is reclaimed at the
    // program's `Pterm`. The command `tail` is copied into the child's
    // `p_cmdlin` (offset 0x80) by the loader.
    // SAFETY: single-CPU boot/desktop context; the BDOS is not concurrently
    // locked, and the returned `LoadedChild` borrows nothing past this call.
    let child = match unsafe {
        process::pexec::load(path, tail, process::loader::EnvSpec::None, false)
    } {
        Ok(c) => c,
        Err(e) => {
            kprintln!("[launch] load failed: {:?}", e);
            busy_cursor(false);
            return e.errno();
        }
    };
    busy_cursor(false);

    let s = match slot(child.slot) {
        Some(s) => s,
        None => return tos_types::error::ENSMEM,
    };
    s.set_state(State::Running);
    // Record the program's own directory so a GEM app's `rsrc_load` finds its
    // `.RSC` beside the executable (lead directive 2026-07-20): QED passes a
    // bare `qed.rsc`, and its resource is staged next to `QED.NAP`.
    tos_api::aes::resource::set_app_dir(path);
    // Name the launched program's AES process descriptor from its launch path
    // (EmuTOS `pstart` → `p_nameit`, `aes/gempd.c:110-131`), so a later
    // `appl_find(name)` resolves it. The program's AES id is `slot + 1`.
    tos_api::aes::register_process(child.slot, path);
    // Publish the start command + command tail for `shel_read` (P6-006): EmuTOS
    // fills `D.s_cmd` (program path) and `ad_stail` (command tail) before
    // `dos_exec` (`aes/gemshlib.c:604-605,645`). `tail` here is the
    // length-prefixed p_cmdlin image (`tail[0]` = length, `tail[1..]` = chars);
    // hand the shell the tail characters (the NUL-terminated round-trip
    // `shel_read`/`shel_write` use — accepted deviation F8).
    let tail_chars: &[u8] = if tail.is_empty() {
        &[]
    } else {
        let len = tail[0] as usize;
        &tail[1..(1 + len).min(tail.len())]
    };
    tos_api::aes::shell::set_launch_info(path, tail_chars);
    set_current(s.basepage_kva.load(Ordering::Acquire));
    kprintln!("[launch] EL0: slot {} entry 0x{:x}", child.slot, child.entry_va);

    // SAFETY: single-CPU; `parent_frame` captures this kernel resume point,
    // and the program's `Pterm` restores it via the handler's SPSR.M = EL1t
    // (kernel-launched) branch — exactly as `selftest_pexec` does. The
    // initial `SP_EL0` is the child's mapped multi-page TPA top
    // (`child.sp_el0`); the SDK crt0 re-derives it from the basepage's
    // p_hitpa, so a compiled program starts on a mapped stack.
    unsafe {
        crate::arch::aarch64::user_entry::enter_el0_with_resume_capture(
            child.entry_va,
            child.l0_pa,
            child.sp_el0,
            &PROCESS_TABLE[child.slot].parent_frame,
        );
    }

    kprintln!("[launch] returned (program Pterm'd)");

    // `sh_ldapp`'s `AP_OPEN` cleanup (`aes/gemshlib.c:650-657`), at exactly
    // the point EmuTOS runs it — immediately after `dos_exec(PE_LOADGO, …)`
    // returns. If the program called `appl_init()` without a matching
    // `appl_exit()` (a crashed or sloppy GEM app), do the important parts for
    // it, so the AES descriptor the next program reuses is clean.
    tos_api::aes::close_client_if_open(tos_api::aes::pid_of_slot(child.slot));

    tos_types::error::E_OK
}

/// Run every auto-run program in `\AUTO\` on the boot drive (C:) in directory
/// order, before the desktop — EmuTOS' `autoexec` (`bios/bios.c:697`). Each
/// runs to its `Pterm`, then the next. AUTO programs are typically drivers /
/// TSRs that set something up at boot. Absent `\AUTO\` folder → no-op.
///
/// Runs **only** the native NewTOS extensions `.NAP` / `.NCP` (ADR-018). Classic
/// Atari `.PRG` / `.TOS` are m68k and cannot load without the (unavailable)
/// emulator — attempting one at boot only produced a failed load and an alert,
/// so they are now skipped here (the desktop still reports them as m68k when a
/// user explicitly opens one). `.NTP` is also excluded: it takes a command tail
/// there is no dialog for at boot.
pub fn run_auto_folder() {
    use crate::desk::ftype::{classify, FileType, Flavor};
    use tos_api::bdos::fs::find::{list_dir_ex, DirInfo};

    let mut ents = [DirInfo::default(); 32];
    let n = list_dir_ex("C:\\AUTO\\*.*", &mut ents);
    let mut ran = 0;
    for e in &ents[..n] {
        if e.attrib & 0x10 != 0 {
            continue; // skip "." / ".." / sub-directories
        }
        let end = e.name.iter().position(|&b| b == 0).unwrap_or(14);
        let name = &e.name[..end];
        // AUTO auto-runs native *console* (.NCP) programs only. It runs at boot,
        // BEFORE the desktop and AES/appl_init are up, so a GEM app (.NAP) — which
        // needs AES (menu_bar/objc_draw/gl_wchar…) — must NOT run here; GEM apps
        // launch from the desktop instead (double-click, or the `#Z` NEWDESK.INF
        // autorun, both after AES init). Classic .PRG/.TOS are non-native m68k, so
        // skip anything that is not a native console program.
        if !matches!(classify(name), FileType::Native(Flavor::Console)) {
            continue;
        }
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
        run_program(&path);
        ran += 1;
    }
    if ran > 0 {
        kprintln!("[auto] {} AUTO program(s) run", ran);
    }
}
