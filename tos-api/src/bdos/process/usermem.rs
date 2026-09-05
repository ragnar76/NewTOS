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
//! (`{ user_va, m_start, map_pa, size, kind, prot }`) plus the window cursor.
//! **Entry 0 is seeded with the TPA** at load ([`seed_tpa`]), so
//! `Mshrink(basepage @ user VA 0)` / `Mfree(0)` translate to the TPA's pool
//! block instead of failing `EIMBA` (the latent bug ADR-021 also fixes).
//! GEMDOS `Malloc` is called rarely with large sizes (a libc `malloc` grabs
//! big chunks and sub-allocates in user space), so a few dozen entries
//! suffice — [`MAX_USER_BLOCKS`].
//!
//! ## Publishing kernel structures (ADR-023)
//!
//! The same window also carries the other direction of the same problem: a
//! syscall that returns a *pointer into the kernel* (BIOS `Getbpb`,
//! `Getmpb`) hands an EL0 program a high-half address it cannot read.
//! [`expose`] maps such a `'static` kernel object into the calling process's
//! window under a FreeMiNT protection mode
//! ([`tos_types::ProtMode`], `references/freemint/sys/mint/mem.h:130-134`) —
//! `PROT_PR` for structures a program only reads — and returns the user VA.
//! These blocks are [`Kind::Shared`]: the process may read them but never
//! owns, shrinks or frees them.
//!
//! ## Concurrency
//!
//! The table lives in a static `UnsafeCell` mutated only under the BDOS giant
//! lock (the single-CPU trap context), the same discipline the pool allocator
//! and the AES resource loader use.

use core::cell::UnsafeCell;

use tos_types::{Long, ProtMode};

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
    /// A **kernel** structure published into this process so the program can
    /// read what a syscall handed it a pointer to ([`expose`], ADR-023). Not
    /// a pool block: the process neither owns nor may free it, so `Mfree` /
    /// `Mshrink` refuse it and `Block::m_start` is `0`.
    Shared,
}

/// One tracked per-process user block: the app-visible window VA, the pool
/// key (`Mxalloc`'s `m_start`, used to free/shrink), the page-aligned PA the
/// window VA maps to, the mapped byte span, and its [`Kind`].
#[derive(Copy, Clone)]
struct Block {
    /// App-visible low-half VA (page-aligned; `0` for the TPA).
    user_va: u64,
    /// Pool block start (`Mxalloc` `m_start`) — the free/shrink key.
    /// `0` for a [`Kind::Shared`] block, which has no pool block behind it.
    m_start: Long,
    /// Page-aligned physical address that `user_va` maps to. `>= m_start`
    /// by the alignment slack (0..one page).
    map_pa: u64,
    /// Mapped span in bytes (page-rounded). Window bytes this block consumes.
    size: u64,
    /// Block role.
    kind: Kind,
    /// Protection the block was published with — `Some` only for
    /// [`Kind::Shared`] ([`expose`]); pool-backed blocks take their page
    /// flags from [`Kind`] alone.
    prot: Option<ProtMode>,
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

/// High-half kernel VA offset — the whole of DRAM is mapped at
/// `phys + VA_OFFSET` for EL1 (`kernel::arch::aarch64::mmu`,
/// `kernel/src/arch/aarch64/link-rpi4.ld:24`). Mirrored here the same way
/// [`super::arena`] and [`super::loader`] mirror it.
const VA_OFFSET: u64 = 0xFFFF_0000_0000_0000;

/// Physical address behind a kernel high-half VA. Inverse of the mapping
/// the kernel image is linked and mapped with.
#[inline]
const fn kva_to_pa(kva: u64) -> u64 {
    kva.wrapping_sub(VA_OFFSET)
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
        prot: None,
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
///   [`Kind::Shared`] belongs to [`expose`] and is rejected here.
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
        // A kernel structure is published by `expose`, which owns the
        // protection choice; it is never a pool block, so it cannot be
        // mapped through this path.
        Kind::Shared => return None,
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
        prot: None,
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

/// Publish a **kernel** structure into `slot`'s EL0 window and return the
/// user VA the process must dereference (ADR-023).
///
/// This is the counterpart of [`map`] for memory the kernel owns and keeps:
/// a syscall that hands an application a pointer to one of its own
/// structures (BIOS `Getbpb` → the drive's BPB, `Getmpb` → the memory
/// descriptor) must hand back an address the *caller* can read. The kernel
/// is linked high-half (`0xFFFF_0000_0000_0000 + phys`), and those pages are
/// EL1-only, so the raw `&'static` is useless at EL0.
///
/// * `kernel_va` — address of the structure inside the kernel image. Its
///   physical page is what gets mapped; the intra-page offset is preserved,
///   so the returned VA points at the same field layout.
/// * `len` — bytes of the structure that must be readable.
/// * `prot` — FreeMiNT protection mode
///   (`references/freemint/sys/mint/mem.h:130-134`):
///   [`ProtMode::Pr`] (`PROT_PR`, read-only) and [`ProtMode::G`]
///   (`PROT_G`, read/write) produce a user mapping; [`ProtMode::P`],
///   [`ProtMode::S`] and [`ProtMode::I`] mean "not reachable from the user"
///   and therefore yield `None` — no mapping is made.
///
/// **Idempotent per physical page.** A second `expose` of anything on an
/// already-published page returns a VA in the same window mapping instead of
/// consuming more window: `Getbpb` is called once per drive and would
/// otherwise leak a page of window per call. The re-use requires the
/// existing entry to already cover the requested span and to have been
/// published with the same protection, otherwise a fresh mapping is made.
///
/// Returns `None` when the block table is full, the window is exhausted, or
/// `prot` grants the user nothing. Callers then keep whatever fallback their
/// contract prescribes (the BIOS functions return the kernel pointer to
/// their in-kernel callers and `0`/NULL to a user they cannot serve).
///
/// # Safety
///
/// Caller holds the BDOS giant lock. `kernel_va .. kernel_va + len` must lie
/// inside the kernel's high-half image mapping and stay valid for as long as
/// the process may hold the returned pointer — i.e. it must be a `'static`
/// kernel object, not a stack or heap temporary. The whole physical page(s)
/// covering it become readable (and, with [`ProtMode::G`], writable) to the
/// process, so the object must own its page(s).
pub unsafe fn expose(
    slot: usize,
    kernel_va: *const u8,
    len: usize,
    prot: ProtMode,
) -> Option<u64> {
    if slot >= MAX_PROCESSES || len == 0 {
        return None;
    }
    // PROT_P / PROT_S / PROT_I give the user no access at all — there is
    // nothing to map, which is a legitimate answer, not a failure.
    let flags = match prot {
        ProtMode::Pr => PageFlags::UserRoNx,
        ProtMode::G => PageFlags::UserRwNx,
        ProtMode::P | ProtMode::S | ProtMode::I => return None,
    };

    let pa = kva_to_pa(kernel_va as u64);
    let base_pa = pa & !(PAGE_SIZE - 1);
    let offset = pa - base_pa;
    let span = page_round_up(offset + len as u64);

    // SAFETY: caller's lock contract; exclusive access to this slot.
    let sm = unsafe { &mut (*USER_MEM.0.get())[slot] };

    // Already published? Re-use the same window VA (see "Idempotent").
    if let Some(b) = sm.blocks.iter().flatten().find(|b| {
        b.kind == Kind::Shared && b.map_pa == base_pa && b.size >= span && b.prot == Some(prot)
    }) {
        return Some(b.user_va + offset);
    }

    let idx = sm.blocks.iter().position(Option::is_none)?;
    if span > WINDOW_SIZE - sm.cursor {
        return None; // window exhausted
    }
    let user_va = WINDOW_BASE + sm.cursor;

    let mut off = 0u64;
    while off < span {
        // SAFETY: slot bounds-checked; the VA is in-window and page-aligned,
        // the PA is the page-aligned physical address of a kernel page.
        if unsafe { mmu::map_page(slot, user_va + off, base_pa + off, flags) }.is_err() {
            // Only unreachable validation errors land here; the window VAs
            // were never handed out, so the partial descriptors are dead.
            return None;
        }
        off += PAGE_SIZE;
    }
    // Publish the PTEs before the app can walk them. The window VAs were
    // invalid, so no stale TLB entry exists → no TLBI (QUIRK-001).
    // SAFETY: barrier-only, permitted at EL1.
    unsafe { mmu::publish_user_mappings() };

    sm.blocks[idx] = Some(Block {
        user_va,
        m_start: 0,
        map_pa: base_pa,
        size: span,
        kind: Kind::Shared,
        prot: Some(prot),
    });
    sm.cursor += span;
    Some(user_va + offset)
}

/// Look up the block whose window/TPA user VA is exactly `user_va`, for
/// `Mshrink` (needs the pool key + alignment prefix). Returns `None`
/// (→ `EIMBA`) if no block *starts* at `user_va`, matching EmuTOS's "address
/// not on `mp_mal`" error.
///
/// [`Kind::Shared`] blocks are invisible here: the process does not own the
/// kernel structure published into its window, so `Mshrink` on that VA is
/// `EIMBA` — exactly what EmuTOS answers for an address that is not on the
/// allocated list.
pub fn translate(slot: usize, user_va: u64) -> Option<TransBlock> {
    if slot >= MAX_PROCESSES {
        return None;
    }
    // SAFETY: single-CPU under the BDOS lock; shared read.
    let sm = unsafe { &(*USER_MEM.0.get())[slot] };
    sm.blocks
        .iter()
        .flatten()
        .find(|b| b.user_va == user_va && b.kind != Kind::Shared)
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
    // RSC blocks are freed via `rsrc_free`, not `Mfree`; Shared blocks are
    // kernel structures the process never owned and must not release.
    if matches!(&blocks[pos], Some(bl) if bl.kind == Kind::Rsc || bl.kind == Kind::Shared) {
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
        .position(|b| matches!(b, Some(bl) if bl.m_start == m_start && bl.kind != Kind::Shared))
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
