//! Pool diagnostics — Phase 04 D13.
//!
//! Per-pool snapshot of free / allocated bytes, list lengths, and
//! the largest contiguous free block, modelled on
//! `kernel::bios::machine::heap::stats()`. Intended for boot-time
//! diagnostics, the Phase-04 EC1–EC9 selftests, and the Phase-06
//! `Sysconf`-style queries.
//!
//! Walks both linked lists (`mp_mfl` for free, `mp_mal` for
//! allocated) under the BDOS giant lock, so the snapshot is
//! consistent w.r.t. concurrent traps. Cost is O(n) in MD count
//! (MEM05: the supply grows, up to tens of thousands of MDs); the
//! callers are the boot log and the selftests, so the walk is
//! acceptable.
//!
//! ## Example
//!
//! ```ignore
//! use tos_api::bdos::mem::stats::{pool_stats, PoolKind};
//!
//! let st = pool_stats(PoolKind::Stram);
//! kprintln!(
//!     "[mm] ST-RAM: {} used / {} free / largest {} ({} alloc, {} free MDs)",
//!     st.used_bytes, st.free_bytes, st.largest_free,
//!     st.allocated_blocks, st.free_blocks,
//! );
//! ```

use tos_types::{Long, MD, MPB};

use super::init;

/// Identifier for the two BDOS memory pools.
///
/// Mirrors the `MX_STRAM`/`MX_TTRAM` selectors for `Mxalloc` but is
/// kept as a strongly-typed enum because diagnostic callers should
/// not deal with raw `i16` mode bits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolKind {
    /// `pmd` — historical Atari ST-RAM pool. NewTOS profile maps the
    /// first 16 MiB of physical DRAM (per [`super::init::stram_range`]).
    Stram,
    /// `pmdalt` — Atari TT/Falcon alt-RAM pool. NewTOS profile maps
    /// the rest of usable DRAM (≥ 256 MiB on every supported board).
    Ttram,
}

/// Snapshot of one pool's accounting at a moment in time.
///
/// All byte counts are sums of `MD::m_length` across the relevant
/// list and therefore reflect *user-visible* allocation sizes. The
/// MD-meta-allocator overhead (the `MDBlock` pages of
/// [`super::mdpool`], kernel memory outside the pools) is **not**
/// counted here — see [`super::mdpool::stats`] and
/// [`super::pages::stats`] (MEM05).
///
/// Field types match the Atari `Long` ABI so the struct can be
/// shipped to userland verbatim by a future XBIOS / cookie-jar
/// query without translation. Counts (`free_blocks`,
/// `allocated_blocks`) use `usize` because they live in kernel
/// space only.
#[derive(Clone, Copy, Debug, Default)]
pub struct PoolStats {
    /// Which pool this snapshot describes.
    pub pool: Option<PoolKind>,
    /// Sum of `m_length` over `mp_mfl` (free list).
    pub free_bytes: Long,
    /// Sum of `m_length` over `mp_mal` (allocated list).
    pub used_bytes: Long,
    /// Largest single free MD on `mp_mfl`. `0` when the pool is
    /// fully allocated (or empty / not initialised).
    pub largest_free: Long,
    /// Number of MDs on `mp_mfl`. A high value relative to
    /// `free_bytes` is a fragmentation signal.
    pub free_blocks: usize,
    /// Number of MDs on `mp_mal`. Equals the count of distinct
    /// outstanding `Malloc`/`Mxalloc` results that have not been
    /// `Mfree`d yet.
    pub allocated_blocks: usize,
}

impl PoolStats {
    /// Total bytes managed by the pool: free + allocated. Saturates
    /// on the (theoretical) overflow case rather than wrapping.
    pub const fn total_bytes(&self) -> Long {
        self.free_bytes.saturating_add(self.used_bytes)
    }
}

/// Compute pool statistics for the named pool.
///
/// Cheap convenience wrapper over [`pool_stats_raw`] that resolves
/// `kind` to the appropriate static MPB pointer via
/// [`super::init`].
///
/// Safe to call any time after [`super::init::init`] has run; called
/// before init it returns an all-zero snapshot (the static cells
/// have empty free/alloc lists).
pub fn pool_stats(kind: PoolKind) -> PoolStats {
    let mp = match kind {
        PoolKind::Stram => init::pmd(),
        PoolKind::Ttram => init::pmdalt(),
    };
    // SAFETY: `init::pmd`/`init::pmdalt` hand out pointers to static
    // MPB cells; they are always non-null and live for `'static`.
    // The list traversal only follows `m_link` chains which are
    // protected by the BDOS giant lock the caller holds.
    let mut s = unsafe { pool_stats_raw(mp) };
    s.pool = Some(kind);
    s
}

/// Compute pool statistics for an arbitrary MPB.
///
/// Walks `mp_mfl` and `mp_mal` once each, summing lengths and
/// counting nodes; tracks the running max for `largest_free`. The
/// `pool` field of the returned [`PoolStats`] is left as `None` —
/// callers that know the [`PoolKind`] should set it themselves
/// (the high-level [`pool_stats`] does this).
///
/// # Safety
///
/// `mp` must point to a valid, properly-linked [`MPB`]; both list
/// heads must terminate in null. The caller must hold the BDOS
/// giant lock for the duration of the call.
pub unsafe fn pool_stats_raw(mp: *mut MPB) -> PoolStats {
    debug_assert!(!mp.is_null());

    // SAFETY: caller contract guarantees `mp` is live and exclusively
    // observed under the BDOS lock for this scan.
    let mp_ref = unsafe { &*mp };

    let mut free_bytes: Long = 0;
    let mut largest: Long = 0;
    let mut free_blocks: usize = 0;
    {
        let mut q: *mut MD = mp_ref.mp_mfl;
        while !q.is_null() {
            // SAFETY: list invariant — every reachable MD is live.
            let len = unsafe { (*q).m_length };
            free_bytes = free_bytes.saturating_add(len);
            if len > largest {
                largest = len;
            }
            free_blocks += 1;
            // SAFETY: list invariant.
            q = unsafe { (*q).m_link };
        }
    }

    let mut used_bytes: Long = 0;
    let mut allocated_blocks: usize = 0;
    {
        let mut q: *mut MD = mp_ref.mp_mal;
        while !q.is_null() {
            // SAFETY: list invariant.
            let len = unsafe { (*q).m_length };
            used_bytes = used_bytes.saturating_add(len);
            allocated_blocks += 1;
            // SAFETY: list invariant.
            q = unsafe { (*q).m_link };
        }
    }

    PoolStats {
        pool: None,
        free_bytes,
        used_bytes,
        largest_free: largest,
        free_blocks,
        allocated_blocks,
    }
}
