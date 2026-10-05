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
//! ## Size — the largest free block (EmuTOS `alloc_tpa`, plan E9)
//!
//! The TPA is the **largest free block** of the pool the program flags
//! choose, exactly as EmuTOS `alloc_tpa` gives it (`bdos/proc.c:460-502`):
//! `ffit(-1L, &pmd)` / `ffit(-1L, &pmdalt)` measure the largest block of each
//! pool (`:466,476`), TT-RAM is a candidate only with `PF_TTRAMLOAD` (`:475`),
//! a pool qualifies when its block holds what the program `needed` (`:467,
//! 477`), the TT-RAM block is taken unless ST-RAM is the larger and TT-RAM
//! cannot hold `needed` plus the `PRGFLAGS` bits 31-28 tie-break amount
//! (`((flags >> 28) & 0x0f) + 1) * TPASIZE_QUANTUM`, 128 KiB, `:34,481-484`),
//! and the whole block is allocated (`ffit(size, …)`, `:486,493`); its length
//! becomes `p_hitpa - p_lowtpa` (`*avail`, `init_pd_fields`, `:374`). A
//! program gives back what it does not need itself — every TOS start-up code
//! does (`Mshrink`; the SDK's `crt0`/`crtinit` as mintlib's `_crtinit`,
//! `mintlib/crtinit.c:227-247`), and the AES does it for an accessory
//! (EmuTOS `pgmld`, `util/gemdos.c:29-47`).
//!
//! ## Alignment
//!
//! The block starts on a 4-KiB page ([`TPA_ALIGN`]): the program is linked at
//! `0x1000` and keeps its link-time page offsets when relocated
//! (`adrp`-relative code), so the TPA base must be page-aligned. With
//! protection on the pool grain is a page already; without it
//! [`crate::bdos::mem::traps::mxalloc_largest`] gives the slack in front of
//! the boundary back, so the TPA is the largest block minus less than a page.

use core::sync::atomic::Ordering;

use tos_types::{Long, MX_STRAM, MX_TTRAM, PF_TTRAMLOAD};

use super::MAX_PROCESSES;

/// Alignment of the TPA block start (one 4-KiB page).
pub const TPA_ALIGN: u64 = super::mmu::PAGE_SIZE;

/// EmuTOS `TPASIZE_QUANTUM` (`bdos/proc.c:34`): the unit of the `PRGFLAGS`
/// bits 31-28 "would like to have" amount in the ST/TT-RAM tie-break.
pub const TPASIZE_QUANTUM: u64 = 128 * 1024;

/// Size of a user page in bytes (= one 4-KiB physical frame).
pub const fn page_size() -> u64 {
    TPA_ALIGN
}

/// A page-aligned per-process TPA block from the BDOS pool.
pub struct Tpa {
    /// The block's address — `m_start`, the user VA (= PA) the program
    /// executes at and its PD pointer.
    pub base: u64,
    /// The block's length — `p_hitpa - p_lowtpa` (EmuTOS `*avail`).
    pub len: u64,
    /// Kernel high-half alias of [`Self::base`] — the loader copies the
    /// ELF image and writes the basepage through this.
    pub base_kva: *mut u8,
}

/// The pool EmuTOS `alloc_tpa` takes the TPA from, given the usable sizes of
/// the largest ST-RAM and TT-RAM blocks (`st`, `alt`; `alt` 0 when there is
/// no TT-RAM), the program's `flags` and what it `needed`
/// (`bdos/proc.c:460-502`). `None`: neither pool can hold `needed` (`ENSMEM`).
pub fn choose_pool(flags: u32, needed: u64, st: u64, alt: u64) -> Option<i16> {
    let st_ok = st >= needed;
    let mut alt_ok = flags & PF_TTRAMLOAD != 0 && alt >= needed;
    if st_ok && alt_ok && st > alt {
        let tpasize = (((flags >> 28) & 0x0f) as u64 + 1) * TPASIZE_QUANTUM;
        if needed + tpasize > alt {
            alt_ok = false; // force allocation in ST RAM
        }
    }
    if alt_ok {
        Some(MX_TTRAM)
    } else if st_ok {
        Some(MX_STRAM)
    } else {
        None
    }
}

/// What the TPA in the largest block of `mp` would hold: the block length
/// minus the slack up to the page boundary ([`TPA_ALIGN`]). 0 without a block.
///
/// # Safety
///
/// Caller holds the BDOS giant lock.
unsafe fn usable_largest(mp: *mut tos_types::MPB) -> u64 {
    // SAFETY: caller contract.
    match unsafe { crate::bdos::mem::traps::largest_free_block(mp) } {
        Some((start, len)) => {
            let start = start as u32 as u64;
            let slack = start.next_multiple_of(TPA_ALIGN) - start;
            (len as u32 as u64).saturating_sub(slack)
        }
        None => 0,
    }
}

/// Take the TPA — the largest free block of the pool EmuTOS `alloc_tpa`
/// chooses ([`choose_pool`], `bdos/proc.c:460-502`) — page-aligned, for a
/// program that `needed` bytes from its PD on (EmuTOS: `tlen + dlen + blen +
/// sizeof(PD)`, `:291`; for `Pexec` 5/7 `sizeof(PD)`, `:220`). Returns `None`
/// on `ENSMEM`.
///
/// The block is tagged with the *current* owner cookie by the allocator
/// (`owner = run`); the loader re-tags it to the child for `PE_LOADGO`.
///
/// # Safety
///
/// Caller holds the BDOS giant lock (or is the boot CPU pre-trap).
pub unsafe fn alloc_tpa(prgflags: u32, needed: u64) -> Option<Tpa> {
    use crate::bdos::mem::init::{pmd, pmdalt};
    // SAFETY: caller holds the BDOS lock.
    let (st, alt) = unsafe { (usable_largest(pmd()), usable_largest(pmdalt())) };
    let mode = choose_pool(prgflags, needed, st, alt)?;
    // SAFETY: caller holds the BDOS lock.
    let (raw, len) =
        unsafe { crate::bdos::mem::traps::mxalloc_largest(mode, TPA_ALIGN as Long) }?;
    // Pool addresses are positive `Long` in the low 2 GiB (BDOS `init`
    // rejects wider ranges), so this widening never sign-extends.
    let base = raw as u64;
    let len = len as u32 as u64;
    if len < needed {
        // SAFETY: caller holds the BDOS lock; `raw` was just allocated.
        unsafe { free_tpa(raw) };
        return None;
    }
    Some(Tpa {
        base,
        len,
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
