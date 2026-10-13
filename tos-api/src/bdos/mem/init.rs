//! Pool initialisation: seed the two MPBs (`pmd` for ST-RAM,
//! `pmdalt` for TT-RAM) from the kernel-discovered memory map.
//!
//! EmuTOS does this in `bdos/iumem.c::bmem_init` and the
//! `xmaddalt` plumbing in `bdos/umem.c`. NewTOS keeps the BDOS
//! layer free of kernel types: the kernel passes plain
//! `(start, len)` tuples and this module turns them into MDs +
//! installs them into the static MPBs.
//!
//! Spec:
//! - [`docs/api/structures/md.md`](../../../../../docs/api/structures/md.md)
//! - [`docs/api/gemdos/memory/14-maddalt.md`](../../../../../docs/api/gemdos/memory/14-maddalt.md)
//!
//! ## Address-width gate (PHASE-04 R4)
//!
//! `MD::m_start` is an `i32` (`Long`) so the Atari ABI is preserved.
//! On QEMU `virt` and on RPi4 with `<= 1` GiB DRAM the entire
//! managed region fits in the positive `i32` range. RPi4 with
//! `>= 2` GiB needs the address-window translation introduced in
//! Phase 05; until then [`init`] returns
//! [`InitError::AddressOutOfRange`] when called with such a region.

use core::cell::UnsafeCell;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

use tos_types::{Long, MD, MPB};

use super::mdpool::{xmgetmd_global, StaticMdSupply};

/// Errors returned by [`init`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitError {
    /// `init` was called more than once.
    AlreadyInitialised,
    /// A pool extent does not fit in the Atari `Long` ABI; Phase 05
    /// introduces the address-window translation that lifts this
    /// limit.
    AddressOutOfRange,
    /// The MD meta-pool is exhausted before either pool was seeded.
    /// Cannot happen: seeding takes two MDs and the supply's static
    /// first page holds [`super::mdpool::MDS_PER_PAGE`] (MEM05).
    NoFreeMd,
}

/// Holds an `MPB` behind an `UnsafeCell` so the BDOS layer can
/// mutate it under the BDOS lock without a per-MPB spin lock.
struct MpbCell(UnsafeCell<MPB>);

// SAFETY: every access to the cell goes through the BDOS giant
// lock (Phase 04 D6). The MPB itself only owns raw pointers into
// the MD supply, whose pages live for `'static` (MEM05: never given back).
unsafe impl Sync for MpbCell {}

impl MpbCell {
    const fn new() -> Self {
        Self(UnsafeCell::new(MPB::empty()))
    }

    /// Borrow the MPB mutably. Caller asserts that the BDOS lock is
    /// held.
    ///
    /// # Safety
    ///
    /// Must not be called concurrently with another `borrow_mut` on
    /// the same cell.
    #[allow(clippy::mut_from_ref)]
    unsafe fn borrow_mut(&self) -> &mut MPB {
        // SAFETY: caller contract.
        unsafe { &mut *self.0.get() }
    }

    /// Raw pointer to the MPB. Used by `ffit`/`freeit` which already
    /// take `*mut MPB`.
    fn as_ptr(&self) -> *mut MPB {
        self.0.get()
    }
}

/// ST-RAM MPB. EmuTOS calls this `pmd`.
static PMD: MpbCell = MpbCell::new();

/// TT-RAM (alt-RAM) MPB. EmuTOS calls this `pmdalt`.
static PMDALT: MpbCell = MpbCell::new();

static INITIALISED: AtomicBool = AtomicBool::new(false);

/// ST-RAM extent recorded at [`init`]. Used by `Maddalt` (D7) to
/// reject ranges that overlap the ST-RAM pool.
static STRAM_START: core::sync::atomic::AtomicI32 =
    core::sync::atomic::AtomicI32::new(0);
static STRAM_END: core::sync::atomic::AtomicI32 =
    core::sync::atomic::AtomicI32::new(0);

/// TT-RAM (alt-RAM) extent recorded at [`init`]. EmuTOS keeps the same
/// information in the `ramtop` system variable, which `ttram_detect`
/// sets once from the detected TT-RAM size and which `xmaddalt` never
/// updates afterwards (`bios/memory2.c:92-127`) — a later `Maddalt`
/// region therefore does not move it here either.
static TTRAM_START: core::sync::atomic::AtomicI32 =
    core::sync::atomic::AtomicI32::new(0);
static TTRAM_END: core::sync::atomic::AtomicI32 =
    core::sync::atomic::AtomicI32::new(0);

/// Inclusive start / exclusive end of the ST-RAM pool, in `Long`
/// (Atari ABI) coordinates. Returns `(0, 0)` before [`init`] runs.
pub fn stram_range() -> (Long, Long) {
    (
        STRAM_START.load(Ordering::Acquire),
        STRAM_END.load(Ordering::Acquire),
    )
}

/// Inclusive start / exclusive end of the TT-RAM (alt-RAM) pool as
/// seeded at boot, in `Long` (Atari ABI) coordinates. Returns `(0, 0)`
/// before [`init`] runs and when the board has no alt-RAM — which is
/// what EmuTOS reports through `ramtop == NULL` for a machine without
/// TT-RAM (`bios/memory2.c:119-121`).
pub fn ttram_range() -> (Long, Long) {
    (
        TTRAM_START.load(Ordering::Acquire),
        TTRAM_END.load(Ordering::Acquire),
    )
}

/// `*mut MPB` for the ST-RAM pool. Callable any time after [`init`]
/// has succeeded; before that, the MPB has empty free/alloc lists
/// and any `ffit` call simply returns `null`.
pub fn pmd() -> *mut MPB {
    PMD.as_ptr()
}

/// `*mut MPB` for the TT-RAM (alt-RAM) pool.
pub fn pmdalt() -> *mut MPB {
    PMDALT.as_ptr()
}

/// `true` once [`init`] has installed both pools.
pub fn is_initialised() -> bool {
    INITIALISED.load(Ordering::Acquire)
}

/// Install both MPBs.
///
/// `stram_start..stram_start+stram_len` becomes the initial free MD
/// of `pmd`; same for `ttram_*` and `pmdalt`. Either length may be
/// zero, in which case the corresponding pool is left empty (callers
/// will get `ENSMEM`).
///
/// Must be called exactly once, before any `Mxalloc` / `Malloc`
/// trap is taken.
///
/// # Safety
///
/// Caller must guarantee:
/// 1. The BDOS giant lock is held (or this is the boot CPU pre-trap,
///    which is equivalent).
/// 2. The two ranges do not overlap each other and do not overlap
///    any other live mapping (kernel image, stack, MMIO).
/// 3. The frame allocator backing these ranges (Phase 03 D4) will
///    never hand the same frames out via `frames::alloc_frame`.
///    NewTOS satisfies this by keeping kernel-side allocations on
///    the kernel heap (which has its own pre-allocated 2 MiB region)
///    and reserving the rest of each pool for BDOS use.
pub unsafe fn init(
    stram_start: u64,
    stram_len: u64,
    ttram_start: u64,
    ttram_len: u64,
) -> Result<(), InitError> {
    if INITIALISED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(InitError::AlreadyInitialised);
    }

    // SAFETY: caller holds the BDOS lock (or boot-CPU equivalent).
    let pmd = unsafe { PMD.borrow_mut() };
    // SAFETY: see above.
    let pmdalt = unsafe { PMDALT.borrow_mut() };

    if stram_len > 0 {
        // SAFETY: same lock invariant.
        let md = unsafe { seed_md(stram_start, stram_len)? };
        pmd.mp_mfl = md;
        pmd.mp_mal = ptr::null_mut();
        pmd.mp_rover = ptr::null_mut();
        // SAFETY: `md` is the seed MD; its m_start / m_length were
        // already validated to fit in `Long` by `seed_md`.
        unsafe {
            STRAM_START.store((*md).m_start, Ordering::Release);
            STRAM_END.store(
                (*md).m_start + (*md).m_length,
                Ordering::Release,
            );
        }
    }

    if ttram_len > 0 {
        // SAFETY: same lock invariant.
        let md = unsafe { seed_md(ttram_start, ttram_len)? };
        pmdalt.mp_mfl = md;
        pmdalt.mp_mal = ptr::null_mut();
        pmdalt.mp_rover = ptr::null_mut();
        // SAFETY: `md` is the seed MD; `seed_md` validated both fields
        // and their sum against the `Long` ABI.
        unsafe {
            TTRAM_START.store((*md).m_start, Ordering::Release);
            TTRAM_END.store(
                (*md).m_start + (*md).m_length,
                Ordering::Release,
            );
        }
    }

    Ok(())
}

/// Allocate one fresh MD from the global meta-pool and initialise it
/// to describe a free region `[start, start+len)`.
///
/// # Safety
///
/// Caller holds the BDOS lock.
unsafe fn seed_md(start: u64, len: u64) -> Result<*mut MD, InitError> {
    // The Atari ABI uses `Long` (`i32`) for both `m_start` and
    // `m_length`; reject ranges that do not fit until Phase 05
    // introduces the address-window translation.
    let start_i32: Long = start.try_into().map_err(|_| InitError::AddressOutOfRange)?;
    let len_i32: Long = len.try_into().map_err(|_| InitError::AddressOutOfRange)?;
    // Also make sure `start + len` does not wrap when the BDOS
    // allocator does its first-fit checks (`m_start + m_length`).
    start_i32
        .checked_add(len_i32)
        .ok_or(InitError::AddressOutOfRange)?;

    // SAFETY: caller holds the BDOS lock.
    let md = unsafe { xmgetmd_global() };
    if md.is_null() {
        return Err(InitError::NoFreeMd);
    }
    // SAFETY: `md` is a fresh, exclusive MD reference.
    unsafe {
        (*md).m_link = ptr::null_mut();
        (*md).m_start = start_i32;
        (*md).m_length = len_i32;
        (*md).m_own = ptr::null_mut();
    }
    Ok(md)
}

/// Construct a fresh `MdSupply` handle for use under the BDOS lock.
/// Convenience wrapper so trap implementations don't have to know
/// about `StaticMdSupply::new`.
///
/// # Safety
///
/// Caller must hold the BDOS giant lock for the lifetime of the
/// returned handle.
pub unsafe fn supply() -> StaticMdSupply {
    // SAFETY: forwarded.
    unsafe { StaticMdSupply::new() }
}
