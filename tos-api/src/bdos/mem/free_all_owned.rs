//! `free_all_owned` / `reserve_all_owned` — the `Pterm` and `Ptermres`
//! sweeps over a dying process's memory.
//!
//! EmuTOS walks each pool's allocated list once and frees every MD whose
//! `m_own` is the dying PD (`free_all_owned`, `bdos/proc.c:92-101`) or
//! unlinks it for `Ptermres` (`reserve_blocks`, `:75-89`). Since ARCH03
//! stage 2d a block can have further owners (FreeMiNT region links,
//! [`super::region`]): the `Pterm` sweep drops the process from every block
//! it owns and frees what nobody else holds — identical to EmuTOS while every
//! block has one owner — and the `Ptermres` sweep hands every link to the
//! TSR holder (stage 2c, FreeMiNT `TSR_Q`).
//!
//! Iteration safety: freeing a block (`freeit`) removes its MD from
//! `mp_mal` and may coalesce *other* MDs, but only on the free list, so the
//! walk reads `next` before it frees (the EmuTOS approach).

use core::ffi::c_void;

use super::init::is_initialised;

/// `Pterm`'s sweep: drop `owner` from every block it owns, freeing what no
/// other owner holds (EmuTOS `free_all_owned`, `bdos/proc.c:92-101`, which
/// frees every MD whose `m_own == owner` — the same thing while a block has
/// one owner; [`super::region::detach_all`]). Returns the number of blocks
/// released across both pools.
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
    // ARCH03 2d: a block can have further owners (FreeMiNT links). Dropping
    // `owner` frees what nobody else holds and passes the EmuTOS tag of the
    // rest on — with one owner per block, `sweep_pool` below exactly.
    // SAFETY: caller holds the BDOS giant lock.
    unsafe { super::region::detach_all(owner) }
}

/// Walk both BDOS pools' allocated lists and **keep resident** every block
/// whose `m_own == owner`: its owner becomes the TSR holder
/// ([`super::traps::tsr_owner`]) and it stays on `mp_mal`, allocated —
/// `Ptermres`' memory survives its process. Returns the number of blocks
/// kept across both pools.
///
/// FreeMiNT's model (ARCH03 stage 2c, plan §3.8 / Frage 16): `Ptermres`
/// leaves the regions attached to the process in `TSR_Q`
/// (`sys/k_exit.c:307-336`), so the memory remains allocated *and owned*.
/// EmuTOS reaches the same "memory survives" contract by unlinking the MDs
/// and dropping them (`reserve_blocks`, `bdos/proc.c:75-89`), which leaves
/// the region in neither list; keeping the MD lets the kernel still tell a
/// resident block from free memory (`is_allocated`, the AES reading a TSR's
/// data) and keeps it from being freed by someone who is not its owner.
///
/// # Safety
///
/// Same contract as [`free_all_owned`]: caller holds the BDOS giant
/// lock.
pub unsafe fn reserve_all_owned(owner: *mut c_void) -> usize {
    // Every link `owner` holds — as `m_own` or as a further owner (ARCH03
    // 2d) — passes to the TSR holder.
    // SAFETY: caller holds the BDOS giant lock.
    unsafe { super::region::keep_resident(owner) }
}
