//! Per-process page tables — Phase 06 D8.
//!
//! When `Pexec` (or the EC4 self-test) prepares a process for EL0
//! execution, it needs an `L0` table to install in `TTBR0_EL1` so
//! the user's low-half VAs translate to the physical pages BDOS
//! allocated for the process. This module provides:
//!
//!   * Static per-slot storage for four page tables (`L0`/`L1`/
//!     `L2`/`L3`, 4 KiB each) → 16 KiB per process slot, 256 KiB
//!     total in BSS for [`super::MAX_PROCESSES`] = 16 slots.
//!   * [`init_slot_tables`] — zero the slot's tables and wire
//!     `L0[0] → L1[0] → L2[0] → L3` so any subsequent
//!     [`map_page`] call only has to fill in the L3 leaf.
//!   * [`map_page`] — install one 4-KiB page descriptor into a
//!     slot's L3, with [`PageFlags`] encoding user RW / RO and
//!     EL0 exec / NX as needed.
//!   * [`l0_pa_for`] — the phys address Pexec hands to
//!     [`install_ttbr0`].
//!   * [`install_ttbr0`] — `MSR TTBR0_EL1, …; ISB` for the boot
//!     CPU. Deliberately **no TLBI** (matches the EC4-Spike
//!     finding that `tlbi vmalle1` after a TTBR0 write wedges
//!     QEMU `aarch64-virt` cortex-a72 — Phase-06-D6 will revisit
//!     this when context-switch TLBI semantics actually matter).
//!
//! ## Coverage
//!
//! One L3 spans 512 × 4 KiB = **2 MiB** of low-half VA. To hold a
//! multi-MiB per-process TPA (a real compiled libc/gemlib app plus a
//! heap and a proper stack), each slot provisions [`USER_L3_COUNT`]
//! contiguous L3 tables wired under `L2[0..USER_L3_COUNT]`, so the
//! per-process low-half mapping capacity is
//! [`USER_TPA_MAX_VA`] = `USER_L3_COUNT × 2 MiB` (32 MiB today: a 4-MiB
//! TPA plus a 28-MiB EL0 heap window into which GEMDOS `Malloc` blocks,
//! the AES `.RSC` blob and the published screen memory are mapped per
//! process — see
//! [`USER_L3_COUNT`] and [`super::usermem`]). The
//! loader maps every 4-KiB page of the TPA individually (all at
//! 4-KiB granularity — the basepage/text/data at the bottom need it),
//! so no 2-MiB block descriptors are used. The EC4 self-test maps a
//! single page and is unaffected. Growing the cap further only means
//! bumping [`USER_L3_COUNT`] (and the pool-backed TPA size in
//! [`super::arena`]).
//!
//! ## Concurrency
//!
//! Static page tables are mutated only under the BDOS giant lock
//! (or the boot-CPU equivalent). The walker is the only concurrent
//! reader — and it accesses physical memory that is held coherent
//! by the Inner-Shareable cacheable mapping (Phase 03 / 05 setup).

use super::MAX_PROCESSES;

// ─── Per-process low-half mapping capacity ──────────────────────────

/// Number of L3 tables provisioned per process slot. Each L3 covers
/// 512 × 4 KiB = 2 MiB, so the per-process low-half VA that can be
/// mapped is `USER_L3_COUNT × 2 MiB`. The loader maps only the low
/// [`super::arena::TPA_SIZE`] (4 MiB, two L3s) as the program's TPA —
/// image + BSS + heap + stack; the remaining L3s are the **per-process
/// EL0 heap window** ([`super::usermem`]) into which GEMDOS
/// `Malloc`/`Mxalloc` blocks *and* the AES `.RSC` blob are mapped on
/// demand (ADR-021), so an EL0 app's `Mxalloc`/`rsrc_gaddr` returns
/// user-dereferenceable pointers — it cannot read the kernel high-half
/// VAs the desktop/selftest path uses.
///
/// `16` L3s ⇒ `32 MiB` mappable VA: the 4-MiB TPA plus a **28-MiB heap
/// window**, a real `malloc` arena for a compiled app. Bump this to
/// enlarge the window; the BSS cost is
/// `MAX_PROCESSES × (3 + USER_L3_COUNT) × 4 KiB` (= 1216 KiB here — the
/// same order as the kernel heap pre-allocation).
///
/// It was `10` (a 16-MiB window) until XBIOS `Setscreen` gained real page
/// flipping. `Physbase`/`Logbase` now publish the display's **whole** screen
/// memory — both stacked screens, so a program can compute the hidden page
/// and hand it back to `Setscreen` (`crate::xbios::screen`) — and at
/// 1920×1080×4 that is 15.8 MiB of the window in one block. With 10 L3s a
/// program that took the screen address had ~184 KiB of window left for
/// `Malloc`; 16 leaves it ~12 MiB. The screen mapping is a real user of the
/// window, so the window has to be sized for it.
pub const USER_L3_COUNT: usize = 16;

/// Low-half VA span covered by one L3 table (2 MiB).
const L3_SPAN: u64 = 512 * 4096;

/// One past the highest low-half VA the per-process tables can map.
/// [`map_page`] rejects any VA at or above this.
pub const USER_TPA_MAX_VA: u64 = USER_L3_COUNT as u64 * L3_SPAN;

// ─── Descriptor bits (subset; mirror `boot.rs` / `mmu.rs`) ──────────

const D_VALID: u64 = 1 << 0;
const D_TABLE: u64 = 1 << 1; // L0/L1/L2 next-level table
const D_PAGE_L3: u64 = 1 << 1; // L3 page (same bit, level-dependent)
const D_AF: u64 = 1 << 10;
const D_AP_USER_RW: u64 = 0b01 << 6; // EL1 + EL0 R/W
const D_AP_USER_RO: u64 = 0b11 << 6; // EL1 + EL0 RO
const D_SH_INNER: u64 = 0b11 << 8;
/// Non-shareable. Only used with [`D_ATTRIDX_NC`]: Normal Non-cacheable
/// memory is architecturally treated as Outer Shareable whatever the SH
/// field says, so this mirrors the kernel's own Non-Cacheable framebuffer
/// mapping (`kernel::arch::aarch64::mmu::remap_normal_nc`) rather than
/// inventing a second spelling for the same thing.
const D_SH_NON: u64 = 0b00 << 8;
const D_ATTRIDX_NORMAL: u64 = 0 << 2;
/// AttrIdx 2 = Normal **Non-Cacheable** (MAIR `0x44`, programmed once by
/// the boot stub — `kernel::arch::aarch64::boot::MAIR_VALUE`). The
/// attribute the framebuffer carries for memory shared with a DMA master
/// the ARM caches are not coherent with.
const D_ATTRIDX_NC: u64 = 2 << 2;
const D_PXN: u64 = 1 << 53; // EL1 must not execute
const D_UXN: u64 = 1 << 54; // EL0 must not execute

/// Phase-05 high-half offset. Mirrors the same constant in
/// `kernel::arch::aarch64::mmu`; per-process page tables live in
/// the high-half kernel image and we need their phys addresses for
/// the L0/L1/L2/L3 chain pointers and for `TTBR0_EL1`.
const VA_OFFSET: u64 = 0xFFFF_0000_0000_0000;

#[inline(always)]
const fn va_to_pa(va: u64) -> u64 {
    va.wrapping_sub(VA_OFFSET)
}

// ─── Static per-slot page tables ────────────────────────────────────

/// 512-entry, 4-KiB-aligned translation table page.
#[repr(C, align(4096))]
struct UserTable([u64; 512]);

/// One process's full L0/L1/L2 chain plus [`USER_L3_COUNT`] leaf L3
/// tables. `(3 + USER_L3_COUNT) × 4 KiB` per slot.
#[repr(C, align(4096))]
struct UserTables {
    l0: UserTable,
    l1: UserTable,
    l2: UserTable,
    l3: [UserTable; USER_L3_COUNT],
}

/// Per-slot table storage. `MAX_PROCESSES = 16` × `(3 + USER_L3_COUNT)`
/// × 4 KiB (= 384 KiB at `USER_L3_COUNT = 3`) in BSS — the same order
/// of magnitude as the kernel heap pre-allocation, well within budget.
/// Only the page tables live here; the TPA bytes they map are carved
/// per-process from the BDOS pool (see [`super::arena`]).
#[unsafe(no_mangle)]
static mut __USER_TABLES: [UserTables; MAX_PROCESSES] =
    [const { UserTables {
        l0: UserTable([0; 512]),
        l1: UserTable([0; 512]),
        l2: UserTable([0; 512]),
        l3: [const { UserTable([0; 512]) }; USER_L3_COUNT],
    }}; MAX_PROCESSES];

// ─── Public API ─────────────────────────────────────────────────────

/// Page-flag preset — selects what the page descriptor's AP / PXN /
/// UXN / AttrIdx bits resolve to. The cacheable presets share the
/// `VALID | PAGE | AF | SH_INNER | ATTRIDX_NORMAL | PXN` core and differ
/// only in the per-EL0-permission bits; the `*Uncached` presets swap that
/// core for `SH_NON | ATTRIDX_NC` (Normal Non-Cacheable) because the
/// region behind them is shared with a non-coherent DMA master.
///
/// `PXN = 1` is set on every preset because the kernel never
/// executes from user pages — even if a user maps a region readable
/// to EL0, EL1 instruction fetches from that VA fault.
#[derive(Copy, Clone, Debug)]
pub enum PageFlags {
    /// User Read/Write, EL0 executable. Used by EC4-Spike (single
    /// page combining code + data) and by source-port test
    /// scaffolding before proper segment-aware Pexec exists.
    UserRwExec,
    /// User Read-Only, EL0 executable. The natural attribute for a
    /// program's `.text` segment.
    UserRoExec,
    /// User Read/Write, **not** executable. Data, BSS, stack.
    UserRwNx,
    /// User Read-Only, **not** executable. The AArch64 spelling of
    /// FreeMiNT's [`tos_types::ProtMode::Pr`] (`PROT_PR`, "any read
    /// OK, no write"): what
    /// [`super::usermem::expose`] uses to publish a kernel structure
    /// a program only *reads* (the `Getbpb` BPB, the `Getmpb` memory
    /// descriptor) into that program's address space.
    UserRoNx,
    /// User Read/Write, **not** executable, **Normal Non-Cacheable**
    /// (AttrIdx 2). For memory the CPU shares with a DMA master it is not
    /// cache-coherent with — the VideoCore framebuffer, whose display
    /// engine scans DRAM out from beyond the ARM caches
    /// (`docs/hardware/mailbox-framebuffer.md` §6). An EL0 program cannot
    /// be asked to clean the D-cache after every store, so the pages it is
    /// handed by `Physbase`/`Logbase` carry the attribute that makes its
    /// writes reach DRAM unaided ([`super::usermem::MemAttr::Uncached`]).
    UserRwNxUncached,
    /// User Read-Only, **not** executable, **Normal Non-Cacheable**
    /// (AttrIdx 2). The read-only counterpart of
    /// [`Self::UserRwNxUncached`], for a DMA-shared region a program is
    /// only allowed to read.
    UserRoNxUncached,
}

impl PageFlags {
    fn descriptor_bits(self) -> u64 {
        let core = D_VALID | D_PAGE_L3 | D_AF | D_SH_INNER | D_ATTRIDX_NORMAL | D_PXN;
        let nc_core = D_VALID | D_PAGE_L3 | D_AF | D_SH_NON | D_ATTRIDX_NC | D_PXN;
        match self {
            Self::UserRwExec => core | D_AP_USER_RW,
            Self::UserRoExec => core | D_AP_USER_RO,
            Self::UserRwNx => core | D_AP_USER_RW | D_UXN,
            Self::UserRoNx => core | D_AP_USER_RO | D_UXN,
            Self::UserRwNxUncached => nc_core | D_AP_USER_RW | D_UXN,
            Self::UserRoNxUncached => nc_core | D_AP_USER_RO | D_UXN,
        }
    }
}

/// Errors from the per-process page-table builder.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MapError {
    /// Slot index ≥ [`MAX_PROCESSES`].
    InvalidSlot,
    /// VA is not 4-KiB aligned.
    UnalignedVa,
    /// Phys is not 4-KiB aligned.
    UnalignedPa,
    /// VA is outside the per-process mapping range
    /// `[0, USER_TPA_MAX_VA)`. Enlarge by bumping [`USER_L3_COUNT`].
    VaOutOfRange,
}

/// Zero a slot's tables and wire the chain `L0[0] → L1[0] → L2[i]
/// → L3[i]` for every `i` in `0..USER_L3_COUNT`. Idempotent — calling
/// twice on the same slot is safe (the second call clears any prior
/// [`map_page`] entries, which is what `Pexec` wants on slot reuse).
///
/// # Safety
///
/// Caller holds the BDOS giant lock (or is the boot CPU pre-trap).
/// `slot` must not currently be the active TTBR0 — if it is, the
/// caller must invalidate the TLB before resuming user execution.
//
// `clippy::deref_addrof` would simplify `&mut *(&raw mut X)` to
// `&X`, but `&` on a `static mut` is a hard error in Rust 2024 — the
// `&raw mut` form is the only sound pattern. (Same dispensation as
// `kernel::arch::aarch64::mmu::build`.)
#[allow(clippy::deref_addrof)]
pub unsafe fn init_slot_tables(slot: usize) -> Result<(), MapError> {
    if slot >= MAX_PROCESSES {
        return Err(MapError::InvalidSlot);
    }
    // SAFETY: caller's BDOS-lock contract guarantees exclusive access.
    let tables = unsafe { &mut (*&raw mut __USER_TABLES)[slot] };

    // Zero every entry. `iter_mut` keeps clippy happy versus an
    // index-based loop.
    for slot_entry in tables.l0.0.iter_mut() {
        *slot_entry = 0;
    }
    for slot_entry in tables.l1.0.iter_mut() {
        *slot_entry = 0;
    }
    for slot_entry in tables.l2.0.iter_mut() {
        *slot_entry = 0;
    }
    for l3 in tables.l3.iter_mut() {
        for slot_entry in l3.0.iter_mut() {
            *slot_entry = 0;
        }
    }

    // Wire the chain. `L0[0] → L1`, `L1[0] → L2`, and one `L2[i] →
    // L3[i]` table descriptor per leaf table (each L3 covers 2 MiB of
    // low-half VA). All intermediate descriptors are table-type
    // pointers to the next level's phys.
    let l1_pa = va_to_pa((&raw const tables.l1) as u64);
    let l2_pa = va_to_pa((&raw const tables.l2) as u64);
    tables.l0.0[0] = l1_pa | D_VALID | D_TABLE;
    tables.l1.0[0] = l2_pa | D_VALID | D_TABLE;
    for (i, l3) in tables.l3.iter().enumerate() {
        let l3_pa = va_to_pa((l3 as *const UserTable) as u64);
        tables.l2.0[i] = l3_pa | D_VALID | D_TABLE;
    }

    Ok(())
}

/// Install one 4-KiB page mapping in `slot`'s L3 tables.
///
/// Both `va` and `pa` must be 4-KiB-aligned, and `va` must be in
/// `[0, USER_TPA_MAX_VA)` (the range covered by the slot's
/// [`USER_L3_COUNT`] L3 tables). The VA selects the leaf table
/// (`va >> 21`) and the entry within it (`(va >> 12) & 511`).
///
/// Caller is expected to have called [`init_slot_tables`] for this
/// slot first.
///
/// # Safety
///
/// Caller holds the BDOS giant lock; the page at `pa` must be
/// exclusively owned by the process whose slot this is (typically
/// allocated via [`crate::bdos::mem::traps::mxalloc`] before this
/// call).
#[allow(clippy::deref_addrof)] // see `init_slot_tables`
pub unsafe fn map_page(
    slot: usize,
    va: u64,
    pa: u64,
    flags: PageFlags,
) -> Result<(), MapError> {
    if slot >= MAX_PROCESSES {
        return Err(MapError::InvalidSlot);
    }
    if !va.is_multiple_of(0x1000) {
        return Err(MapError::UnalignedVa);
    }
    if !pa.is_multiple_of(0x1000) {
        return Err(MapError::UnalignedPa);
    }
    if va >= USER_TPA_MAX_VA {
        return Err(MapError::VaOutOfRange);
    }
    let l3_table = (va >> 21) as usize; // which 2-MiB leaf table.
    let idx = ((va >> 12) & 0x1FF) as usize; // L3 index within it, 0..511.
    // SAFETY: caller contract.
    let tables = unsafe { &mut (*&raw mut __USER_TABLES)[slot] };
    tables.l3[l3_table].0[idx] = pa | flags.descriptor_bits();
    Ok(())
}

/// Phys address of `slot`'s L0 table — the value `Pexec` writes to
/// `TTBR0_EL1` via [`install_ttbr0`].
///
/// Returns `0` for an out-of-range slot index (defensive — callers
/// would normally check the slot before reaching here).
#[allow(clippy::deref_addrof)] // see `init_slot_tables`
pub fn l0_pa_for(slot: usize) -> u64 {
    if slot >= MAX_PROCESSES {
        return 0;
    }
    // Raw address-of a `static mut` does not require `unsafe` (no
    // deref); read-only, no aliasing.
    let tables_ptr = (&raw const __USER_TABLES) as *const UserTables;
    // Cast straight from base + slot * size_of::<UserTables>() to
    // avoid any compiler-introduced indirection through the L1 / L2
    // / L3 fields. `&raw const tables.l0` produced surprising
    // results during Phase-06 D3 bring-up (the address of L1 was
    // returned in some build configurations); the manual offset
    // computation is unambiguous.
    let base = tables_ptr as u64;
    let offset = (slot as u64) * (core::mem::size_of::<UserTables>() as u64);
    va_to_pa(base + offset)
}

/// Install `l0_pa` into `TTBR0_EL1` and synchronise the pipeline.
///
/// **No TLBI on the install path.** `tlbi vmalle1` immediately
/// before an `eret`-to-EL0 wedges QEMU `aarch64-virt cortex-a72`
/// (verified empirically in Phase 05 EC4 spike, reproduced again
/// in Phase 06 D6 bring-up). Callers that need to drop stale
/// translations from a *previous* slot must invalidate the TLB
/// **before** calling this function — typically by routing through
/// [`crate::arch::aarch64::user_entry::restore_boot_ttbr0`] (which
/// does include the TLBI safely, since it executes wholly at EL1
/// without an immediately-following `eret`-to-EL0). The Pterm
/// handler does exactly that on every process exit, so any later
/// `Pexec` install starts from an empty EL0 TLB.
///
/// **All processes run with ASID 0.** Per-process ASIDs would be the
/// architecturally correct way to distinguish the VA-aliased address
/// spaces (Phase-06 D3 loads every TPA at VA `0x100`), but QEMU's
/// cortex-a72 TCG model (11.0.0) *wedges on the ASID-tagged TTBR0
/// write itself* — verified: tagging EC4's install with a non-zero
/// ASID hangs at the `msr`/`isb`. The QUIRK-001 wedge is purely a
/// QEMU artifact (real PIPT caches never alias), so ASIDs are not
/// needed to fix it; they are deferred to real-hardware multitasking
/// (TLB efficiency). See QUIRK-001 in `docs/MEMORY.md`.
///
/// # Safety
///
/// Caller is the boot CPU and is about to either `eret` to EL0
/// (Pexec) or fall through to kernel code that does **not**
/// touch low-half VAs.
pub unsafe fn install_ttbr0(l0_pa: u64) {
    // SAFETY: writing TTBR0_EL1 is permitted at EL1. The `dsb ish`
    // before the install is mandatory (ARM ARM D5.10.1): the slot's
    // page-table entries were just written through cacheable kernel
    // memory, and a translation-table walk may only rely on them after
    // a DSB makes the writes observable in the Inner-Shareable domain.
    // Without it, real hardware's walker reads stale/unordered table
    // memory and faults (QEMU's coherent walker tolerated the absence —
    // the real-Pi4 Instruction-Abort permission fault at vaddr 0x100).
    // ISB is the ARM ARM K11.5 context-synchronization event after the
    // TTBR write. No TLBI here (see the function docs — it wedges QEMU
    // before an eret-to-EL0; the TLB is flushed earlier in EL1 context).
    unsafe {
        core::arch::asm!(
            "dsb ish",
            "msr ttbr0_el1, {ttbr0}",
            "isb",
            ttbr0 = in(reg) l0_pa,
            options(nostack, preserves_flags),
        );
    }
}

/// Make freshly-installed user page-table entries (and the data they
/// map) observable to a subsequent EL0 access, **without** a TLB
/// invalidate.
///
/// The AES resource loader calls this after installing a per-process
/// `.RSC` mapping while the owning process's `TTBR0_EL1` is already
/// active (the app trapped via `svc`). The target VAs were previously
/// **invalid** (unmapped spare-window pages), so no stale TLB entry can
/// exist for them and no TLBI is required — which also avoids the
/// QUIRK-001 QEMU wedge that a TLBI immediately before an `eret`-to-EL0
/// triggers (see [`install_ttbr0`]). A `dsb ish` orders the cacheable
/// page-table and blob writes before the translation-table walker and
/// EL0 observe them; the trailing `isb` is the context-synchronisation
/// event so the handler's own EL1 view is consistent.
///
/// # Safety
///
/// Barrier-only; always permitted at EL1.
pub unsafe fn publish_user_mappings() {
    // SAFETY: DSB/ISB are unprivileged-safe barriers, permitted at EL1.
    unsafe {
        core::arch::asm!("dsb ish", "isb", options(nostack, preserves_flags));
    }
}

/// Make freshly-written program bytes at `[va, va + len)` observable to
/// instruction fetch: clean each D-cache line to the Point of Unification,
/// then invalidate the matching I-cache lines, with the barriers ARM ARM
/// B2.4.4 mandates for "data written to be executed". Required on real
/// hardware after the loader copies code into a page; QEMU has no caches,
/// so it is effectively a no-op there.
///
/// `va` is a currently-mapped (kernel high-half) VA aliasing the program
/// page's physical address. The Cortex-A72 caches are PIPT, so by-VA
/// maintenance via any alias of the same PA affects the correct physical
/// lines — the program later fetches the same PA through its EL0 VA.
///
/// # Safety
///
/// `[va, va + len)` must be a mapped, readable range in the active EL1
/// translation regime.
pub unsafe fn sync_code_cache(va: usize, len: usize) {
    if len == 0 {
        return;
    }
    // D/I-cache minimum line sizes from CTR_EL0: the fields are log2 of the
    // line size in 32-bit words, so bytes = 4 << field (ARM ARM CTR_EL0).
    let ctr: u64;
    // SAFETY: CTR_EL0 is read-only and readable at EL1.
    unsafe {
        core::arch::asm!("mrs {}, ctr_el0", out(reg) ctr,
            options(nomem, nostack, preserves_flags));
    }
    let dline = 4usize << ((ctr >> 16) & 0xF); // DminLine
    let iline = 4usize << (ctr & 0xF); // IminLine
    let end = va + len;

    // Clean the D-cache to PoU so the writes reach the point instruction
    // fetch observes, then order before the I-cache maintenance.
    let mut a = va & !(dline - 1);
    while a < end {
        // SAFETY: `a` is within the mapped range; DC CVAU is permitted at EL1.
        unsafe {
            core::arch::asm!("dc cvau, {}", in(reg) a, options(nostack, preserves_flags));
        }
        a += dline;
    }
    // SAFETY: barrier-only.
    unsafe {
        core::arch::asm!("dsb ish", options(nostack, preserves_flags));
    }

    // Invalidate the I-cache to PoU so stale lines cannot be fetched.
    let mut a = va & !(iline - 1);
    while a < end {
        // SAFETY: `a` is within the mapped range; IC IVAU is permitted at EL1.
        unsafe {
            core::arch::asm!("ic ivau, {}", in(reg) a, options(nostack, preserves_flags));
        }
        a += iline;
    }
    // SAFETY: barrier + context synchronisation after I-cache maintenance.
    unsafe {
        core::arch::asm!("dsb ish", "isb", options(nostack, preserves_flags));
    }
}

/// Clean **and invalidate** the D-cache lines covering `[va, va + len)` to
/// the point of coherency, so that afterwards no cached copy of that
/// physical memory exists in this PE's caches — neither a dirty one that
/// could later be written back over somebody else's data, nor a clean one
/// that could satisfy a stale read.
///
/// This is the maintenance the ARM ARM requires before the same physical
/// memory is also accessed through an alias with **mismatched cacheability**
/// attributes (ARM ARM, section "Mismatched memory attributes"): the loss
/// of coherency such a mismatch permits is avoided by ensuring the region
/// is not held in any cache across the mismatched accesses.
///
/// [`super::usermem::expose_attr`] calls it when it publishes a region as
/// Normal Non-Cacheable to EL0 while the kernel keeps a Write-Back alias of
/// the same pages — the framebuffer.
///
/// `va` is a currently-mapped VA aliasing the region's physical address; the
/// Cortex-A72 caches are PIPT, so by-VA maintenance through any alias hits
/// the correct physical lines.
///
/// # Safety
///
/// `[va, va + len)` must be a mapped range in the active EL1 translation
/// regime.
pub unsafe fn clean_invalidate_dcache_range(va: usize, len: usize) {
    if len == 0 {
        return;
    }
    // D-cache minimum line size from CTR_EL0.DminLine: log2 of the line
    // size in 32-bit words, so bytes = 4 << field (ARM ARM CTR_EL0).
    let ctr: u64;
    // SAFETY: CTR_EL0 is read-only and readable at EL1.
    unsafe {
        core::arch::asm!("mrs {}, ctr_el0", out(reg) ctr,
            options(nomem, nostack, preserves_flags));
    }
    let line = 4usize << ((ctr >> 16) & 0xF);
    let end = va + len;
    let mut a = va & !(line - 1);
    while a < end {
        // SAFETY: `a` is within the mapped range; DC CIVAC is permitted at EL1.
        unsafe {
            core::arch::asm!("dc civac, {}", in(reg) a, options(nostack, preserves_flags));
        }
        a += line;
    }
    // SAFETY: barrier-only. `sy` because the other observer is a DMA master
    // outside the Inner-Shareable domain, not just another PE.
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

/// Switch `TTBR0_EL1` to `l0_pa` **with** a TLB invalidate, for
/// callers that run wholly at EL1 and are *not* about to `eret`-to-
/// EL0 immediately. The Phase-05 EC4 spike found that pairing
/// `tlbi vmalle1` with an immediately-following `eret-to-EL0` wedges
/// QEMU `aarch64-virt cortex-a72`; the workaround is to keep the
/// TLBI in kernel context where the wedge does not occur.
///
/// Use this from the Pterm SVC handler when restoring a real
/// parent's address space (the resume-`eret` lands at EL1, not
/// EL0, so the wedge condition does not apply). For the
/// `eret-to-EL0` install path, [`install_ttbr0`] is the right
/// primitive — but the caller must have flushed the TLB **earlier**
/// in kernel context (e.g. via [`crate::bdos::process::pterm`]'s
/// boot-table restore or via Phase 07 `Pexec`'s pre-install flush
/// step).
///
/// # Safety
///
/// Caller is at EL1, single-CPU. Caller must not be holding a
/// low-half VA reference whose translation depends on the *old*
/// TTBR0 — those become invalid the instant the new TTBR0 is
/// observed.
pub unsafe fn switch_ttbr0_kernel(l0_pa: u64) {
    // SAFETY: writing TTBR0_EL1 + TLBI is permitted at EL1; the
    // DSB pair brackets the invalidate per ARM ARM B5.2.
    unsafe {
        core::arch::asm!(
            "dsb ishst",
            "msr ttbr0_el1, {ttbr0}",
            "tlbi vmalle1",
            "dsb ish",
            "isb",
            ttbr0 = in(reg) l0_pa,
            options(nostack, preserves_flags),
        );
    }
}
