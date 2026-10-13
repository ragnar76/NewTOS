//! Allocated blocks per owner, and the per-process limit (MEM05).
//!
//! EmuTOS has no per-process limit; its fixed MD pool (`bdos/osmem.c:36-39`,
//! at most 354 MDs) is the only bound, and Atari TOS 1.0 had "20 blocks per
//! process" (`doc/tos14fix.txt:822-824`). NewTOS grows its MD supply on
//! demand ([`super::mdpool`]) and bounds the number of **allocated blocks per
//! owner cookie** instead — a sanctioned deviation
//! (`docs/COMPATIBILITY.md`, "No fixed memory-descriptor limit"; design
//! `docs/analysis/mem05-design-2026-10-13.md` §3).
//!
//! ## What is counted
//!
//! For every owner cookie that is limited ([`is_limited`]: not null, not the
//! kernel, not the TSR holder) the number of MDs on either pool's allocated
//! list whose `m_own` is that cookie. Free-list MDs belong to nobody
//! (`freeit` clears `m_own`). Further owners ([`super::region`]) do not
//! count; only the EmuTOS tag `m_own`.
//!
//! Every change of `m_own` of an MD on an allocated list goes through
//! [`super::pool::set_md_owner`], which books the change here — the one
//! write point (design §3.4). Re-bookings (`set_owner`, `detach`, `F_KEEP`,
//! `Ptermres`) are never refused, so a receiver can briefly sit above the
//! limit; the limit applies to **new** allocations only ([`admits`],
//! checked by [`super::traps::mxalloc`] before `ffit`).
//!
//! ## Table
//!
//! Keyed by cookie, not by process slot: a child's cookie (its basepage)
//! owns blocks before a slot names it (`loader.rs`, `set_owner(tpa, tpa)`).
//! An entry is free again when its count drops to 0. A new cookie that
//! finds no free entry is refused by [`admits`] (fail-closed); a re-booking
//! that finds none is counted in [`lost_bookings`] and shows up in
//! [`audit`] — neither can happen with at most [`super::super::process::MAX_PROCESSES`]
//! processes alive.
//!
//! All functions run under the BDOS giant lock (single CPU, trap path).

use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use super::init::{is_initialised, pmd, pmdalt};
use super::traps;

/// Default limit of allocated blocks per process (user decision E3,
/// 2026-10-13). The boot argument `mdlimit=<n>` overrides it; `0` = off.
pub const DEFAULT_OWNER_MD_LIMIT: u32 = 8192;

/// Entries of the count table: two cookies per process (a running process
/// and a child it is loading) plus slack.
pub const OWNER_TABLE_LEN: usize = 2 * crate::bdos::process::MAX_PROCESSES + 8;

static LIMIT: AtomicU32 = AtomicU32::new(DEFAULT_OWNER_MD_LIMIT);
static LOST: AtomicUsize = AtomicUsize::new(0);

#[derive(Copy, Clone)]
struct Entry {
    /// Owner cookie; null = unused entry.
    who: *mut c_void,
    count: u32,
}

const NO_ENTRY: Entry = Entry { who: ptr::null_mut(), count: 0 };

struct Table(UnsafeCell<[Entry; OWNER_TABLE_LEN]>);
// SAFETY: accessed only under the BDOS giant lock (single CPU, trap path).
unsafe impl Sync for Table {}
static TABLE: Table = Table(UnsafeCell::new([NO_ENTRY; OWNER_TABLE_LEN]));

/// # Safety
///
/// BDOS giant lock held; no other borrow of the table alive.
unsafe fn table() -> &'static mut [Entry; OWNER_TABLE_LEN] {
    // SAFETY: caller contract.
    unsafe { &mut *TABLE.0.get() }
}

/// Set the per-process limit (`0` = no limit). The kernel calls this from
/// the boot argument `mdlimit=` before the pools are seeded; the host tests
/// call it freely.
pub fn set_limit(n: u32) {
    LIMIT.store(n, Ordering::Relaxed);
}

/// The per-process limit in force (`0` = none).
pub fn limit() -> u32 {
    LIMIT.load(Ordering::Relaxed)
}

/// Does the limit apply to `who`? Not to "nobody" (null), the kernel
/// ([`traps::kernel_owner`], which also holds inherited `F_KEEP` blocks) or
/// the TSR holder ([`traps::tsr_owner`]).
pub fn is_limited(who: *mut c_void) -> bool {
    !who.is_null() && who != traps::kernel_owner() && who != traps::tsr_owner()
}

/// Allocated blocks `who` holds the EmuTOS tag of (`0` for a cookie that is
/// not limited — those are not counted).
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn count_of(who: *mut c_void) -> u32 {
    if !is_limited(who) {
        return 0;
    }
    // SAFETY: lock held.
    unsafe { table() }.iter().find(|e| e.who == who).map_or(0, |e| e.count)
}

/// May `who` take one more block? `false` when the limit is on and `who`
/// holds that many already, or when `who` is new and the table has no free
/// entry (fail-closed). Always `true` for a cookie that is not limited.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn admits(who: *mut c_void) -> bool {
    if !is_limited(who) {
        return true;
    }
    // SAFETY: lock held.
    let t = unsafe { table() };
    match t.iter().find(|e| e.who == who) {
        Some(e) => {
            let lim = limit();
            lim == 0 || e.count < lim
        }
        // A cookie with no block yet: it needs a free entry.
        None => t.iter().any(|e| e.who.is_null()),
    }
}

/// Book the change of an allocated block's EmuTOS tag from `old` to `new`
/// (either may be null: a block entering or leaving the allocated list).
/// Never refuses.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn rebook(old: *mut c_void, new: *mut c_void) {
    if old == new {
        return;
    }
    // SAFETY: lock held.
    let t = unsafe { table() };
    if is_limited(old) {
        match t.iter_mut().find(|e| e.who == old) {
            Some(e) => {
                e.count = e.count.saturating_sub(1);
                if e.count == 0 {
                    *e = NO_ENTRY;
                }
            }
            None => {
                LOST.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    if is_limited(new) {
        if let Some(e) = t.iter_mut().find(|e| e.who == new) {
            e.count += 1;
        } else if let Some(e) = t.iter_mut().find(|e| e.who.is_null()) {
            *e = Entry { who: new, count: 1 };
        } else {
            LOST.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Bookings that found no table entry since boot (should stay `0`).
pub fn lost_bookings() -> usize {
    LOST.load(Ordering::Relaxed)
}

/// Owner cookies with a non-zero count right now.
///
/// # Safety
///
/// BDOS giant lock held.
pub unsafe fn tracked_owners() -> usize {
    // SAFETY: lock held.
    unsafe { table() }.iter().filter(|e| !e.who.is_null()).count()
}

/// Recompute every count by walking both pools' allocated lists and compare
/// with the table (design §3.4, R4). `Ok(blocks)` = the number of counted
/// blocks when everything agrees; `Err((who, booked, actual))` names the
/// first difference.
///
/// # Safety
///
/// BDOS giant lock held.
#[allow(clippy::type_complexity)]
pub unsafe fn audit() -> Result<usize, (*mut c_void, u32, u32)> {
    // SAFETY: lock held.
    let t = unsafe { table() };
    let mut seen = [0u32; OWNER_TABLE_LEN];
    let mut total = 0usize;
    if is_initialised() {
        for mp in [pmd(), pmdalt()] {
            // SAFETY: lock held; read-only list walk.
            let mut m = unsafe { (*mp).mp_mal };
            while !m.is_null() {
                // SAFETY: list invariant.
                let (own, next) = unsafe { ((*m).m_own, (*m).m_link) };
                if is_limited(own) {
                    match t.iter().position(|e| e.who == own) {
                        Some(i) => seen[i] += 1,
                        None => return Err((own, 0, 1)),
                    }
                    total += 1;
                }
                m = next;
            }
        }
    }
    for (e, &n) in t.iter().zip(seen.iter()) {
        if !e.who.is_null() && e.count != n {
            return Err((e.who, e.count, n));
        }
    }
    Ok(total)
}
