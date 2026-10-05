//! Per-process TPA (Transient Program Area) allocation.
//!
//! A real compiled program (libc / gemlib / cflib app) wants a multi-page
//! image + zeroed BSS + a heap for `malloc` + a proper downward-growing
//! stack. This module takes that region — one per process — from the
//! **BDOS memory pool**, page-aligned, and hands the loader its address.
//!
//! ## The TPA is an ordinary pool block, at its own address
//!
//! Since ARCH03 stage 1 every user address is its own physical address
//! (VA = PA, [`super::mmu`]) and the pools are mapped for every process from
//! boot on, so the TPA executes where the pool put it: its block address is
//! the process's basepage — its **PD pointer** — exactly as EmuTOS'
//! `alloc_tpa` returns `md->m_start` and `init_pd_fields` sets `p_lowtpa =
//! p` (`bdos/proc.c:372,445-502`). `Mshrink(basepage, …)` and `Mfree(basepage)`
//! therefore name the block directly, and the owner tag decides who frees it:
//! the child for `PE_LOADGO` (its `Pterm` `free_all_owned` sweep), the caller
//! for `PE_LOAD` (EmuTOS `owner = (flag == PE_LOADGO) ? p : run`,
//! `bdos/proc.c:302-307`).
//!
//! ## Size
//!
//! [`TPA_SIZE`] = 4 MiB, fixed — plan decision E9 (fixed rather than
//! EmuTOS' "largest free block", which would starve every other process;
//! FreeMiNT bounds its TPA the same way with `initialmem`). The program
//! flags (E8b) choose its pool, not its size: no `PRGFLAGS` bit is defined
//! that would.
//!
//! ## Alignment
//!
//! The block starts on a 4-KiB page ([`TPA_ALIGN`]): the program is linked at
//! `0x1000` and keeps its link-time page offsets when relocated
//! (`adrp`-relative code), so the TPA base must be page-aligned.
//! [`crate::bdos::mem::traps::mxalloc_aligned`] gives the block that start
//! and returns the slack to the pool, so the block holds exactly the TPA.

use core::sync::atomic::Ordering;

use tos_types::Long;

use super::MAX_PROCESSES;

/// Per-process TPA size in bytes (4 MiB, plan E9).
pub const TPA_SIZE: u64 = 4 * 1024 * 1024;

/// Alignment of the TPA block start (one 4-KiB page).
pub const TPA_ALIGN: u64 = super::mmu::PAGE_SIZE;

const _: () = assert!(TPA_SIZE.is_multiple_of(TPA_ALIGN));

/// Size of a user page in bytes (= one 4-KiB physical frame).
pub const fn page_size() -> u64 {
    TPA_ALIGN
}

/// A page-aligned per-process TPA block from the BDOS pool.
pub struct Tpa {
    /// The block's address — `m_start`, the user VA (= PA) the program
    /// executes at and its PD pointer.
    pub base: u64,
    /// Kernel high-half alias of [`Self::base`] — the loader copies the
    /// ELF image and writes the basepage through this.
    pub base_kva: *mut u8,
}

/// Take one page-aligned [`TPA_SIZE`] block from the BDOS pool.
///
/// The pool follows the program's `PRGFLAGS` (EmuTOS `alloc_tpa`,
/// `bdos/proc.c:460-502`): with `PF_TTRAMLOAD` TT-RAM is preferred
/// (`MX_PREFTTRAM`, ST-RAM if TT-RAM cannot supply it), without it the TPA
/// is ST-RAM only. Returns `None` on `ENSMEM`.
///
/// The block is tagged with the *current* owner cookie by the allocator
/// (`owner = run`); the loader re-tags it to the child for `PE_LOADGO`.
///
/// # Safety
///
/// Caller holds the BDOS giant lock (or is the boot CPU pre-trap).
pub unsafe fn alloc_tpa(prgflags: u32) -> Option<Tpa> {
    let mode = super::loader::pool_mode(prgflags);
    // SAFETY: caller holds the BDOS lock.
    let raw = unsafe {
        crate::bdos::mem::traps::mxalloc_aligned(TPA_SIZE as Long, mode, TPA_ALIGN as Long)
    };
    if raw == 0 {
        return None;
    }
    // Pool addresses are positive `Long` in the low 2 GiB (BDOS `init`
    // rejects wider ranges), so this widening never sign-extends.
    let base = raw as u64;
    Some(Tpa {
        base,
        base_kva: super::kalias(base) as *mut u8,
    })
}

/// Return a TPA to the BDOS pool. Used on the loader's error-cleanup
/// path (a launched process instead frees its TPA via `Pterm`'s
/// `free_all_owned` owner sweep).
///
/// # Safety
///
/// Caller holds the BDOS giant lock and `base` is a live pool block
/// previously returned by [`alloc_tpa`].
pub unsafe fn free_tpa(base: Long) {
    // SAFETY: caller holds the BDOS lock; `base` is on the allocated list.
    // `Mfree`'s return is `EIMBA` only for a wild address, which cannot
    // happen for our own allocation.
    let _ = unsafe { crate::bdos::mem::traps::mfree(base) };
}

/// Page 0 of a loaded slot as a mutable byte page — the system variables,
/// the cookie-jar mirror, the trampolines and the vector table the loader
/// wrote ([`super::mmu::page0_kva`]). For the cookie-jar refresh and for
/// tests that inspect the loader's page-0 image.
///
/// Returns `None` for an out-of-range slot or a slot with no loaded image
/// (`basepage_kva == null`).
///
/// # Safety
///
/// Caller holds the BDOS giant lock for the duration of the borrow and
/// the slot currently holds a loaded image.
pub unsafe fn page_bytes_mut(slot: usize) -> Option<&'static mut [u8; 4096]> {
    if slot >= MAX_PROCESSES {
        return None;
    }
    if super::PROCESS_TABLE[slot]
        .basepage_kva
        .load(Ordering::Acquire)
        .is_null()
    {
        return None;
    }
    // SAFETY: caller's lock and aliasing contract.
    unsafe { super::mmu::page0_bytes(slot) }
}

/// The first 4 KiB of a loaded slot's **TPA** — its basepage and the
/// reserved gap above it — as a mutable byte page, through the kernel alias
/// of the TPA. For tests that inspect the basepage the loader wrote.
///
/// # Safety
///
/// As [`page_bytes_mut`].
pub unsafe fn basepage_bytes_mut(slot: usize) -> Option<&'static mut [u8; 4096]> {
    if slot >= MAX_PROCESSES {
        return None;
    }
    let kva = super::PROCESS_TABLE[slot].basepage_kva.load(Ordering::Acquire);
    if kva.is_null() {
        return None;
    }
    // SAFETY: the loader's TPA is at least 4 KiB from `basepage_kva`; the
    // caller's lock contract keeps the borrow unaliased.
    Some(unsafe { &mut *(kva as *mut [u8; 4096]) })
}
