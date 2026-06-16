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
//! One L3 spans 512 × 4 KiB = **2 MiB** of low-half VA. That fits
//! every reasonable Pexec'd test program (text+data+bss+stack ≪
//! 2 MiB), and the EC4 self-test maps a single page.  When real
//! source-port programs land in Phase 09+ and start needing larger
//! TPAs, this module will gain L2-block support (each L2 entry
//! covers a 2-MiB block; an L2 covers 1 GiB total).
//!
//! ## Concurrency
//!
//! Static page tables are mutated only under the BDOS giant lock
//! (or the boot-CPU equivalent). The walker is the only concurrent
//! reader — and it accesses physical memory that is held coherent
//! by the Inner-Shareable cacheable mapping (Phase 03 / 05 setup).

use super::MAX_PROCESSES;

// ─── Descriptor bits (subset; mirror `boot.rs` / `mmu.rs`) ──────────

const D_VALID: u64 = 1 << 0;
const D_TABLE: u64 = 1 << 1; // L0/L1/L2 next-level table
const D_PAGE_L3: u64 = 1 << 1; // L3 page (same bit, level-dependent)
const D_AF: u64 = 1 << 10;
const D_AP_USER_RW: u64 = 0b01 << 6; // EL1 + EL0 R/W
const D_AP_USER_RO: u64 = 0b11 << 6; // EL1 + EL0 RO
const D_SH_INNER: u64 = 0b11 << 8;
const D_ATTRIDX_NORMAL: u64 = 0 << 2;
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

/// One process's full L0/L1/L2/L3 chain. 16 KiB total.
#[repr(C, align(4096))]
struct UserTables {
    l0: UserTable,
    l1: UserTable,
    l2: UserTable,
    l3: UserTable,
}

/// Per-slot table storage. `MAX_PROCESSES = 16` × 16 KiB = 256 KiB
/// in BSS — the same order of magnitude as the kernel heap pre-
/// allocation, well within budget.
#[unsafe(no_mangle)]
static mut __USER_TABLES: [UserTables; MAX_PROCESSES] =
    [const { UserTables {
        l0: UserTable([0; 512]),
        l1: UserTable([0; 512]),
        l2: UserTable([0; 512]),
        l3: UserTable([0; 512]),
    }}; MAX_PROCESSES];

// ─── Public API ─────────────────────────────────────────────────────

/// Page-flag preset — selects what the page descriptor's AP / PXN /
/// UXN / AttrIdx bits resolve to. All presets share the
/// `VALID | PAGE | AF | SH_INNER | ATTRIDX_NORMAL | PXN` core; only
/// the per-EL0-permission bits change.
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
}

impl PageFlags {
    fn descriptor_bits(self) -> u64 {
        let core = D_VALID | D_PAGE_L3 | D_AF | D_SH_INNER | D_ATTRIDX_NORMAL | D_PXN;
        match self {
            Self::UserRwExec => core | D_AP_USER_RW,
            Self::UserRoExec => core | D_AP_USER_RO,
            Self::UserRwNx => core | D_AP_USER_RW | D_UXN,
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
    /// VA is outside the L3-covered range `[0, 2 MiB)`. Will be
    /// lifted in a future revision that grows the per-process
    /// mapping to include L2 blocks.
    VaOutOfRange,
}

/// Zero a slot's tables and wire the chain `L0[0] → L1[0] → L2[0]
/// → L3`. Idempotent — calling twice on the same slot is safe (the
/// second call clears any prior [`map_page`] entries, which is
/// what `Pexec` wants on slot reuse).
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
    for slot_entry in tables.l3.0.iter_mut() {
        *slot_entry = 0;
    }

    // Wire the chain. All three intermediate descriptors are
    // table-type pointers to the next level's phys.
    let l1_pa = va_to_pa((&raw const tables.l1) as u64);
    let l2_pa = va_to_pa((&raw const tables.l2) as u64);
    let l3_pa = va_to_pa((&raw const tables.l3) as u64);

    tables.l0.0[0] = l1_pa | D_VALID | D_TABLE;
    tables.l1.0[0] = l2_pa | D_VALID | D_TABLE;
    tables.l2.0[0] = l3_pa | D_VALID | D_TABLE;

    Ok(())
}

/// Install one 4-KiB page mapping in `slot`'s L3.
///
/// Both `va` and `pa` must be 4-KiB-aligned, and `va` must be in
/// `[0, 0x200000)` (the range covered by one L3 table).
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
    if va >= 0x200000 {
        return Err(MapError::VaOutOfRange);
    }
    let idx = (va >> 12) as usize; // L3 index, 0..511.
    // SAFETY: caller contract.
    let tables = unsafe { &mut (*&raw mut __USER_TABLES)[slot] };
    tables.l3.0[idx] = pa | flags.descriptor_bits();
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
    // SAFETY: writing TTBR0_EL1 is permitted at EL1; ISB is the
    // ARM ARM K11.5 mandated context-synchronization event.
    unsafe {
        core::arch::asm!(
            "msr ttbr0_el1, {ttbr0}",
            "isb",
            ttbr0 = in(reg) l0_pa,
            options(nostack, preserves_flags),
        );
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
