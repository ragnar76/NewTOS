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

use tos_types::MD;

use super::init::{is_initialised, pmd, pmdalt};
use super::mdpool::StaticMdSupply;
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
