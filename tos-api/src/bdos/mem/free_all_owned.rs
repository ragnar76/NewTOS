//! `free_all_owned` — Pterm cleanup walker.
//!
//! Port of EmuTOS `bdos/proc.c:88` `free_all_owned(PD *r, MPB *mp)`.
//! Walks one pool's allocated list (`mp_mal`) once and calls
//! [`super::pool::freeit`] for every MD whose `m_own` matches the
//! supplied cookie. Returns the number of MDs released so the
//! caller (Pterm) can log / assert.
//!
//! ## Why this lives in its own module
//!
//! `freeit` itself is a per-block primitive; `Pterm` needs the
//! aggregate "release everything this dying process owns" walk and
//! we want to iterate over **both** pools (ST-RAM + TT-RAM) without
//! making that a property of the trap layer.
//!
//! ## Iteration safety
//!
//! `freeit(m, mp, supply)` removes `m` from `mp_mal` and may free
//! up to two *other* MDs via coalescing on the free list — but
//! coalescing only ever touches `mp_mfl`, not `mp_mal`. So reading
//! `next = m->m_link` *before* the `freeit` call is safe: the
//! `m_link` value points at the next MD that was on `mp_mal` and
//! that MD is unaffected by the coalesce (it's still on `mp_mal`,
//! same link).
//!
//! This is exactly the EmuTOS approach.

use core::ffi::c_void;

use tos_types::{MD, MPB};

use super::init::{is_initialised, pmd, pmdalt};
use super::mdpool::{xmfremd_global, StaticMdSupply};
use super::pool::freeit;

/// Walk both BDOS pools' allocated lists and free every MD whose
/// `m_own == owner`. Returns the number of MDs released across
/// both pools.
///
/// `owner` is the same cookie that
/// [`super::traps::current_owner`] stamps into freshly allocated
/// MDs — for Pterm, the dying [`tos_types::BasePage`] kernel VA;
/// for the kernel sentinel sweep, `KERNEL_PD_SENTINEL`.
///
/// Returns `0` immediately if the BDOS pools have not been
/// initialised yet (boot races) or if `owner` is null (would
/// otherwise match every free MD).
///
/// # Safety
///
/// Caller must hold the BDOS giant lock for the duration. The lock
/// guarantees that no other code path mutates `mp_mal` / `mp_mfl`
/// while we walk; without it the linked-list invariants would
/// race with concurrent `Mxalloc` / `Mfree` callers.
pub unsafe fn free_all_owned(owner: *mut c_void) -> usize {
    if !is_initialised() || owner.is_null() {
        return 0;
    }

    // SAFETY: caller holds the BDOS giant lock; both pools are
    // initialised so their MPB statics are well-formed.
    let mut count = 0usize;
    unsafe {
        count += sweep_pool(pmd(), owner);
        count += sweep_pool(pmdalt(), owner);
    }
    count
}

/// Walk one pool's `mp_mal` and free every MD with the matching
/// owner. Returns the count released from this pool.
///
/// # Safety
///
/// `mp` must point at a well-formed MPB. Caller holds the BDOS
/// giant lock.
unsafe fn sweep_pool(mp: *mut tos_types::MPB, owner: *mut c_void) -> usize {
    // SAFETY: caller contract — `mp` is a valid MPB.
    let mut p: *mut MD = unsafe { (*mp).mp_mal };
    let mut count = 0usize;
    // SAFETY: caller contract — see Phase 04 D6 BDOS-lock note.
    let mut supply = unsafe { StaticMdSupply::new() };

    while !p.is_null() {
        // Snapshot the link *before* `freeit` reorganises lists.
        // SAFETY: list invariant — `p` is on `mp_mal`.
        let next = unsafe { (*p).m_link };
        // SAFETY: list invariant.
        let m_own = unsafe { (*p).m_own };

        if m_own == owner {
            // SAFETY: `p` is a valid allocated MD on `mp_mal`;
            // `mp` is its owning MPB; `supply` holds the lock.
            unsafe { freeit(p, mp, &mut supply) };
            count += 1;
        }
        p = next;
    }
    count
}

/// Walk both BDOS pools' allocated lists and **reserve** every MD
/// whose `m_own == owner`: unlink it from `mp_mal` and free the MD
/// struct, but **do not** return its memory to `mp_mfl`. The region
/// becomes "owner-less but allocated" — resident — which is the
/// technical heart of `Ptermres` (terminate-and-stay-resident).
/// Returns the number of MDs reserved across both pools.
///
/// Port of EmuTOS `reserve_blocks` (`bdos/proc.c:75`), applied to
/// both pools the way [`free_all_owned`] sweeps them.
///
/// **Model note:** EmuTOS first shrinks the basepage MD to `blkln`
/// via `xsetblk`, then reserves. NewTOS's TPA is a static arena page
/// (not a BDOS MD) until pool-backed TPAs land, so `blkln`-sizing is
/// a no-op here — the whole TPA is always resident, and this call
/// reserves the process's `Malloc`-ed pool blocks.
///
/// # Safety
///
/// Same contract as [`free_all_owned`]: caller holds the BDOS giant
/// lock.
pub unsafe fn reserve_all_owned(owner: *mut c_void) -> usize {
    if !is_initialised() || owner.is_null() {
        return 0;
    }
    // SAFETY: caller holds the BDOS giant lock; both pools initialised.
    let mut count = 0usize;
    unsafe {
        count += reserve_pool(pmd(), owner);
        count += reserve_pool(pmdalt(), owner);
    }
    count
}

/// Walk one pool's `mp_mal`, unlinking + freeing the MD struct (but
/// not the memory) for every MD with the matching owner. Returns the
/// count reserved from this pool.
///
/// # Safety
///
/// `mp` must point at a well-formed MPB; caller holds the BDOS lock.
unsafe fn reserve_pool(mp: *mut MPB, owner: *mut c_void) -> usize {
    let mut count = 0usize;
    // `q` is the address of the link that points at the current MD,
    // so we can splice the MD out in place (the EmuTOS `**q` idiom).
    // SAFETY: caller contract — `mp` is a valid MPB.
    let mut q: *mut *mut MD = unsafe { &raw mut (*mp).mp_mal };
    loop {
        // SAFETY: `q` points at a valid `mp_mal` link slot.
        let m = unsafe { *q };
        if m.is_null() {
            break;
        }
        // SAFETY: `m` is on `mp_mal`.
        let next = unsafe { (*m).m_link };
        let m_own = unsafe { (*m).m_own };
        if m_own == owner {
            // Unlink from the allocated list and free the MD struct;
            // the memory it described is intentionally left out of
            // both lists (resident / orphaned).
            // SAFETY: `m` is a valid allocated MD; `q` is its link.
            unsafe {
                *q = next;
                xmfremd_global(m);
            }
            count += 1;
            // `q` already points at the (now spliced-in) `next` link.
        } else {
            // SAFETY: advance the cursor to this MD's link slot.
            q = unsafe { &raw mut (*m).m_link };
        }
    }
    count
}
