//! `Pexec` (GEMDOS 0x4B) — Phase 06 D3.
//!
//! Spec: [`docs/api/gemdos/process/4b-pexec.md`].
//!
//! Phase 06 D3 implements **mode 0 only** (`PE_LOADGO` = load and
//! launch). The other modes (`PE_LOAD`, `PE_GO`, `PE_BASEPAGE`,
//! `PE_GOTHENFREE`, `PE_BASEPAGEFLAGS`) belong to D4 / D5 and the
//! per-mode dispatch returns [`PexecError::UnsupportedMode`] until
//! they land.
//!
//! ## Pipeline (PE_LOADGO)
//!
//! 1. Resolve `path` via [`super::registry`] → `&'static [u8]` ELF
//!    blob (no filesystem yet — see registry doc).
//! 2. [`super::allocate_slot`] → reserves a fresh process slot for
//!    the child.
//! 3. [`super::loader::load_to_slot`] → parses the ELF, copies
//!    segments, populates the basepage, installs page tables.
//! 4. The kernel SVC handler then snapshots the parent's trap frame
//!    into `slot.parent_frame`, stamps `slot.parent_l0_pa` with
//!    the parent's `TTBR0_EL1`, sets `RUN = child.basepage_kva`,
//!    rewrites the trap frame to point at the child's entry, and
//!    swaps TTBR0 to the child.
//! 5. The post-handler `eret` lands in EL0 at the child's entry.
//!
//! Step 4 is in the kernel because it touches `ExceptionFrame`
//! (kernel-private). This module returns the metadata the handler
//! needs to perform that work.

use core::sync::atomic::Ordering;

use tos_types::BasePage;

use super::loader::{load_file_to_slot, load_to_slot_with, EnvSpec, LoadError, LoadedImage};
use super::registry;
use super::{allocate_slot, free_slot, PROCESS_TABLE};
use crate::bdos::fs::FileImage;

// ─── Mode constants (`include/bdosdefs.h:24-32`) ────────────────────

/// `PE_LOADGO` (0): load executable, basepage owned by child, launch.
pub const PE_LOADGO: u16 = 0;
/// `PE_LOAD` (3): load only, basepage owned by parent, return PD.
pub const PE_LOAD: u16 = 3;
/// `PE_GO` (4): launch already-loaded PD passed in `tail`.
pub const PE_GO: u16 = 4;
/// `PE_BASEPAGE` (5): allocate basepage only, `flags = 0`.
pub const PE_BASEPAGE: u16 = 5;
/// `PE_GOTHENFREE` (6): like `PE_GO`, then re-own basepage to child.
pub const PE_GOTHENFREE: u16 = 6;
/// `PE_BASEPAGEFLAGS` (7): allocate basepage, `flags = (ULONG)path`.
pub const PE_BASEPAGEFLAGS: u16 = 7;

// ─── Result + error types ───────────────────────────────────────────

/// Metadata the kernel SVC handler needs after a `Pexec` dispatch.
/// For a launch mode (`PE_LOADGO` / `PE_GO`) the handler redirects the
/// trap frame to the child and switches `TTBR0_EL1`; for `PE_LOAD` it
/// returns [`Self::handle`] to the parent without launching.
#[derive(Copy, Clone, Debug)]
pub struct LoadedChild {
    /// Slot index in [`PROCESS_TABLE`] holding the child.
    pub slot: usize,
    /// User VA of the child's entry point.
    pub entry_va: u64,
    /// Phys of the child's `L0` table — feeds `TTBR0_EL1`.
    pub l0_pa: u64,
    /// The child's **PD pointer** — the user VA of its
    /// [`tos_types::BasePage`] (the start of its TPA, VA = PA) — passed to
    /// the child entry in `x0` per NewTOS's AArch64 process ABI.
    pub basepage_va: u64,
    /// Initial `SP_EL0` for the child: the TPA top (`p_hitpa`),
    /// 4-KiB / 16-byte aligned. The kernel launch path sets `SP_EL0` to
    /// this before the eret.
    pub sp_el0: u64,
    /// What `PE_LOAD` returns and `PE_GO` takes back (in `tail`): the
    /// child's **PD pointer** (`basepage_va`) — a real basepage
    /// address the caller can read (`p_tlen`, `p_env`, …) in the shared
    /// address space (ARCH03 stage 1), as EmuTOS returns `(long)cur_p`
    /// (`bdos/proc.c:360`).
    pub handle: u64,
    /// `true` for `PE_LOADGO` / `PE_GO` (kernel launches the child);
    /// `false` for `PE_LOAD` (kernel returns `handle` to the parent).
    pub launch: bool,
}

/// Why `Pexec` failed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PexecError {
    /// `mode` is not one of the documented [`PE_LOADGO`] /
    /// [`PE_LOAD`] / etc. constants. Mapped to `EINVFN` (-32).
    UnknownMode,
    /// `mode` is documented but not yet implemented in this phase.
    /// Mapped to `EINVFN`.
    UnsupportedMode,
    /// `path` did not match any registered program.
    /// Mapped to `EFILNF` (-33).
    NotFound,
    /// [`allocate_slot`] returned `None` — process table is full.
    /// Mapped to `ENSMEM` (-39).
    NoSlot,
    /// `PE_GO`'s `tail` is not the PD pointer of a program loaded with
    /// `PE_LOAD` and not yet started. Mapped to `EIMBA` (-40) — FreeMiNT's
    /// `EFAULT` for a foreign or invalid PD (`sys/k_exec.c:378-387`), whose
    /// value is the same −40.
    BadHandle,
    /// The ELF blob was malformed, or its image + BSS overran the
    /// per-process TPA, or the TPA/env pool allocation failed. Mapped
    /// to `EPLFMT` (-66) for parser errors and `ENSMEM` for the
    /// size/allocation shortfalls.
    LoadFailed(LoadError),
}

impl PexecError {
    /// The GEMDOS error code `Pexec` reports for this failure — the value a
    /// caller of `Pexec` (or of the shell loop's `dos_exec`) gets back.
    ///
    /// A *memory/size* shortfall maps to `ENSMEM` (EmuTOS `bdos/proc.c:295-300`:
    /// no TPA → `ENSMEM`); a malformed image / bad header maps to `EPLFMT`
    /// (`bdos/kpgmld.c:47-63`). `ProgramTooLarge` is the image+BSS overrunning
    /// the TPA image region — a size shortfall, so `ENSMEM`. A failed TPA pool
    /// allocation (`NoTpaMemory`) and the env allocation shortfall
    /// (`alloc_env`, `proc.c:284-288`) are likewise `ENSMEM`. A well-formed
    /// but ILP32 image (`Ilp32Unsupported`) is a format NewTOS refuses to
    /// run, so it takes the `EPLFMT` arm too.
    pub const fn errno(self) -> i32 {
        use tos_types::error;
        match self {
            PexecError::UnknownMode | PexecError::UnsupportedMode => error::EINVFN,
            PexecError::NotFound => error::EFILNF,
            PexecError::NoSlot => error::ENSMEM,
            PexecError::BadHandle => error::EIMBA,
            PexecError::LoadFailed(le) => match le {
                LoadError::ProgramTooLarge
                | LoadError::NoEnvMemory
                | LoadError::NoTpaMemory => error::ENSMEM,
                _ => error::EPLFMT,
            },
        }
    }
}

// ─── Mode dispatch ──────────────────────────────────────────────────

/// `Pexec(mode, path, tail, env)` — the GEMDOS-side dispatch.
///
/// `path_bytes` is the registry key / file path (NUL-terminated user
/// string; caller is responsible for the bytes it hands in — Pexec does
/// **not** scan past the slice's `len`).
///
/// The pointer arguments are overloaded by mode. For the **load** modes
/// (`PE_LOADGO` / `PE_LOAD`), `tail` is the command-tail image and `env`
/// the environment source (both materialised by the kernel SVC handler
/// from the caller's address space, since they are user pointers). For
/// **`PE_GO`**, `go_handle` is the opaque PD handle a prior `PE_LOAD`
/// returned (the child's `basepage_pa`); `tail`/`env` are unused.
///
/// # Safety
///
/// Caller (the kernel SVC handler) holds the BDOS giant lock; the
/// caller is the boot CPU. The slot allocated here lives until the
/// child terminates via [`super::pterm::terminate`].
pub unsafe fn dispatch(
    mode: u16,
    path_bytes: &[u8],
    go_handle: u64,
    tail: &[u8],
    env: EnvSpec<'_>,
) -> Result<LoadedChild, PexecError> {
    match mode {
        // PE_LOADGO: load + launch, env+basepage owned by the child.
        // PE_LOAD: load only, basepage owned by the parent, returned as
        // a handle.
        PE_LOADGO => unsafe { load(path_bytes, tail, env, true, true) },
        PE_LOAD => unsafe { load(path_bytes, tail, env, false, false) },
        // PE_GO: launch an already-loaded slot identified by the
        // handle (the basepage_pa a prior PE_LOAD returned).
        PE_GO => go(go_handle),
        PE_BASEPAGE | PE_GOTHENFREE | PE_BASEPAGEFLAGS => {
            Err(PexecError::UnsupportedMode)
        }
        _ => Err(PexecError::UnknownMode),
    }
}

/// Load `path`'s program into a fresh slot. `launch` selects
/// `PE_LOADGO` (true: the kernel's `Pexec` handler transfers control to
/// the child) vs `PE_LOAD` (false: the handler returns the PD pointer to
/// the caller and the slot stays [`State::Loaded`]; the kernel launcher
/// and the accessory loader also pass `false` and start the program
/// themselves).
///
/// `owned_by_child` is EmuTOS' `owner = (flag == PE_LOADGO) ? p : run`
/// (`bdos/proc.c:302-307`) for the TPA and the environment block: `true`
/// for `PE_LOADGO` and for every program the kernel starts itself (its
/// `Pterm` frees them), `false` for `PE_LOAD` (the caller owns them and
/// may `Mfree` them). The `parent_slot` link and `p_parent` are set at
/// **launch** time ([`set_parent`] — EmuTOS `proc_go`, `bdos/proc.c:541`)
/// so `Pterm` returns to whoever launched it.
///
/// `tail` is the command-tail image copied verbatim into the child's
/// `p_cmdlin`; `env` is the environment source (`EnvSpec::Explicit` /
/// `Inherit`, or `None` for callers that hand down neither).
///
/// # Safety
///
/// Caller's BDOS-lock contract; see [`dispatch`].
pub unsafe fn load(
    path_bytes: &[u8],
    tail: &[u8],
    env: EnvSpec<'_>,
    launch: bool,
    owned_by_child: bool,
) -> Result<LoadedChild, PexecError> {
    // Resolve the program source without yet consuming a slot (so a
    // `NotFound` costs nothing): an embedded registry entry first (test
    // programs), otherwise open the FAT16 file for streaming. The file's
    // PT_LOAD segments are read straight into the mapped TPA by the
    // loader — there is no fixed intermediate buffer capping the program
    // size, so a multi-hundred-KiB GEM app loads the same as a tiny one.
    let blob: Option<&[u8]> = registry::lookup(path_bytes);
    let file: Option<FileImage> = match blob {
        Some(_) => None,
        None => {
            let end = path_bytes.iter().position(|&c| c == 0).unwrap_or(path_bytes.len());
            let path =
                core::str::from_utf8(&path_bytes[..end]).map_err(|_| PexecError::NotFound)?;
            Some(FileImage::open(path).ok_or(PexecError::NotFound)?)
        }
    };
    let slot_idx = allocate_slot().ok_or(PexecError::NoSlot)?;

    // SAFETY: slot is freshly claimed; loader populates the slot's
    // basepage / root / entry / stack atomics, the command tail and the
    // environment.
    let loaded: Result<LoadedImage, LoadError> = match (blob, file) {
        (Some(b), _) => unsafe { load_to_slot_with(b, slot_idx, tail, env, owned_by_child) },
        (_, Some(f)) => unsafe { load_file_to_slot(f, slot_idx, tail, env, owned_by_child) },
        // Unreachable: `blob`/`file` resolved above, one is always `Some`.
        (None, None) => Err(LoadError::Parse(super::elf::ParseError::TooShort)),
    };
    let loaded: LoadedImage = match loaded {
        Ok(img) => img,
        Err(e) => {
            // Roll back the slot reservation so the table doesn't
            // leak entries on a malformed image.
            free_slot(slot_idx);
            return Err(PexecError::LoadFailed(e));
        }
    };

    // `PE_LOAD`: remember whose program this is, so the slot goes away with
    // its owner if it is never started ([`release_unstarted_of`]).
    if !launch {
        PROCESS_TABLE[loaded.slot]
            .load_owner
            .store(super::current(), Ordering::Release);
    }

    Ok(LoadedChild {
        slot: loaded.slot,
        entry_va: loaded.entry_va,
        l0_pa: loaded.l0_pa,
        basepage_va: loaded.basepage_va,
        sp_el0: loaded.sp_el0,
        handle: loaded.basepage_va,
        launch,
    })
}

/// Convenience `PE_LOADGO` launcher with no command tail and an
/// inherited environment (the common "run this program" case).
///
/// # Safety
///
/// See [`load`].
pub unsafe fn load_go(path_bytes: &[u8]) -> Result<LoadedChild, PexecError> {
    unsafe { load(path_bytes, &[], EnvSpec::Inherit, true, true) }
}

/// `Pexec(PE_GO, 0, tail, 0)` — launch an already-loaded program. `tail`
/// is the PD pointer a prior `PE_LOAD` returned. Resolve it to its slot and
/// hand the kernel a launch-mode [`LoadedChild`]; ownership is unchanged
/// (EmuTOS `PE_GO` → `proc_go`, `bdos/proc.c:244-248`): the TPA and the
/// environment stay the caller's, and what the child allocates is the
/// child's (`run = p`), freed at its `Pterm` — plan E6.
///
/// The child keeps the root the loader built for it. In stage 1 every root
/// shares the same tables except page 0, so "the caller's address space"
/// (plan E6, FreeMiNT `FORK_SHAREVM`) is the child's too; page 0 stays the
/// child's own so its `os_run` cell names the child and its KEYS02 table is
/// its own (plan E3/E4).
pub fn go(handle: u64) -> Result<LoadedChild, PexecError> {
    let slot_idx = resolve_handle(handle).ok_or(PexecError::BadHandle)?;
    let slot = &PROCESS_TABLE[slot_idx];
    slot.load_owner.store(core::ptr::null_mut(), Ordering::Release);
    Ok(LoadedChild {
        slot: slot_idx,
        entry_va: slot.entry_va.load(Ordering::Acquire),
        l0_pa: slot.l0_pa.load(Ordering::Acquire),
        basepage_va: super::pd_of(slot_idx) as u64,
        sp_el0: slot.sp_el0.load(Ordering::Acquire),
        handle,
        launch: true,
    })
}

/// Find the slot holding a program that was loaded with `PE_LOAD` and not
/// started yet, by its PD pointer. A desk accessory is `Loaded` too until
/// its first dispatch, but it sits in the cooperative run-set from its
/// `pstart` on (`aes::acc::register_process`), so it never matches.
fn resolve_handle(handle: u64) -> Option<usize> {
    if handle == 0 {
        return None;
    }
    (0..PROCESS_TABLE.len()).find(|&i| {
        let p = &PROCESS_TABLE[i];
        p.read_state() == super::State::Loaded
            && !p.runnable.load(Ordering::Acquire)
            && super::pd_of(i) as u64 == handle
    })
}

/// Record the launch linkage of `child` (EmuTOS `proc_go`,
/// `bdos/proc.c:541` — `p->p_parent = run`): the child's `p_parent`
/// becomes the caller's PD pointer, or `0` when the caller is no process
/// (the kernel launcher, an EL1 caller — as before stage 1).
///
/// # Safety
///
/// Caller holds the BDOS giant lock; `child` holds a loaded image.
pub unsafe fn set_parent(child: usize, parent_pd: *mut BasePage) {
    let kva = PROCESS_TABLE[child].basepage_kva.load(Ordering::Acquire);
    if kva.is_null() {
        return;
    }
    // SAFETY: `kva` is the kernel alias of the child's live basepage;
    // `p_parent` is a plain 32-bit LONG (pool addresses are below 2 GiB).
    unsafe { (*kva).p_parent = parent_pd as u64 as tos_types::Long };
}

/// Release every slot holding a program that `owner` loaded with `PE_LOAD`
/// and never started — called when `owner` terminates. Its TPA and
/// environment were `owner`'s blocks (`owner = run`) and go with
/// `free_all_owned`; the slot would otherwise outlive them.
pub fn release_unstarted_of(owner: *mut BasePage) {
    if owner.is_null() {
        return;
    }
    for (i, p) in PROCESS_TABLE.iter().enumerate() {
        if p.read_state() == super::State::Loaded
            && p.load_owner.load(Ordering::Acquire) == owner
        {
            free_slot(i);
        }
    }
}

/// Convenience accessor for the child slot's [`BasePage`] VA on the
/// kernel side — the handler reads this so it can stamp `RUN` to
/// the child's basepage right before the eret.
#[inline]
pub fn child_basepage_kva(slot: usize) -> *mut BasePage {
    PROCESS_TABLE[slot].basepage_kva.load(Ordering::Acquire)
}
