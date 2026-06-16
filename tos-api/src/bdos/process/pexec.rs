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

use super::loader::{load_to_slot, LoadError, LoadedImage};
use super::registry;
use super::{allocate_slot, free_slot, PROCESS_TABLE};

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

/// Metadata the kernel SVC handler needs after a successful
/// `PE_LOADGO` setup. The handler uses these to redirect the trap
/// frame to the child and switch `TTBR0_EL1`.
#[derive(Copy, Clone, Debug)]
pub struct LoadedChild {
    /// Slot index in [`PROCESS_TABLE`] holding the child.
    pub slot: usize,
    /// User VA of the child's entry point.
    pub entry_va: u64,
    /// Phys of the child's `L0` table — feeds `TTBR0_EL1`.
    pub l0_pa: u64,
    /// User VA of the child's [`tos_types::BasePage`] — passed to
    /// the child entry in `x0` per NewTOS's AArch64 process ABI.
    pub basepage_va: u64,
}

/// Why `Pexec` failed.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PexecError {
    /// `mode` is not one of the documented [`PE_LOADGO`] /
    /// [`PE_LOAD`] / etc. constants. Mapped to `EINVFN` (-32).
    UnknownMode,
    /// `mode` is documented but not yet implemented in this phase.
    /// Mapped to `EINVFN` until D4 / D5 land.
    UnsupportedMode,
    /// `path` did not match any registered program.
    /// Mapped to `EFILNF` (-33).
    NotFound,
    /// [`allocate_slot`] returned `None` — process table is full.
    /// Mapped to `ENSMEM` (-39).
    NoSlot,
    /// The ELF blob was malformed or did not fit a single user
    /// page (Phase 06 D3 limit). Mapped to `EPLFMT` (-66) for
    /// most parser errors and `ENSMEM` for size-related ones.
    LoadFailed(LoadError),
}

// ─── Mode dispatch ──────────────────────────────────────────────────

/// `Pexec(mode, path, tail, env)` — the GEMDOS-side dispatch. Only
/// [`PE_LOADGO`] is implemented in Phase 06 D3.
///
/// `path_bytes` is the registry key (NUL-terminated user string;
/// caller is responsible for the bytes it hands in — Pexec does
/// **not** scan past the slice's `len`). `_tail` and `_env` are
/// reserved for D4 / D5 (cmdline tail and environment block); D3
/// ignores them and the child sees the basepage's defaults.
///
/// # Safety
///
/// Caller (the kernel SVC handler) holds the BDOS giant lock; the
/// caller is the boot CPU. The slot allocated here lives until the
/// child terminates via [`super::pterm::terminate`].
pub unsafe fn dispatch(
    mode: u16,
    path_bytes: &[u8],
    _tail: u64,
    _env: u64,
) -> Result<LoadedChild, PexecError> {
    match mode {
        PE_LOADGO => unsafe { load_go(path_bytes) },
        PE_LOAD | PE_GO | PE_BASEPAGE | PE_GOTHENFREE | PE_BASEPAGEFLAGS => {
            Err(PexecError::UnsupportedMode)
        }
        _ => Err(PexecError::UnknownMode),
    }
}

/// `Pexec(PE_LOADGO, path, …)` — load and launch.
///
/// # Safety
///
/// Caller's BDOS-lock contract; see [`dispatch`].
pub unsafe fn load_go(path_bytes: &[u8]) -> Result<LoadedChild, PexecError> {
    let blob = registry::lookup(path_bytes).ok_or(PexecError::NotFound)?;
    let slot_idx = allocate_slot().ok_or(PexecError::NoSlot)?;

    // SAFETY: slot is freshly claimed; loader populates the slot's
    // basepage_kva / basepage_pa / l0_pa atomics.
    let loaded: LoadedImage = match unsafe { load_to_slot(blob, slot_idx) } {
        Ok(img) => img,
        Err(e) => {
            // Roll back the slot reservation so the table doesn't
            // leak entries on a malformed blob.
            free_slot(slot_idx);
            return Err(PexecError::LoadFailed(e));
        }
    };

    Ok(LoadedChild {
        slot: loaded.slot,
        entry_va: loaded.entry_va,
        l0_pa: loaded.l0_pa,
        basepage_va: loaded.basepage_va,
    })
}

/// Convenience accessor for the child slot's [`BasePage`] VA on the
/// kernel side — the handler reads this so it can stamp `RUN` to
/// the child's basepage right before the eret.
#[inline]
pub fn child_basepage_kva(slot: usize) -> *mut BasePage {
    PROCESS_TABLE[slot].basepage_kva.load(Ordering::Acquire)
}
