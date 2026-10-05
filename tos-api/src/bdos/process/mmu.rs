//! User page tables — one shared address space, one root per process
//! (ARCH03 stage 1, ADR-025 draft in `docs/analysis/arch03-implementation-plan.md`
//! §1, §3.1, §3.2).
//!
//! ## Model
//!
//! Every user address is its own physical address (**VA = PA**) — the two
//! BDOS pools, the published kernel structures (ADR-023) and the screen
//! memory appear at the same address in every process, so a pointer means the
//! same thing in every process, like on EmuTOS' flat machine and under
//! FreeMiNT with memory protection off. The single exception is **page 0**,
//! `[0, 0x1000)`: each process has its own (the classic system variables, the
//! OSHEADER, the `$5A0` cookie jar, the upcall trampolines, the `Keytbl` area
//! and the KEYS02 vector table — plan E3), backed by a per-slot kernel page
//! ([`page0_kva`]).
//!
//! FreeMiNT keeps "one of these per process" over one logical address space
//! (`references/freemint/sys/arch/mprot030.c:18,76-80`); stage 1 is its
//! `no_mem_prot` form: every process root shares all tables below it except
//! the ones that lead to page 0.
//!
//! ## Tables
//!
//! ```text
//!   per slot:  L0 ─► L1 ─► L2 (first GiB) ─► L3 for [0, 2 MiB) ─► page 0 (private)
//!                    │       └ entries 1..511: copies of the shared GiB-0 template
//!                    └ entries 1..3: the shared L2 tables of GiB 1..3
//!   shared:    L2 templates/tables for GiB 0..3, the [0, 2 MiB) L3 template,
//!              and a fixed set of L3 tables ([`SHARED_L3_COUNT`]) for 2-MiB
//!              regions that are only partly user memory.
//! ```
//!
//! Only VA `[0, 4 GiB)` is coverable (four L1 entries); every user address is
//! below `0x8000_0000` anyway (plan invariant I2, `TOS_MD_MAX_END`). Entries of
//! the GiB-0 L2 template and of the `[0, 2 MiB)` L3 template are copied into
//! every slot's own tables — at [`init_slot_tables`] and on every later
//! change ([`map_shared`] writes the template and all sixteen copies) — so the
//! per-slot tables never fall behind the shared ones.
//!
//! The pools are entered once at boot ([`map_pools`], plan Frage 2 / E18: the
//! 0b spike and the Pi run confirmed boot-time mapping), with 2-MiB blocks
//! where a whole aligned 2-MiB region is pool and 4-KiB pages at the edges.
//! Later publications ([`map_shared`]) only ever turn **invalid** entries
//! valid, which needs no TLB invalidation (ARM ARM `RXCLRD` D8-7852 /
//! `IWZCBG` D8-7858, plan §3.2) — nothing in stage 1 is ever unmapped.
//!
//! ## Descriptor bits
//!
//! AArch64 stage-1 descriptor layout (ARM ARM DDI 0487 M.d, Figure D8-16 /
//! Table D8-52, verified in V0a): `AP[2:1]` = bits 7:6 (`01` EL1+EL0 RW, `11`
//! EL1+EL0 RO), `SH` = 9:8, `AF` = 10, `AttrIndx` = 4:2, `PXN` = 53, `UXN` =
//! 54; a level-2 entry with bit 1 clear is a 2-MiB block. `AF` is set in every
//! valid entry (the A72 has no hardware access-flag management, V0a C3).
//!
//! ## Concurrency
//!
//! Tables are mutated only under the BDOS giant lock (the single-CPU trap /
//! boot context). The walker is the only concurrent reader; a `dsb ish`
//! orders the writes before it ([`publish_user_mappings`],
//! [`install_ttbr0`]).

use core::sync::atomic::{AtomicUsize, Ordering};

use super::MAX_PROCESSES;

// ─── Geometry ────────────────────────────────────────────────────────

/// Bytes in one page (the 4-KiB granule).
pub const PAGE_SIZE: u64 = 4096;

/// VA span of one L3 table / one L2 block (2 MiB).
const L3_SPAN: u64 = 512 * PAGE_SIZE;

/// VA span of one L1 entry / one L2 table (1 GiB).
const L2_SPAN: u64 = 512 * L3_SPAN;

/// Number of 1-GiB regions the user tables cover (`[0, 4 GiB)`).
const GIB_COUNT: usize = 4;

/// One past the highest user VA the tables can map.
pub const USER_VA_LIMIT: u64 = GIB_COUNT as u64 * L2_SPAN;

/// L3 tables available for 2-MiB regions that are only partly user memory —
/// the edges of the pools, the screen memory (not 2-MiB aligned on the Pi,
/// 0b Q5), published kernel pages. Measured need (0b Q2, Plan Frage 14): the
/// pools take at most 2 per pool edge, the largest screen (1920×1080×4, two
/// pages) 9, the kernel publications 1–2. 64 tables = 256 KiB of BSS; the
/// per-process window tables this replaces were 1216 KiB.
pub const SHARED_L3_COUNT: usize = 64;

// ─── Descriptor bits ─────────────────────────────────────────────────

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

/// Output-address field of a 4-KiB page / table descriptor (bits 47:12).
const OA_MASK: u64 = 0x0000_FFFF_FFFF_F000;

/// Phase-05 high-half offset. Mirrors the same constant in
/// `kernel::arch::aarch64::mmu`; the user tables live in the high-half kernel
/// image and their physical addresses go into the descriptors and `TTBR0_EL1`.
const VA_OFFSET: u64 = 0xFFFF_0000_0000_0000;

#[inline(always)]
const fn va_to_pa(va: u64) -> u64 {
    va.wrapping_sub(VA_OFFSET)
}

// ─── Static tables ───────────────────────────────────────────────────

/// 512-entry, 4-KiB-aligned translation table page.
#[repr(C, align(4096))]
struct UserTable([u64; 512]);

impl UserTable {
    const fn zeroed() -> Self {
        UserTable([0; 512])
    }
}

/// One process slot's private tables: its root chain down to the L3 that
/// holds page 0. 16 KiB per slot (plan Frage 14).
#[repr(C, align(4096))]
struct SlotTables {
    l0: UserTable,
    l1: UserTable,
    /// L2 for VA `[0, 1 GiB)`: entry 0 → [`Self::l3`], entries 1..511 copied
    /// from [`SHARED_L2`]`[0]`.
    l2: UserTable,
    /// L3 for VA `[0, 2 MiB)`: entry 0 → the slot's page 0, entries 1..511
    /// copied from [`SHARED_L3_0`].
    l3: UserTable,
}

/// One process slot's page 0 — the data page user VA `[0, 0x1000)` maps to.
#[repr(C, align(4096))]
pub struct Page0(pub [u8; 4096]);

#[unsafe(no_mangle)]
static mut __USER_TABLES: [SlotTables; MAX_PROCESSES] = [const {
    SlotTables {
        l0: UserTable::zeroed(),
        l1: UserTable::zeroed(),
        l2: UserTable::zeroed(),
        l3: UserTable::zeroed(),
    }
}; MAX_PROCESSES];

/// The per-slot page-0 data pages.
static mut PAGE0: [Page0; MAX_PROCESSES] = [const { Page0([0; 4096]) }; MAX_PROCESSES];

/// The shared L2 tables: index 0 is the **template** for every slot's
/// GiB-0 L2 (its entry 0 is unused — it is the slot's own), 1..3 are the real
/// shared L2 tables of GiB 1..3 that every slot's L1 points at.
static mut SHARED_L2: [UserTable; GIB_COUNT] = [const { UserTable::zeroed() }; GIB_COUNT];

/// Template for every slot's `[0, 2 MiB)` L3 (entry 0 unused — page 0).
static mut SHARED_L3_0: UserTable = UserTable::zeroed();

/// L3 tables for partly-user 2-MiB regions, handed out in order.
static mut SHARED_L3: [UserTable; SHARED_L3_COUNT] =
    [const { UserTable::zeroed() }; SHARED_L3_COUNT];

/// Number of [`SHARED_L3`] tables handed out.
static SHARED_L3_USED: AtomicUsize = AtomicUsize::new(0);

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
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PageFlags {
    /// User Read/Write, EL0 executable — what every pool page and page 0
    /// carry (plan §3.2: `AP=01`, `PXN=1`, `UXN=0`; LDG and `Pexec` load
    /// code into ordinary pool blocks).
    UserRwExec,
    /// User Read-Only, EL0 executable.
    UserRoExec,
    /// User Read/Write, **not** executable.
    UserRwNx,
    /// User Read-Only, **not** executable. The AArch64 spelling of
    /// FreeMiNT's [`tos_types::ProtMode::Pr`] (`PROT_PR`, "any read
    /// OK, no write"): what
    /// [`super::usermem::expose`] uses to publish a kernel structure
    /// a program only *reads* (the `Getbpb` BPB, the `Getmpb` memory
    /// descriptor).
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
    /// The attribute bits of an L3 page descriptor with these flags.
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

    /// The attribute bits of an L2 **block** descriptor with these flags:
    /// the page bits with descriptor bit 1 clear (ARM ARM Figure D8-16).
    fn block_bits(self) -> u64 {
        self.descriptor_bits() & !D_PAGE_L3
    }
}

/// Errors from the user page-table builder.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MapError {
    /// Slot index ≥ [`MAX_PROCESSES`].
    InvalidSlot,
    /// VA is not 4-KiB aligned.
    UnalignedVa,
    /// Phys is not 4-KiB aligned.
    UnalignedPa,
    /// VA is outside the range the call may map: for [`map_page`] the
    /// slot-private `[0, 2 MiB)`, for [`map_shared`] `[0x1000,
    /// USER_VA_LIMIT)`.
    VaOutOfRange,
    /// A page or block is already mapped there with different attributes.
    /// Stage 1 never changes a valid entry (that would need a TLB
    /// invalidation, plan §3.2), so the request is refused.
    Conflict,
    /// [`SHARED_L3_COUNT`] tables are all in use.
    NoTable,
}

// SAFETY (all accessors below): the statics are mutated only under the BDOS
// giant lock (single CPU, IRQs masked in the trap path / boot CPU before any
// trap), the contract every caller states.

#[allow(clippy::deref_addrof)]
unsafe fn slot_tables(slot: usize) -> &'static mut SlotTables {
    // SAFETY: caller's lock contract; `slot` bounds-checked by the caller.
    unsafe { &mut (*&raw mut __USER_TABLES)[slot] }
}

#[allow(clippy::deref_addrof)]
unsafe fn shared_l2(gib: usize) -> &'static mut UserTable {
    // SAFETY: caller's lock contract; `gib < GIB_COUNT` checked by the caller.
    unsafe { &mut (*&raw mut SHARED_L2)[gib] }
}

#[allow(clippy::deref_addrof)]
unsafe fn shared_l3_0() -> &'static mut UserTable {
    // SAFETY: caller's lock contract.
    unsafe { &mut *&raw mut SHARED_L3_0 }
}

#[inline]
fn table_pa(t: &UserTable) -> u64 {
    va_to_pa(t as *const UserTable as u64)
}

/// Kernel VA of `slot`'s page 0 — the page user VA `[0, 0x1000)` maps to in
/// that slot's address space. The loader writes the system variables, the
/// trampolines and the vector table here; the kernel reads and updates them
/// here (`$4BA` `_hz_200`, `$484` `conterm`, the `kbdvec` table, the userdef
/// scratch). `None` for an out-of-range slot.
pub fn page0_kva(slot: usize) -> Option<*mut u8> {
    if slot >= MAX_PROCESSES {
        return None;
    }
    // Raw address of a `static mut` element: no reference is formed.
    let base = (&raw mut PAGE0) as *mut Page0;
    // SAFETY: `slot < MAX_PROCESSES`, in bounds of the array.
    Some(unsafe { base.add(slot) } as *mut u8)
}

/// [`page0_kva`] as a mutable byte page.
///
/// # Safety
///
/// Caller holds the BDOS giant lock for the borrow's lifetime and does not
/// alias it.
pub unsafe fn page0_bytes(slot: usize) -> Option<&'static mut [u8; 4096]> {
    let p = page0_kva(slot)?;
    // SAFETY: `p` addresses the slot's 4-KiB page; caller's aliasing contract.
    Some(unsafe { &mut *(p as *mut [u8; 4096]) })
}

/// Build `slot`'s root: `L0[0] → L1`, `L1[0] → L2`, `L1[1..4]` → the shared
/// GiB-1..3 L2 tables, `L2[0] → L3`, `L2[1..]` = the GiB-0 template, `L3[0]`
/// = the slot's page 0 (`UserRwExec`), `L3[1..]` = the `[0, 2 MiB)` template.
/// Idempotent; the page-0 *contents* are not touched (the loader writes them).
///
/// # Safety
///
/// Caller holds the BDOS giant lock (or is the boot CPU pre-trap).
/// `slot` must not currently be the active TTBR0 — if it is, the
/// caller must invalidate the TLB before resuming user execution.
pub unsafe fn init_slot_tables(slot: usize) -> Result<(), MapError> {
    if slot >= MAX_PROCESSES {
        return Err(MapError::InvalidSlot);
    }
    let page0_pa = va_to_pa(page0_kva(slot).ok_or(MapError::InvalidSlot)? as u64);
    // SAFETY: caller's lock contract; slot bounds-checked.
    let t = unsafe { slot_tables(slot) };
    // SAFETY: caller's lock contract.
    let (tmpl_l2, tmpl_l3) = unsafe { (&shared_l2(0).0, &shared_l3_0().0) };

    t.l0.0 = [0; 512];
    t.l0.0[0] = table_pa(&t.l1) | D_VALID | D_TABLE;

    t.l1.0 = [0; 512];
    t.l1.0[0] = table_pa(&t.l2) | D_VALID | D_TABLE;
    for gib in 1..GIB_COUNT {
        // SAFETY: caller's lock contract; `gib < GIB_COUNT`.
        let l2_pa = table_pa(unsafe { shared_l2(gib) });
        t.l1.0[gib] = l2_pa | D_VALID | D_TABLE;
    }

    t.l2.0 = *tmpl_l2;
    t.l2.0[0] = table_pa(&t.l3) | D_VALID | D_TABLE;

    t.l3.0 = *tmpl_l3;
    t.l3.0[0] = page0_pa | PageFlags::UserRwExec.descriptor_bits();
    Ok(())
}

/// Install one 4-KiB page in `slot`'s **private** `[0, 2 MiB)` L3 — page 0
/// by default; the EC4 self-test maps its single code page at VA 0 this way.
/// Shared memory goes through [`map_shared`] instead.
///
/// # Safety
///
/// Caller holds the BDOS giant lock; [`init_slot_tables`] ran for the slot.
pub unsafe fn map_page(
    slot: usize,
    va: u64,
    pa: u64,
    flags: PageFlags,
) -> Result<(), MapError> {
    if slot >= MAX_PROCESSES {
        return Err(MapError::InvalidSlot);
    }
    if !va.is_multiple_of(PAGE_SIZE) {
        return Err(MapError::UnalignedVa);
    }
    if !pa.is_multiple_of(PAGE_SIZE) {
        return Err(MapError::UnalignedPa);
    }
    if va >= L3_SPAN {
        return Err(MapError::VaOutOfRange);
    }
    // SAFETY: caller contract.
    let t = unsafe { slot_tables(slot) };
    t.l3.0[(va / PAGE_SIZE) as usize] = (pa & OA_MASK) | flags.descriptor_bits();
    Ok(())
}

/// Write entry `idx` of the GiB-`gib` L2: for GiB 0 into the template and
/// into every slot's copy, otherwise into the shared table itself.
///
/// # Safety
///
/// Caller's lock contract; `gib < GIB_COUNT`, `idx < 512`, and `idx != 0`
/// for GiB 0 (that entry is each slot's own).
unsafe fn set_l2(gib: usize, idx: usize, desc: u64) {
    // SAFETY: caller contract.
    unsafe {
        shared_l2(gib).0[idx] = desc;
        if gib == 0 {
            for slot in 0..MAX_PROCESSES {
                slot_tables(slot).l2.0[idx] = desc;
            }
        }
    }
}

/// Write entry `idx` (≥ 1) of the `[0, 2 MiB)` L3: template + every slot.
///
/// # Safety
///
/// Caller's lock contract; `1 <= idx < 512`.
unsafe fn set_l3_0(idx: usize, desc: u64) {
    // SAFETY: caller contract.
    unsafe {
        shared_l3_0().0[idx] = desc;
        for slot in 0..MAX_PROCESSES {
            slot_tables(slot).l3.0[idx] = desc;
        }
    }
}

/// The L3 table covering the 2-MiB region that starts at `region` (≥ 2 MiB),
/// creating it from [`SHARED_L3`] if the L2 entry is still invalid.
///
/// # Safety
///
/// Caller's lock contract; `L3_SPAN <= region < USER_VA_LIMIT`, aligned.
#[allow(clippy::deref_addrof)]
unsafe fn shared_l3_for(region: u64) -> Result<&'static mut UserTable, MapError> {
    let gib = (region / L2_SPAN) as usize;
    let idx = ((region % L2_SPAN) / L3_SPAN) as usize;
    // SAFETY: caller contract.
    let cur = unsafe { shared_l2(gib).0[idx] };
    if cur & D_VALID != 0 {
        if cur & D_TABLE == 0 {
            // A block already maps the whole region; a page-level request in
            // it would have to split the block (break-before-make, plan §3.2).
            return Err(MapError::Conflict);
        }
        let pa = cur & OA_MASK;
        let n = SHARED_L3_USED.load(Ordering::Relaxed);
        // SAFETY: the table this descriptor names is one of ours.
        let pool = unsafe { &mut *&raw mut SHARED_L3 };
        return pool[..n]
            .iter_mut()
            .find(|t| table_pa(t) == pa)
            .ok_or(MapError::Conflict);
    }
    let n = SHARED_L3_USED.load(Ordering::Relaxed);
    if n >= SHARED_L3_COUNT {
        return Err(MapError::NoTable);
    }
    SHARED_L3_USED.store(n + 1, Ordering::Relaxed);
    // SAFETY: caller's lock contract; `n` was free.
    let t = unsafe { &mut (*&raw mut SHARED_L3)[n] };
    t.0 = [0; 512];
    // SAFETY: caller contract; GiB-0 index 0 is excluded because `region` ≥ 2 MiB.
    unsafe { set_l2(gib, idx, table_pa(t) | D_VALID | D_TABLE) };
    Ok(t)
}

/// Enter `[va, va + len)` into the **shared** user address space at
/// VA = PA, with `flags`. Whole, aligned 2-MiB regions become blocks when
/// `allow_blocks` is set; the rest is 4-KiB pages. Entries that already exist
/// with the same attributes are kept; a different existing mapping is a
/// [`MapError::Conflict`] (stage 1 never rewrites a valid entry). Page 0 is
/// never touched.
///
/// Visible to every process at once — present and future slots — because
/// every slot's root shares these tables.
///
/// # Safety
///
/// Caller holds the BDOS giant lock. `[va, va + len)` must be memory that may
/// be handed to user programs (a BDOS pool, a published kernel page, the
/// screen memory).
pub unsafe fn map_shared(va: u64, len: u64, flags: PageFlags, allow_blocks: bool) -> Result<(), MapError> {
    if !va.is_multiple_of(PAGE_SIZE) {
        return Err(MapError::UnalignedVa);
    }
    let end = va
        .checked_add(len)
        .map(|e| (e + PAGE_SIZE - 1) & !(PAGE_SIZE - 1))
        .ok_or(MapError::VaOutOfRange)?;
    if va < PAGE_SIZE || end > USER_VA_LIMIT {
        return Err(MapError::VaOutOfRange);
    }
    let page_bits = flags.descriptor_bits();
    let block_bits = flags.block_bits();
    let mut a = va;
    while a < end {
        let region = a & !(L3_SPAN - 1);
        let region_end = region + L3_SPAN;
        if allow_blocks && a == region && end >= region_end && region >= L3_SPAN {
            let gib = (region / L2_SPAN) as usize;
            let idx = ((region % L2_SPAN) / L3_SPAN) as usize;
            let want = region | block_bits;
            // SAFETY: caller's lock contract; indices in range.
            let cur = unsafe { shared_l2(gib).0[idx] };
            if cur & D_VALID == 0 {
                // SAFETY: as above; GiB-0 entry 0 excluded (region ≥ 2 MiB).
                unsafe { set_l2(gib, idx, want) };
                a = region_end;
                continue;
            }
            if cur == want {
                a = region_end;
                continue;
            }
            if cur & D_TABLE == 0 {
                return Err(MapError::Conflict); // a block with other attributes
            }
            // Already an L3 table (part of the region was mapped page by
            // page before): fill in the pages below.
        }
        let stop = end.min(region_end);
        if region == 0 {
            // `[0x1000, 2 MiB)`: the template every slot copies (page 0 is
            // excluded by the `va >= PAGE_SIZE` check above).
            while a < stop {
                let idx = (a / PAGE_SIZE) as usize;
                let want = a | page_bits;
                // SAFETY: caller's lock contract.
                let cur = unsafe { shared_l3_0().0[idx] };
                if cur & D_VALID == 0 {
                    // SAFETY: `1 <= idx < 512`.
                    unsafe { set_l3_0(idx, want) };
                } else if cur != want {
                    return Err(MapError::Conflict);
                }
                a += PAGE_SIZE;
            }
            continue;
        }
        // SAFETY: caller's lock contract; `region` aligned, ≥ 2 MiB, in range.
        let t = unsafe { shared_l3_for(region)? };
        while a < stop {
            let idx = ((a - region) / PAGE_SIZE) as usize;
            let want = a | page_bits;
            let cur = t.0[idx];
            if cur & D_VALID == 0 {
                t.0[idx] = want;
            } else if cur != want {
                return Err(MapError::Conflict);
            }
            a += PAGE_SIZE;
        }
    }
    // SAFETY: barrier-only.
    unsafe { publish_user_mappings() };
    Ok(())
}

/// Enter both BDOS pools into the shared address space at VA = PA,
/// `UserRwExec` (plan §3.2: pool entries `AP=01`, `PXN=1`, `UXN=0`), 2-MiB
/// blocks where possible. Called once at boot, right after the pools are
/// seeded (`kernel::bring_up_bdos_pools`).
///
/// # Safety
///
/// Boot CPU, before any trap; the pools are initialised.
pub unsafe fn map_pools() -> Result<(), MapError> {
    for (start, end) in [
        crate::bdos::mem::init::stram_range(),
        crate::bdos::mem::init::ttram_range(),
    ] {
        if end > start {
            // SAFETY: boot CPU; the pool is user memory by definition.
            unsafe {
                map_shared(start as u64, (end - start) as u64, PageFlags::UserRwExec, true)?;
            }
        }
    }
    Ok(())
}

/// Walk `slot`'s tables in software and return the output address and the
/// leaf descriptor that maps user VA `va`, or `None` if `va` is not mapped
/// there. What the MMU would do for the process — for the boot log, the
/// self-tests and the host tests of the shared-space invariants (plan §3.1
/// I1/I3). Reads only.
pub fn translate(slot: usize, va: u64) -> Option<(u64, u64)> {
    if slot >= MAX_PROCESSES || va >= USER_VA_LIMIT {
        return None;
    }
    // A table descriptor's output address is a physical address; the tables
    // are kernel statics reachable at `pa + VA_OFFSET`. Our table descriptors
    // carry no upper attribute bits ([63:59] = 0, plan §3.2 invariant 1), so
    // everything above the low 12 bits is the address — which also lets the
    // same arithmetic undo `va_to_pa` on the host, where a "physical" table
    // address does not fit the 48-bit field.
    let table = |desc: u64| (desc & !(PAGE_SIZE - 1)).wrapping_add(VA_OFFSET) as *const u64;
    // SAFETY: read-only walk of the static tables under the BDOS lock (the
    // caller's single-CPU context); every pointer followed is a table this
    // module wrote.
    unsafe {
        // `l0` is the first field of the `#[repr(C)]` `SlotTables`.
        let l0 = ((&raw const __USER_TABLES) as *const SlotTables).add(slot) as *const u64;
        let d0 = l0.read();
        if d0 & D_VALID == 0 {
            return None;
        }
        let d1 = table(d0).add(((va >> 30) & 0x1FF) as usize).read();
        if d1 & D_VALID == 0 {
            return None;
        }
        let d2 = table(d1).add(((va >> 21) & 0x1FF) as usize).read();
        if d2 & D_VALID == 0 {
            return None;
        }
        if d2 & D_TABLE == 0 {
            return Some(((d2 & OA_MASK & !(L3_SPAN - 1)) | (va & (L3_SPAN - 1)), d2));
        }
        let d3 = table(d2).add(((va >> 12) & 0x1FF) as usize).read();
        if d3 & D_VALID == 0 {
            return None;
        }
        Some(((d3 & OA_MASK) | (va & (PAGE_SIZE - 1)), d3))
    }
}

/// The descriptor attribute bits `flags` maps a 4-KiB page with (tests).
pub fn page_bits(flags: PageFlags) -> u64 {
    flags.descriptor_bits()
}

/// Number of [`SHARED_L3`] tables in use (boot log / tests).
pub fn shared_l3_used() -> usize {
    SHARED_L3_USED.load(Ordering::Relaxed)
}

/// Phys address of `slot`'s L0 table — the value written to `TTBR0_EL1` via
/// [`install_ttbr0`].
///
/// Returns `0` for an out-of-range slot index (defensive — callers
/// would normally check the slot before reaching here).
pub fn l0_pa_for(slot: usize) -> u64 {
    if slot >= MAX_PROCESSES {
        return 0;
    }
    // Raw address-of a `static mut`: no reference, read-only arithmetic.
    // The manual offset keeps the computation unambiguous (`&raw const
    // tables.l0` produced surprising results during Phase-06 D3 bring-up).
    let base = (&raw const __USER_TABLES) as *const SlotTables as u64;
    let offset = (slot as u64) * (core::mem::size_of::<SlotTables>() as u64);
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
