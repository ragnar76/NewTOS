//! Blocks with several owners — FreeMiNT's region links on top of EmuTOS'
//! `m_own` (ARCH03 stage 2d, plan Frage 6 / §3.4, decision E15).
//!
//! Spec: `docs/api/gemdos/memory/49-mfree.md` § "several owners, and only an
//! owner frees"; `docs/api/gemdos/process/4b-pexec.md` (NewTOS notes, 2d).
//!
//! ## Model
//!
//! FreeMiNT gives a memory region a `links` count; every process (memory
//! space) that has the region attached holds one link, `Mfree` detaches the
//! caller, and the region goes back to the pool when `links` reaches 0
//! (`sys/memory.c:512-660` `attach_region`/`detach_region`,
//! `sys/dosmem.c:330-398` `sys_m_free`). EmuTOS has one owner per block, the
//! `m_own` tag, which decides what `Pterm` frees (`bdos/proc.c:92-101`).
//!
//! NewTOS keeps `m_own` as the **first** owner — the EmuTOS tag, unchanged
//! for every block that has one owner, which is every block but a few — and
//! records the **further** owners of a block here, keyed by its start
//! address. A block's links are `1 + its further owners`. Owners are PD
//! pointers (or the kernel / TSR sentinels), the same cookies `m_own` holds.
//!
//! Today further owners come only from `Pexec` 4/6 (the started child
//! attaches to its basepage and environment, FreeMiNT `sys/k_exec.c:518-520`),
//! so the table is small; it is a fixed array (no allocator under the BDOS).
//!
//! ## Invariants
//!
//! * An entry names an allocated block; whoever frees a block forgets its
//!   entry ([`forget`] — every free path in `traps` and the sweeps here).
//!   A fresh allocation clears a stale one as well (`traps::mxalloc`).
//! * An owner appears at most once per block (as `m_own` or in the entry).
//! * Callers hold the BDOS giant lock.

use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::ptr;

use tos_types::{Long, MD};

use super::init::{is_initialised, pmd, pmdalt};
use super::traps;

/// Blocks that can have further owners at the same time. Each running
/// `Pexec` 4/6 child needs two (basepage, environment); 16 processes at most.
pub const MAX_SHARED_BLOCKS: usize = 64;

/// Further owners per block (besides `m_own`).
pub const MAX_FURTHER_OWNERS: usize = 4;

#[derive(Copy, Clone)]
struct Entry {
    /// Block start (`m_start`); `0` = unused entry.
    loc: Long,
    /// Further owners; `null` = unused place.
    owners: [*mut c_void; MAX_FURTHER_OWNERS],
}

const EMPTY: Entry = Entry {
    loc: 0,
    owners: [ptr::null_mut(); MAX_FURTHER_OWNERS],
};

struct Table(UnsafeCell<[Entry; MAX_SHARED_BLOCKS]>);
// SAFETY: accessed only under the BDOS giant lock (single CPU, trap path).
unsafe impl Sync for Table {}
static TABLE: Table = Table(UnsafeCell::new([EMPTY; MAX_SHARED_BLOCKS]));

/// # Safety
///
/// BDOS giant lock held; no other borrow of the table alive.
unsafe fn table() -> &'static mut [Entry; MAX_SHARED_BLOCKS] {
    // SAFETY: caller contract.
    unsafe { &mut *TABLE.0.get() }
}

/// Why an ownership operation was refused.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RegionError {
    /// `addr` starts no allocated block.
    NoBlock,
    /// The caller is not an owner of the block.
    NotOwner,
    /// No room to record another owner ([`MAX_SHARED_BLOCKS`] /
    /// [`MAX_FURTHER_OWNERS`]).
    Full,
}

/// What dropping an owner did to the block.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Detached {
    /// It was the last owner: the block is back in the pool.
    Freed,
    /// Another owner still holds the block.
    Kept,
}

/// The allocated block starting at `addr` (its MD), on either pool.
///
/// # Safety
///
/// BDOS giant lock held.
unsafe fn block(addr: Long) -> Option<*mut MD> {
    if addr == 0 || !is_initialised() {
        return None;
    }
    for mp in [pmd(), pmdalt()] {
        // SAFETY: lock held; read-only list walk.
        let mut m = unsafe { (*mp).mp_mal };
        while !m.is_null() {
            // SAFETY: list invariant.
            unsafe {
                if (*m).m_start == addr {
                    return Some(m);
                }
                m = (*m).m_link;
            }
        }
    }
    None
}

fn entry_of(t: &mut [Entry; MAX_SHARED_BLOCKS], addr: Long) -> Option<&mut Entry> {
    t.iter_mut().find(|e| e.loc == addr && addr != 0)
}

/// Is `who` an owner of the block at `addr` (`m_own` or a further owner)?
/// `false` for an address that starts no block.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn is_owner(addr: Long, who: *mut c_void) -> bool {
    if who.is_null() {
        return false;
    }
    // SAFETY: lock held.
    let Some(m) = (unsafe { block(addr) }) else {
        return false;
    };
    // SAFETY: `m` is on `mp_mal`.
    if unsafe { (*m).m_own } == who {
        return true;
    }
    // SAFETY: lock held.
    let t = unsafe { table() };
    entry_of(t, addr).is_some_and(|e| e.owners.contains(&who))
}

/// Make `who` an owner of the block at `addr` — FreeMiNT `attach_region`.
/// Nothing to do if it is one already.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn attach(addr: Long, who: *mut c_void) -> Result<(), RegionError> {
    // SAFETY: lock held.
    let m = unsafe { block(addr) }.ok_or(RegionError::NoBlock)?;
    // SAFETY: `m` is on `mp_mal`.
    if who.is_null() || unsafe { (*m).m_own } == who {
        return Ok(());
    }
    // SAFETY: lock held.
    let t = unsafe { table() };
    if entry_of(t, addr).is_none() {
        let free = t.iter_mut().find(|e| e.loc == 0).ok_or(RegionError::Full)?;
        *free = Entry { loc: addr, ..EMPTY };
    }
    let e = entry_of(t, addr).ok_or(RegionError::Full)?;
    if e.owners.contains(&who) {
        return Ok(());
    }
    let place = e.owners.iter_mut().find(|o| o.is_null()).ok_or(RegionError::Full)?;
    *place = who;
    // The new owner's rights (ARCH03 3b, FreeMiNT `attach_region` →
    // `mark_proc_region(PROT_P)`, `sys/memory.c:539`).
    // SAFETY: lock held.
    unsafe { prot_changed(addr) };
    Ok(())
}

/// The owners of the block at `addr` changed: its rights follow
/// ([`crate::bdos::process::memprot`]).
///
/// # Safety
///
/// BDOS giant lock held.
unsafe fn prot_changed(addr: Long) {
    // SAFETY: lock held.
    unsafe { crate::bdos::process::memprot::on_owner_change(addr as u32 as u64) };
}

/// Drop `who` from the block at `addr` — FreeMiNT `detach_region`: if it was
/// the last owner the block is freed ([`Detached::Freed`]); if it held the
/// EmuTOS tag and another owner is left, that owner takes the tag.
///
/// # Errors
///
/// [`RegionError::NoBlock`] / [`RegionError::NotOwner`]; nothing changes.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn detach(addr: Long, who: *mut c_void) -> Result<Detached, RegionError> {
    // SAFETY: lock held.
    let m = unsafe { block(addr) }.ok_or(RegionError::NoBlock)?;
    // SAFETY: lock held.
    let t = unsafe { table() };
    let further = entry_of(t, addr);
    // A further owner: drop its place.
    if let Some(e) = further {
        if let Some(o) = e.owners.iter_mut().find(|o| **o == who && !who.is_null()) {
            *o = ptr::null_mut();
            if e.owners.iter().all(|o| o.is_null()) {
                *e = EMPTY;
            }
            // SAFETY: lock held.
            unsafe { prot_changed(addr) };
            return Ok(Detached::Kept);
        }
    }
    // SAFETY: `m` is on `mp_mal`.
    if who.is_null() || unsafe { (*m).m_own } != who {
        return Err(RegionError::NotOwner);
    }
    // The tag holder: pass the tag on, or free the block.
    if let Some(e) = entry_of(t, addr) {
        if let Some(o) = e.owners.iter_mut().find(|o| !o.is_null()) {
            // SAFETY: `m` is on `mp_mal`.
            unsafe { (*m).m_own = *o };
            *o = ptr::null_mut();
            if e.owners.iter().all(|o| o.is_null()) {
                *e = EMPTY;
            }
            // SAFETY: lock held.
            unsafe { prot_changed(addr) };
            return Ok(Detached::Kept);
        }
    }
    // SAFETY: lock held; `addr` starts an allocated block.
    let _ = unsafe { traps::mfree(addr) };
    Ok(Detached::Freed)
}

/// Hand the block at `addr` from `from` to `to`: `to` becomes an owner and
/// `from` stops being one — FreeMiNT's `Pexec` 6 (`attach_region(child)`,
/// then `mkfree` → `detach_region(parent)`, `sys/k_exec.c:518-520,586-590`),
/// EmuTOS' `set_owner(p, p)` (`bdos/proc.c:238-242`). A block `from` does not
/// own only gains `to`.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn hand_over(addr: Long, from: *mut c_void, to: *mut c_void) -> Result<(), RegionError> {
    // SAFETY: lock held.
    unsafe { attach(addr, to)? };
    // SAFETY: lock held. `to` holds a link now, so this never frees.
    match unsafe { detach(addr, from) } {
        Ok(_) | Err(RegionError::NotOwner) => Ok(()),
        Err(e) => Err(e),
    }
}

/// The block at `addr` was freed (by any path): forget its further owners.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn forget(addr: Long) {
    // SAFETY: lock held.
    let t = unsafe { table() };
    if let Some(e) = entry_of(t, addr) {
        *e = EMPTY;
    }
}

/// `Pterm`: drop `who` from every block it owns — freeing what nobody else
/// owns, passing the EmuTOS tag on where somebody does. Returns the number
/// of blocks freed. With one owner per block this is EmuTOS'
/// `free_all_owned` (`bdos/proc.c:92-101`) exactly.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn detach_all(who: *mut c_void) -> usize {
    if who.is_null() || !is_initialised() {
        return 0;
    }
    // Its further-owner places first: they never free anything.
    // SAFETY: lock held.
    for e in unsafe { table() }.iter_mut().filter(|e| e.loc != 0) {
        let loc = e.loc;
        let mut changed = false;
        for o in e.owners.iter_mut().filter(|o| **o == who) {
            *o = ptr::null_mut();
            changed = true;
        }
        if e.owners.iter().all(|o| o.is_null()) {
            *e = EMPTY;
        }
        if changed {
            // SAFETY: lock held.
            unsafe { prot_changed(loc) };
        }
    }
    // Then every block it holds the tag of.
    let mut freed = 0;
    for mp in [pmd(), pmdalt()] {
        // SAFETY: lock held; `next` is read before `detach` may free `m`
        // (freeing only touches the free list, `free_all_owned` module docs).
        let mut m = unsafe { (*mp).mp_mal };
        while !m.is_null() {
            // SAFETY: list invariant.
            let (next, start, own) = unsafe { ((*m).m_link, (*m).m_start, (*m).m_own) };
            if own == who {
                // SAFETY: lock held; `start` starts a block `who` owns.
                if let Ok(Detached::Freed) = unsafe { detach(start, who) } {
                    freed += 1;
                }
            }
            m = next;
        }
    }
    freed
}

/// `Ptermres`: every link `who` holds passes to the TSR holder
/// ([`traps::tsr_owner`]) — its blocks stay allocated and owned (FreeMiNT
/// keeps the TSR's regions attached in `TSR_Q`, `sys/k_exit.c:307-336`).
/// Returns the number of blocks whose EmuTOS tag it held.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn keep_resident(who: *mut c_void) -> usize {
    if who.is_null() || !is_initialised() {
        return 0;
    }
    let tsr = traps::tsr_owner();
    // SAFETY: lock held.
    for e in unsafe { table() }.iter_mut().filter(|e| e.loc != 0) {
        let loc = e.loc;
        let had_tsr = e.owners.contains(&tsr);
        let mut changed = false;
        for o in e.owners.iter_mut().filter(|o| **o == who) {
            *o = if had_tsr { ptr::null_mut() } else { tsr };
            changed = true;
        }
        if e.owners.iter().all(|o| o.is_null()) {
            *e = EMPTY;
        }
        if changed {
            // SAFETY: lock held.
            unsafe { prot_changed(loc) };
        }
    }
    let mut kept = 0;
    for mp in [pmd(), pmdalt()] {
        // SAFETY: lock held.
        let mut m = unsafe { (*mp).mp_mal };
        while !m.is_null() {
            // SAFETY: list invariant.
            unsafe {
                if (*m).m_own == who {
                    (*m).m_own = tsr;
                    kept += 1;
                    // The TSR holder may already be a further owner: one link.
                    if let Some(e) = entry_of(table(), (*m).m_start) {
                        for o in e.owners.iter_mut().filter(|o| **o == tsr) {
                            *o = ptr::null_mut();
                        }
                        if e.owners.iter().all(|o| o.is_null()) {
                            *e = EMPTY;
                        }
                    }
                    if entry_of(table(), (*m).m_start).is_none() {
                        // Only the resident holder owns it now: private
                        // memory becomes global (FreeMiNT `sys_ptermres`,
                        // `sys/k_exit.c:320-336`, ARCH03 3b).
                        crate::bdos::process::memprot::on_resident((*m).m_start as u32 as u64);
                    } else {
                        prot_changed((*m).m_start);
                    }
                }
                m = (*m).m_link;
            }
        }
    }
    kept
}

/// The further owners of the block at `addr` (besides `m_own`); null
/// places are unused. All null for a block with one owner.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn further_owners(addr: Long) -> [*mut c_void; MAX_FURTHER_OWNERS] {
    // SAFETY: lock held.
    let t = unsafe { table() };
    entry_of(t, addr).map_or([ptr::null_mut(); MAX_FURTHER_OWNERS], |e| e.owners)
}

/// Number of owners of the block at `addr` (FreeMiNT's `links`): `0` if it
/// starts no block. For tests and diagnostics.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn links(addr: Long) -> usize {
    // SAFETY: lock held.
    if unsafe { block(addr) }.is_none() {
        return 0;
    }
    // SAFETY: lock held.
    let t = unsafe { table() };
    1 + entry_of(t, addr).map_or(0, |e| e.owners.iter().filter(|o| !o.is_null()).count())
}
