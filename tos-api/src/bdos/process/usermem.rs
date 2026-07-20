//! Per-process EL0 user-memory manager (ADR-021).
//!
//! Classic TOS is flat and unprotected: a program dereferences whatever
//! physical address `Malloc`/`Mxalloc` returns. NewTOS runs apps at **EL0**
//! behind `TTBR0_EL1`, and the ported EmuTOS pool ([`super::super::mem`])
//! returns **physical addresses** that are mapped only in the kernel high
//! half — so an EL0 app faults the instant it touches a returned pointer
//! (QED's `Mxalloc` → a TT-RAM PA `0x0160a000` write fault).
//!
//! This module is the thin **address-translation + EL0-mapping shim** that
//! ADR-021 wraps around the *unchanged* pool allocator. For an EL0 caller,
//! [`crate::gemdos::memory`] allocates from the real ST/TT pool, then calls
//! [`map`] to place the block into a **per-process EL0 heap window** and hand
//! back a user VA; [`translate`] / [`free`] undo that mapping on
//! `Mshrink`/`Mfree`. It **generalises the task-#99 RSC window**
//! ([`crate::aes::resource`]) — the resource blob is now just another mapped
//! block ([`Kind::Rsc`]).
//!
//! ## Per-process VA layout (all low-half, ILP32-safe)
//!
//! ```text
//!   [0, USER_TPA_TOP)                     TPA image + BSS + heap + stack
//!   [USER_TPA_TOP, USER_TPA_MAX_VA)       the EL0 heap window (this module)
//! ```
//!
//! The heap window is a compact low-VA range mapped **offset** (user VA ≠ PA),
//! kept well under 2 GiB so every returned pointer is a positive `Long`
//! (ADR-020). Blocks are handed out with a **bump cursor**: each allocation
//! takes a *fresh* window VA that was previously **invalid**, so no stale TLB
//! entry can exist for it and the app's return-to-EL0 `eret` needs **no TLBI**
//! (which would wedge QEMU — QUIRK-001; see [`super::mmu::install_ttbr0`]).
//!
//! ## Block table
//!
//! Each process slot owns a small table of [`Block`]s
//! (`{ user_va, m_start, map_pa, size, kind }`) plus the window cursor.
//! **Entry 0 is seeded with the TPA** at load ([`seed_tpa`]), so
//! `Mshrink(basepage @ user VA 0)` / `Mfree(0)` translate to the TPA's pool
//! block instead of failing `EIMBA` (the latent bug ADR-021 also fixes).
//! GEMDOS `Malloc` is called rarely with large sizes (a libc `malloc` grabs
//! big chunks and sub-allocates in user space), so a few dozen entries
//! suffice — [`MAX_USER_BLOCKS`].
//!
//! ## Concurrency
//!
//! The table lives in a static `UnsafeCell` mutated only under the BDOS giant
//! lock (the single-CPU trap context), the same discipline the pool allocator
//! and the AES resource loader use.

use core::cell::UnsafeCell;

use tos_types::Long;

use super::mmu::{self, PageFlags};
use super::MAX_PROCESSES;

/// User-page size (one 4-KiB physical frame). Public so
/// [`crate::gemdos::memory`] can size its one-page alignment slack.
pub const PAGE_SIZE: u64 = 4096;

/// Base user VA of the per-process EL0 heap window: the low-half VA directly
/// above the TPA (`= USER_TPA_TOP`). `Malloc`/`Mxalloc` blocks and the RSC
/// blob are mapped here.
const WINDOW_BASE: u64 = super::arena::TPA_SIZE;

/// Size of the EL0 heap window in bytes: all mappable low-half VA above the
/// TPA. Enlarge by bumping [`super::mmu::USER_L3_COUNT`].
const WINDOW_SIZE: u64 = mmu::USER_TPA_MAX_VA - super::arena::TPA_SIZE;

// The window must start page-aligned (`map_page` requires it) and be
// non-empty (at least one spare L3 beyond the TPA).
const _: () = assert!(WINDOW_BASE.is_multiple_of(PAGE_SIZE));
const _: () = assert!(WINDOW_SIZE >= PAGE_SIZE);

/// Maximum number of simultaneously-tracked user blocks per process (the TPA
/// entry plus `Malloc`/`Mxalloc`/RSC blocks). Sized generously versus real
/// apps, which make a handful of large GEMDOS allocations (a libc `malloc`
/// sub-allocates its own arena in user space rather than calling GEMDOS per
/// object).
const MAX_USER_BLOCKS: usize = 32;

/// What a mapped block is, so the page permissions and bookkeeping match its
/// role. Purely internal accounting — the pool block is identical regardless.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Kind {
    /// The process's Transient Program Area (seeded as entry 0 at load).
    Tpa,
    /// A `Malloc`/`Mxalloc` heap block (mapped user RW, EL0-executable — the
    /// flat classic-TOS heap semantics, matching the TPA's own attributes).
    Heap,
    /// The AES resource (`.RSC`) blob (mapped user RW, non-executable — it is
    /// data). Folds the task-#99 RSC window into this table.
    Rsc,
}

/// One tracked per-process user block: the app-visible window VA, the pool
/// key (`Mxalloc`'s `m_start`, used to free/shrink), the page-aligned PA the
/// window VA maps to, the mapped byte span, and its [`Kind`].
#[derive(Copy, Clone)]
struct Block {
    /// App-visible low-half VA (page-aligned; `0` for the TPA).
    user_va: u64,
    /// Pool block start (`Mxalloc` `m_start`) — the free/shrink key.
    m_start: Long,
    /// Page-aligned physical address that `user_va` maps to. `>= m_start`
    /// by the alignment slack (0..one page).
    map_pa: u64,
    /// Mapped span in bytes (page-rounded). Window bytes this block consumes.
    size: u64,
    /// Block role.
    kind: Kind,
}

/// One process slot's user-memory state: the block table and the bump cursor
/// (next free byte offset within the window).
struct SlotMem {
    blocks: [Option<Block>; MAX_USER_BLOCKS],
    cursor: u64,
}

impl SlotMem {
    const fn new() -> Self {
        Self {
            blocks: [None; MAX_USER_BLOCKS],
            cursor: 0,
        }
    }
}

/// Static per-slot user-memory tables. Mutated only under the BDOS giant lock
/// (single-CPU trap context), like the pool allocator and the RSC loader.
struct UserMem(UnsafeCell<[SlotMem; MAX_PROCESSES]>);
// SAFETY: single boot CPU under the implicit BDOS giant lock.
unsafe impl Sync for UserMem {}

static USER_MEM: UserMem =
    UserMem(UnsafeCell::new([const { SlotMem::new() }; MAX_PROCESSES]));

/// The result of an address lookup ([`translate`]): the pool key, the
/// alignment `prefix` (`map_pa − m_start`) a `Mshrink` must add to a
/// user-relative length to reach the corresponding pool-block length, and the
/// mapped `size` (the largest length the app can validly hold — the window
/// maps exactly this many bytes).
#[derive(Copy, Clone)]
pub struct TransBlock {
    /// Pool block start (`Mxalloc` `m_start`) — the `Mfree`/`Mshrink` key.
    pub m_start: Long,
    /// Bytes between the pool block start and the page-aligned base the user
    /// VA maps to. Add to a user-relative length for the pool-block length.
    pub prefix: u64,
    /// Mapped span in bytes (page-rounded). A `Mshrink` to a longer length
    /// cannot be honoured — the window maps only this much — so it is `EGSBF`.
    pub size: u64,
}

/// Round `v` up to the next multiple of [`PAGE_SIZE`].
#[inline]
fn page_round_up(v: u64) -> u64 {
    (v + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
}

/// Reset `slot`'s user-memory table + window cursor and install the **TPA as
/// block 0** (`user_va = 0`, pool key `raw_start`, page-aligned PA `base_pa`,
/// span `size`). Called by the loader once the TPA is mapped, so an EL0
/// `Mshrink`/`Mfree` of the basepage (user VA 0) translates to the pool block
/// rather than returning `EIMBA` (ADR-021). Resetting also clears any entries
/// left by a previous tenant of this slot.
///
/// # Safety
///
/// Caller holds the BDOS giant lock (single-CPU trap/boot context).
pub unsafe fn seed_tpa(slot: usize, raw_start: Long, base_pa: u64, size: u64) {
    if slot >= MAX_PROCESSES {
        return;
    }
    // SAFETY: caller's lock contract; exclusive access to this slot.
    let sm = unsafe { &mut (*USER_MEM.0.get())[slot] };
    *sm = SlotMem::new();
    sm.blocks[0] = Some(Block {
        user_va: 0,
        m_start: raw_start,
        map_pa: base_pa,
        size,
        kind: Kind::Tpa,
    });
}

/// Map an already-allocated pool block into `slot`'s EL0 heap window and
/// record it, returning the page-aligned **user VA** the app dereferences.
///
/// * `m_start` — the pool block's `Mxalloc` return (the free/shrink key). The
///   caller must have over-allocated at least one extra page so the
///   page-aligned base (`round_up(m_start)`) plus `size` still lies inside the
///   block (`m_start` is 16-byte-aligned, not page-aligned).
/// * `size` — bytes to map (rounded up to whole pages).
/// * `kind` — [`Kind::Heap`] (user RW-exec) or [`Kind::Rsc`] (user RW-NX).
///
/// Returns `None` — mapping nothing — when the block table is full or the
/// window is exhausted (an honest `ENSMEM`-style failure); the caller then
/// frees `m_start`.
///
/// The mapped pages sit at a **fresh** window VA (bump cursor), so no stale
/// TLB entry exists and [`publish`] needs no TLBI (QUIRK-001).
///
/// # Safety
///
/// Caller holds the BDOS giant lock; `m_start` addresses a live pool block of
/// at least `round_up(size) + one page` bytes exclusively owned by `slot`'s
/// process.
pub unsafe fn map(slot: usize, m_start: Long, size: u64, kind: Kind) -> Option<u64> {
    if slot >= MAX_PROCESSES {
        return None;
    }
    let map_len = page_round_up(size);
    if map_len == 0 {
        return None;
    }
    // Pool addresses are positive `Long` in the low 2 GiB (BDOS `init`
    // rejects wider ranges), so widening never sign-extends.
    let base_pa = page_round_up(m_start as u64);

    // SAFETY: caller's lock contract; exclusive access to this slot.
    let sm = unsafe { &mut (*USER_MEM.0.get())[slot] };
    let idx = sm.blocks.iter().position(Option::is_none)?;
    if map_len > WINDOW_SIZE - sm.cursor {
        return None; // window exhausted
    }
    let user_va = WINDOW_BASE + sm.cursor;

    let flags = match kind {
        Kind::Rsc => PageFlags::UserRwNx,
        Kind::Tpa | Kind::Heap => PageFlags::UserRwExec,
    };
    let mut off = 0u64;
    while off < map_len {
        // SAFETY: slot bounds-checked; the VA is in-window (< USER_TPA_MAX_VA)
        // and page-aligned, the PA is page-aligned.
        if unsafe { mmu::map_page(slot, user_va + off, base_pa + off, flags) }.is_err() {
            // Only unreachable validation errors can land here; the fresh
            // window VAs were never handed out, so leaving the partial
            // descriptors is harmless. The caller frees the pool block.
            return None;
        }
        off += PAGE_SIZE;
    }
    // Publish the new PTEs before the app can walk them. The window VAs were
    // invalid, so no stale TLB entry exists → no TLBI (QUIRK-001).
    // SAFETY: barrier-only, permitted at EL1.
    unsafe { mmu::publish_user_mappings() };

    sm.blocks[idx] = Some(Block {
        user_va,
        m_start,
        map_pa: base_pa,
        size: map_len,
        kind,
    });
    sm.cursor += map_len;
    Some(user_va)
}

/// Order writes made through a mapped block's kernel high-half alias before
/// the owning EL0 process reads them (barrier-only). Used by `rsrc_load`
/// after it copies + relocates the `.RSC` blob into the block via the alias:
/// [`map`] already published the PTEs, this orders the **data** that was
/// written afterwards.
///
/// # Safety
///
/// Barrier-only; always permitted at EL1.
pub unsafe fn publish() {
    // SAFETY: DSB/ISB are unprivileged-safe barriers.
    unsafe { mmu::publish_user_mappings() };
}

/// Look up the block whose window/TPA user VA is exactly `user_va`, for
/// `Mshrink` (needs the pool key + alignment prefix). Returns `None`
/// (→ `EIMBA`) if no block *starts* at `user_va`, matching EmuTOS's "address
/// not on `mp_mal`" error.
pub fn translate(slot: usize, user_va: u64) -> Option<TransBlock> {
    if slot >= MAX_PROCESSES {
        return None;
    }
    // SAFETY: single-CPU under the BDOS lock; shared read.
    let sm = unsafe { &(*USER_MEM.0.get())[slot] };
    sm.blocks
        .iter()
        .flatten()
        .find(|b| b.user_va == user_va)
        .map(|b| TransBlock {
            m_start: b.m_start,
            prefix: b.map_pa - b.m_start as u64,
            size: b.size,
        })
}

/// Remove the block starting at `user_va` from `slot`'s table and return its
/// pool key (`m_start`) so the caller frees it via
/// [`crate::bdos::mem::traps::mfree`]. Returns `None` (→ `EIMBA`) if no block
/// starts there.
///
/// The window page mappings are intentionally **left in place**: the app
/// holds no live pointer into the freed block and the bump cursor never
/// re-hands the VA, so the stale descriptors are never walked again and no
/// TLBI is required (QUIRK-001).
///
/// A [`Kind::Rsc`] block is **not** reclaimable through `Mfree` — it is
/// managed by the AES `rsrc_free`, which tracks it by pool key
/// ([`free_by_mstart`]). Skipping it here (→ `EIMBA`) keeps a stray
/// `Mfree(rsc_va)` from racing a later `rsrc_free` into a double free.
pub fn free(slot: usize, user_va: u64) -> Option<Long> {
    if slot >= MAX_PROCESSES {
        return None;
    }
    // SAFETY: single-CPU under the BDOS lock; exclusive access.
    let blocks = unsafe { &mut (*USER_MEM.0.get())[slot].blocks };
    let pos = blocks
        .iter()
        .position(|b| matches!(b, Some(bl) if bl.user_va == user_va))?;
    // RSC blocks are freed via `rsrc_free`, not `Mfree` (reads `kind`).
    if matches!(&blocks[pos], Some(bl) if bl.kind == Kind::Rsc) {
        return None;
    }
    blocks[pos].take().map(|bl| bl.m_start)
}

/// Drop the table entry whose pool key is `m_start` (if any), for callers
/// that track the block by its pool address rather than its user VA (the AES
/// `rsrc_free`, which remembers the RSC block's `m_start`). Does **not** free
/// the pool block — the caller does that separately. Returns `true` if an
/// entry was removed.
pub fn free_by_mstart(slot: usize, m_start: Long) -> bool {
    if slot >= MAX_PROCESSES {
        return false;
    }
    // SAFETY: single-CPU under the BDOS lock; exclusive access.
    let blocks = unsafe { &mut (*USER_MEM.0.get())[slot].blocks };
    match blocks
        .iter()
        .position(|b| matches!(b, Some(bl) if bl.m_start == m_start))
    {
        Some(pos) => {
            blocks[pos] = None;
            true
        }
        None => false,
    }
}

/// Remaining mappable window bytes for `slot`, rounded **down** to a whole
/// page. Bounds an EL0 `Malloc(-1)`/`Mxalloc(-1)` so the largest reported
/// block is one the app can actually map (ADR-021 §4.5.6): the honest answer
/// is `min(pool-largest-free, window-remaining)`, and this returns the window
/// term.
pub fn window_remaining(slot: usize) -> u64 {
    if slot >= MAX_PROCESSES {
        return 0;
    }
    // SAFETY: single-CPU under the BDOS lock; shared read.
    let cursor = unsafe { (*USER_MEM.0.get())[slot].cursor };
    (WINDOW_SIZE - cursor) & !(PAGE_SIZE - 1)
}
