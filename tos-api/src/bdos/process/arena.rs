//! Per-process TPA (Transient Program Area) allocation.
//!
//! A real compiled program (libc / gemlib / cflib app) needs far more
//! than the 4-KiB single page the Phase-06 D3 bring-up handed out: it
//! wants a multi-page image + zeroed BSS + a heap for `malloc` + a
//! proper downward-growing stack. This module carves that region — one
//! per process — from the **BDOS memory pool** (`Mxalloc`), page-
//! aligns it, and hands the loader the addresses it needs to copy the
//! image in and map it into the process's `TTBR0_EL1`.
//!
//! ## Why the pool, not a static BSS arena
//!
//! The earlier interim arena reserved a fixed 4-KiB BSS page per slot.
//! Growing that to a multi-MiB × [`super::MAX_PROCESSES`] static array
//! would waste megabytes of always-resident BSS for slots that are
//! usually idle. Instead the TPA is a normal BDOS allocation: it is
//! taken on `Pexec` and returned on `Pterm`. That is also the *correct*
//! `m_own` process-memory ownership model (EmuTOS `bdos/proc.c` — the
//! basepage MD is one of the process-owned blocks): the loader tags the
//! TPA block with the child's basepage cookie via
//! [`crate::bdos::mem::traps::set_owner`], so the child's `Pterm`
//! [`crate::bdos::mem::free_all_owned::free_all_owned`] sweep reclaims
//! it with no special-casing — exactly like the environment block.
//!
//! ## Size
//!
//! [`TPA_SIZE`] = 4 MiB. Rationale: enough for a real app's image
//! (tens–hundreds of KiB), zeroed BSS, a `malloc` heap, and a generous
//! stack, while staying within the per-process page-table capacity
//! [`super::mmu::USER_TPA_MAX_VA`]. It is a compile-time constant today;
//! a per-`Pexec` size (from a `PRGFLAGS` stack request or a caller-
//! supplied length) is a later refinement — the loader would thread a
//! requested size through [`alloc_tpa`] and size the L3 count to match.
//!
//! ## Alignment
//!
//! `Mxalloc` returns 16-byte-aligned addresses, but page-table leaves
//! need 4-KiB alignment. [`alloc_tpa`] over-allocates one page and
//! rounds the base up, so the page-aligned `[base_pa, base_pa +
//! TPA_SIZE)` window always lies inside the pool block. The block is
//! freed by its 16-byte-aligned pool start ([`Tpa::raw_start`]).

use core::sync::atomic::Ordering;

use tos_types::{Long, MX_PREFTTRAM};

use super::mmu;
use super::MAX_PROCESSES;

/// Per-process TPA size in bytes (4 MiB). See the module docs for the
/// choice; must not exceed the page-table mapping capacity.
pub const TPA_SIZE: u64 = 4 * 1024 * 1024;

const PAGE_SIZE: u64 = 4096;

/// Phase-05 high-half offset. Mirror of the constant in
/// `kernel::arch::aarch64::mmu`; a BDOS-pool physical address is
/// reachable from the kernel at `phys + VA_OFFSET`.
const VA_OFFSET: u64 = 0xFFFF_0000_0000_0000;

// The TPA must be page-aligned-mappable and fit the per-process tables.
const _: () = assert!(TPA_SIZE.is_multiple_of(PAGE_SIZE));
const _: () = assert!(TPA_SIZE <= mmu::USER_TPA_MAX_VA);

#[inline(always)]
const fn pa_to_kva(pa: u64) -> u64 {
    pa.wrapping_add(VA_OFFSET)
}

/// Size of a user page in bytes (= one 4-KiB physical frame).
pub const fn page_size() -> u64 {
    PAGE_SIZE
}

/// A page-aligned per-process TPA carved from the BDOS pool.
pub struct Tpa {
    /// Pool block start (`Mxalloc` `m_start`) — the value to hand to
    /// [`free_tpa`] / `Mfree`. 16-byte-aligned, may be below
    /// [`Self::base_pa`] by up to one page.
    pub raw_start: Long,
    /// Page-aligned base physical address. The process's `TTBR0_EL1`
    /// maps user VA `USER_VA_BASE` (0) here.
    pub base_pa: u64,
    /// Kernel high-half VA aliasing [`Self::base_pa`] — the loader
    /// copies the ELF image and writes the basepage through this.
    pub base_kva: *mut u8,
}

/// Allocate one page-aligned [`TPA_SIZE`] region from the BDOS pool.
///
/// TT-RAM is preferred (`MX_PREFTTRAM`), matching the environment-block
/// allocation and the modern `PF_TTRAMLOAD` default; it falls back to
/// ST-RAM only if TT-RAM is exhausted. Over-allocates one page so the
/// rounded-up page-aligned base always fits inside the block. Returns
/// `None` on `ENSMEM`.
///
/// The returned block is tagged with the *current* owner cookie by
/// `Mxalloc`; the caller re-tags it to the child basepage via
/// [`crate::bdos::mem::traps::set_owner`] so `Pterm` frees it.
///
/// # Safety
///
/// Caller holds the BDOS giant lock (or is the boot CPU pre-trap).
pub unsafe fn alloc_tpa() -> Option<Tpa> {
    // One extra page of slack guarantees the page-aligned base fits.
    let want = (TPA_SIZE + PAGE_SIZE) as Long;
    // SAFETY: caller holds the BDOS lock.
    let raw = unsafe { crate::bdos::mem::traps::mxalloc(want, MX_PREFTTRAM) };
    if raw == 0 {
        return None;
    }
    // Pool addresses are positive `Long` in the low 2 GiB (BDOS `init`
    // rejects wider ranges), so this widening never sign-extends.
    let raw_u = raw as u64;
    let base_pa = (raw_u + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    Some(Tpa {
        raw_start: raw,
        base_pa,
        base_kva: pa_to_kva(base_pa) as *mut u8,
    })
}

/// Return a TPA to the BDOS pool. Used on the loader's error-cleanup
/// path (a launched process instead frees its TPA via `Pterm`'s
/// `free_all_owned` owner sweep).
///
/// # Safety
///
/// Caller holds the BDOS giant lock and `raw_start` is a live pool
/// block previously returned by [`alloc_tpa`].
pub unsafe fn free_tpa(raw_start: Long) {
    // SAFETY: caller holds the BDOS lock; `raw_start` is on the
    // allocated list. `Mfree`'s return is `EIMBA` only for a wild
    // address, which cannot happen for our own allocation.
    let _ = unsafe { crate::bdos::mem::traps::mfree(raw_start) };
}

/// First-page (4-KiB) mutable view of a loaded slot's TPA, for tests
/// that inspect the basepage the loader wrote. Reads the slot's
/// published `basepage_kva` (the page-aligned TPA base).
///
/// Returns `None` for an out-of-range slot or a slot with no loaded
/// TPA (`basepage_kva == null`).
///
/// # Safety
///
/// Caller holds the BDOS giant lock for the duration of the borrow and
/// the slot currently holds a loaded TPA.
pub unsafe fn page_bytes_mut(slot: usize) -> Option<&'static mut [u8; 4096]> {
    if slot >= MAX_PROCESSES {
        return None;
    }
    let kva = super::PROCESS_TABLE[slot]
        .basepage_kva
        .load(Ordering::Acquire);
    if kva.is_null() {
        return None;
    }
    // SAFETY: the loader mapped `[base_kva, base_kva + TPA_SIZE)`; the
    // first 4 KiB (basepage + start of image) is live and, by the
    // caller's lock contract, not aliased for the borrow's lifetime.
    Some(unsafe { &mut *(kva as *mut [u8; 4096]) })
}
