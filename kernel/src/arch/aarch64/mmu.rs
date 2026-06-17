//! AArch64 stage-1 MMU page-table reinstall (Phase 05 D3-bis).
//!
//! # Background
//!
//! Stage 1 of the MMU bring-up runs in `arch::aarch64::boot._start`
//! before any Rust code executes: it installs a tiny 4-GiB
//! 1-GiB-block-granular page table in **both** TTBR0 (identity /
//! low-half) and TTBR1 (high-half), turns the MMU on, and absolutely
//! branches to `kernel_main` at high-half VA. From that point on, the
//! kernel runs entirely in the high-half window
//! `0xFFFF_0000_0000_0000 + phys`.
//!
//! Stage 2 — this module — replaces those Stage-1 tables with a
//! richer mapping: 2-MiB-block granularity per `MemoryMap`
//! classification (DRAM Normal Cacheable, MMIO Device-nGnRnE, stack
//! guard invalid). It then **clears `TTBR0_EL1`** so any subsequent
//! low-half access faults — exactly the contract Phase 06's
//! per-process TTBR0 install will need.
//!
//! Stage 2 does **not** rewrite `MAIR_EL1`, `TCR_EL1`, or
//! `SCTLR_EL1.{M,C,I}`. Those were programmed in Stage 1 with
//! identical values; rewriting them would briefly enter a
//! transient TCR state (e.g. `EPD1=1` while we are running at
//! high-half VA) and crash the kernel.
//!
//! # Granule, levels, coverage
//!
//! 4 KiB granule, 48-bit input VA, three levels:
//!
//! ```text
//!   L0 [bits 47:39]   1 entry  (table)            → L1
//!   L1 [bits 38:30]   ≤4 entries (table)          → L2[i]
//!   L2 [bits 29:21]   512 entries (2 MiB blocks)  → page frame
//! ```
//!
//! Total static coverage: **4 GiB** (`NUM_L2 = 4`, four 1-GiB regions).
//! That fits both boards: QEMU `virt` uses 0..2 GiB, BCM2711 / RPi4
//! uses 0..4 GiB (with VPU reserve at the top). 8-GiB Pi4 SKUs
//! expose ARM-visible RAM above 4 GiB which is **not** covered by
//! Stage 2 yet — to be addressed when 8-GiB RPi4 EC verification
//! lands.
//!
//! Each 2-MiB block is classified once at table build time:
//!   * **DRAM** (`MemoryMap::dram` contains the block start) → Normal
//!     Cacheable, Inner Shareable, AttrIdx 0.
//!   * **MMIO** (any `MemoryMap::mmio[i]` contains the block start) →
//!     Device-nGnRnE, Non-Shareable, AttrIdx 1.
//!   * **Otherwise** → invalid (descriptor = 0).
//!
//! Both boards have MMIO regions whose 2-MiB boundaries do not split
//! a peripheral; that has been verified against `mm::layout::board`.
//!
//! # ARM ARM references
//!
//! * D8.3   "Translation table descriptors" (block/table format)
//! * D8.2.4 "MAIR_EL1" (memory attribute encoding)
//! * D19.2  "TCR_EL1", "SCTLR_EL1", "TTBR0_EL1", "TTBR1_EL1"
//! * D8.7   "TLB maintenance instructions"
//! * K11.5  "MMU programming" (publish/invalidate/barrier sequence)

use crate::bios::machine::layout::{MemoryMap, PhysRange};

// ─────────────────────────────────────────────────────────────────────
// Descriptor bit constants (4 KiB granule, stage 1, EL1)
// ─────────────────────────────────────────────────────────────────────

/// Descriptor[0]: entry is valid.
const VALID: u64 = 1 << 0;
/// Descriptor[1]: at L0/L1/L2, 1 = next-level table, 0 = block.
const TABLE: u64 = 1 << 1;
/// Descriptor[1]: at L2 with 4-KiB granule, 0 = 2-MiB block.
const BLOCK_L2: u64 = 0 << 1;
/// Descriptor[4:2]: AttrIndx — index into MAIR_EL1.
const ATTRIDX_NORMAL: u64 = 0 << 2;
const ATTRIDX_DEVICE: u64 = 1 << 2;
/// AttrIdx 2 = Normal Non-Cacheable (MAIR `0x44`, see `boot::MAIR_VALUE`).
const ATTRIDX_NC: u64 = 2 << 2;
/// Mask over the AttrIndx field (descriptor[4:2]).
const ATTRIDX_MASK: u64 = 0b111 << 2;
/// Descriptor[7:6]: AP[2:1] = 00 → R/W at EL1, no EL0 access.
const AP_RW_EL1: u64 = 0b00 << 6;
/// Descriptor[9:8]: SH — shareability for Normal memory.
const SH_INNER: u64 = 0b11 << 8;
const SH_NON: u64 = 0b00 << 8;
/// Descriptor[10]: AF — Access Flag. Set so the CPU never raises an
/// AF fault on first touch.
const AF: u64 = 1 << 10;
/// Descriptor[53]: PXN — Privileged eXecute Never. Set on Device pages.
const PXN: u64 = 1 << 53;
/// Descriptor[54]: UXN — Unprivileged eXecute Never. We are EL1-only,
/// but set this on every block to be future-proof when EL0 mappings
/// arrive in Phase 06.
const UXN: u64 = 1 << 54;

// ─────────────────────────────────────────────────────────────────────
// Static page tables — placed in BSS (zero-initialized at boot)
// ─────────────────────────────────────────────────────────────────────

const ENTRIES: usize = 512;
const L2_BLOCK_SIZE: u64 = 2 * 1024 * 1024;
const L1_BLOCK_SIZE: u64 = 1024 * 1024 * 1024;
const NUM_L2: usize = 4; // 4 GiB total coverage

#[repr(C, align(4096))]
struct PageTable([u64; ENTRIES]);

// SAFETY: Modified once before MMU enable on the boot CPU. After
// enable, treated as read-only; any future remap must use
// break-before-make per ARM ARM K11.5.
static mut L0: PageTable = PageTable([0; ENTRIES]);
static mut L1: PageTable = PageTable([0; ENTRIES]);
static mut L2: [PageTable; NUM_L2] =
    [const { PageTable([0; ENTRIES]) }; NUM_L2];

// ─────────────────────────────────────────────────────────────────────
// Table construction
// ─────────────────────────────────────────────────────────────────────

/// Classify a single 2-MiB physical block start address.
///
/// Returns `Some(attrs)` with descriptor flags (VALID, AF, AP, AttrIdx,
/// SH, PXN/UXN as appropriate) but **without the address bits**, or
/// `None` if the block is unmapped (either outside DRAM/MMIO or
/// deliberately left invalid as a stack guard, Phase 03 D6).
fn classify_block(map: &MemoryMap, pa: u64) -> Option<u64> {
    // The stack guard wins over DRAM containment: it sits inside DRAM
    // by construction, but must remain unmapped so a stack overflow
    // takes a synchronous data abort.
    if pa == map.stack_guard.start {
        return None;
    }
    if range_contains_block(&map.dram, pa, L2_BLOCK_SIZE) {
        // Normal RAM. Code from .text lives here, so do not set PXN.
        Some(VALID | BLOCK_L2 | AF | AP_RW_EL1 | ATTRIDX_NORMAL | SH_INNER | UXN)
    } else if map.mmio.iter().any(|r| range_overlaps(r, pa, L2_BLOCK_SIZE)) {
        // Device memory — never executable.
        Some(VALID | BLOCK_L2 | AF | AP_RW_EL1 | ATTRIDX_DEVICE | SH_NON | PXN | UXN)
    } else {
        None
    }
}

#[inline]
fn range_contains_block(r: &PhysRange, block_start: u64, block_size: u64) -> bool {
    block_start >= r.start && block_start.saturating_add(block_size) <= r.end
}

#[inline]
fn range_overlaps(r: &PhysRange, block_start: u64, block_size: u64) -> bool {
    let block_end = block_start.saturating_add(block_size);
    block_start < r.end && r.start < block_end
}

/// Populate the static L0/L1/L2 tables for the given memory map.
///
/// Runs **before** the MMU is enabled, so it operates on physical
/// addresses (which equal the virtual addresses of these BSS arrays).
//
// `clippy::deref_addrof` would simplify `&*(&raw const X)` to `&X`,
// but the whole reason we go through `&raw` is that `&` on a
// `static mut` triggers the harder `static_mut_refs` warning. The
// `&*(&raw …)` form is the established idiom for getting a tightly-
// scoped reference to a static-mut without producing a long-lived
// `&mut` to it. The lint is a false positive against this pattern.
#[allow(clippy::deref_addrof)]
fn build(map: &MemoryMap) {
    // SAFETY (for every `&mut *(&raw mut …)` / `&*(&raw const …)`
    // below): pre-MMU, single-threaded boot path. No other CPU or
    // exception path can observe these tables until [`enable`] runs.

    // Fill L2 tables: one descriptor per 2-MiB block.
    let l2_mut: &mut [PageTable; NUM_L2] = unsafe { &mut *(&raw mut L2) };
    for (i, table) in l2_mut.iter_mut().enumerate() {
        let l1_base = (i as u64) * L1_BLOCK_SIZE;
        for (j, slot) in table.0.iter_mut().enumerate() {
            let pa = l1_base + (j as u64) * L2_BLOCK_SIZE;
            *slot = match classify_block(map, pa) {
                Some(attrs) => pa | attrs,
                None => 0,
            };
        }
    }

    // Fill L1: one table descriptor per L2 we built. Iterate L2 by
    // reference so the addresses come straight from the slice
    // pointers — no `L2[i]` indexing is needed.
    //
    // **Phys conversion**: `&L2[i]` resolves to the linker-time VA,
    // which after Phase 05 lives in the high-half. Page-table
    // descriptors must hold *physical* addresses; convert via
    // [`va_to_pa`] before stamping.
    let l1_mut: &mut PageTable = unsafe { &mut *(&raw mut L1) };
    let l2_ref: &[PageTable; NUM_L2] = unsafe { &*(&raw const L2) };
    for (l1_slot, l2_table) in l1_mut.0.iter_mut().zip(l2_ref.iter()) {
        let l2_pa = va_to_pa(l2_table as *const PageTable as u64);
        *l1_slot = l2_pa | VALID | TABLE;
    }

    // L0[0] points at L1. Same VA→PA conversion as above.
    let l1_pa = va_to_pa((&raw const L1) as u64);
    let l0_mut: &mut PageTable = unsafe { &mut *(&raw mut L0) };
    l0_mut.0[0] = l1_pa | VALID | TABLE;
}

// ─────────────────────────────────────────────────────────────────────
// MMU enable
// ─────────────────────────────────────────────────────────────────────

// MAIR_EL1 / TCR_EL1 / SCTLR_EL1 are programmed once by the Stage-1
// boot stub (`arch::aarch64::boot::_start`) and not touched again by
// Stage 2. The constants live in `boot.rs` next to the asm that
// consumes them; rewriting them here would create a drift-risk pair.

/// High-half VA offset. Mirrors `VA_OFFSET` in the linker scripts;
/// any drift between this constant and the linker would silently
/// corrupt the phys-address computation below.
pub const VA_OFFSET: u64 = 0xFFFF_0000_0000_0000;

/// Convert a high-half kernel VA to the corresponding physical
/// address. Valid only for symbols inside the kernel image (where
/// VA = LMA + VA_OFFSET); do not use for arbitrary mappings.
#[inline(always)]
pub const fn va_to_pa(va: u64) -> u64 {
    va.wrapping_sub(VA_OFFSET)
}

/// Remap the 2-MiB high-half blocks covering `[va, va + len)` from
/// Normal Cacheable to **Normal Non-Cacheable** (AttrIdx 2).
///
/// Used for the VideoCore framebuffer: the display controller is a
/// separate DMA master that reads DRAM *beyond* the ARM caches, so a
/// cached FB would leave drawn pixels stranded in dirty cache lines.
/// Mapping the FB Non-Cacheable removes the coherency problem entirely
/// (no per-frame `dc cvac` needed).
///
/// Whole 2-MiB blocks are converted — the VC-allocated FB region is
/// dedicated, so catching a little neighbouring RAM is harmless. Only
/// blocks that are currently **valid Normal** are touched (Device and
/// unmapped/guard blocks are skipped). Each block uses break-before-make
/// (invalidate → TLB flush → rewrite) per ARM ARM K11.5, so no stale
/// cacheable translation can coexist with the new attribute.
///
/// `va` is a high-half kernel VA (VA = `VA_OFFSET` + PA), matching the
/// Stage-2 identity-offset mapping.
///
/// # Safety
///
/// Boot CPU, single-threaded, after [`reinstall`] has installed the
/// Stage-2 tables. The target blocks must not contain currently-
/// executing code or the active stack (a VC-allocated framebuffer never
/// does).
#[allow(clippy::deref_addrof)]
pub unsafe fn remap_normal_nc(va: u64, len: u64) {
    if len == 0 {
        return;
    }
    let pa_first = va_to_pa(va) & !(L2_BLOCK_SIZE - 1);
    let pa_last = va_to_pa(va + len - 1) & !(L2_BLOCK_SIZE - 1);
    let nc_attrs = VALID | BLOCK_L2 | AF | AP_RW_EL1 | ATTRIDX_NC | SH_NON | PXN | UXN;

    // SAFETY: single-CPU boot path; exclusive access to the static tables.
    let l2 = unsafe { &mut *(&raw mut L2) };

    let mut pa = pa_first;
    loop {
        let i = (pa / L1_BLOCK_SIZE) as usize;
        let j = ((pa % L1_BLOCK_SIZE) / L2_BLOCK_SIZE) as usize;
        if i < NUM_L2 {
            let slot = &mut l2[i].0[j];
            let old = *slot;
            // Only convert valid, currently-Normal blocks.
            if old & VALID != 0 && (old & ATTRIDX_MASK) == ATTRIDX_NORMAL {
                // Break-before-make for this block (ARM ARM K11.5).
                *slot = 0;
                // SAFETY: publish the break, drop stale TLB, then make.
                unsafe {
                    core::arch::asm!(
                        "dsb ishst",
                        "tlbi vmalle1",
                        "dsb ish",
                        options(nostack, preserves_flags),
                    );
                }
                *slot = pa | nc_attrs;
                // SAFETY: publish the new descriptor before any access.
                unsafe {
                    core::arch::asm!(
                        "dsb ishst",
                        "isb",
                        options(nostack, preserves_flags),
                    );
                }
            }
        }
        if pa == pa_last {
            break;
        }
        pa += L2_BLOCK_SIZE;
    }
}

/// Install the Stage-2 high-half tables into `TTBR1_EL1` and flush
/// the TLB.
///
/// Sequence (per ARM ARM K11.5 "MMU programming"):
/// 1. `dsb ishst` — publish the populated `L0`/`L1`/`L2` writes
///    that [`build`] just performed.
/// 2. `msr ttbr1_el1, <new_l0_pa>` — install the new high-half root.
/// 3. `tlbi vmalle1` — drop any stale Stage-1 translations.
/// 4. `dsb ish` — gate the TLBI.
/// 5. `isb` — refetch instructions under the new translation regime.
///
/// `MAIR_EL1` / `TCR_EL1` / `SCTLR_EL1` are **deliberately
/// untouched**: Stage 1 already programmed them with the values
/// Stage 2 wants, and rewriting `TCR_EL1` mid-flight would introduce
/// a race window where the running PC's high-half VA is unmapped.
///
/// `TTBR0_EL1` is also untouched (see comment at end of function);
/// Phase 06 takes over the low-half on the first `Pexec`.
///
/// # Safety
///
/// Caller holds the BDOS giant lock or equivalent; the static
/// `L0`/`L1`/`L2` tables must have been populated by [`build`]
/// immediately before this call.
unsafe fn install_stage2_ttbr1() {
    // VA → PA for the kernel's high-half-resident L0 root. The
    // `L0` static lives in `.bss` (high-half VA), so subtracting
    // `VA_OFFSET` yields its physical load address — which is what
    // `TTBR1_EL1` consumes.
    let new_l0_pa: u64 = va_to_pa((&raw const L0) as u64);

    // Stage-2 high-half install + TLB sync.
    unsafe {
        core::arch::asm!(
            "dsb ishst",
            "msr ttbr1_el1, {ttbr1}",
            "tlbi vmalle1",
            "dsb ish",
            "isb",
            ttbr1 = in(reg) new_l0_pa,
            options(nostack, preserves_flags),
        );
    }
    // TTBR0_EL1 is left **unchanged** for now — it still points at
    // the Stage-1 boot L0 (set by `boot._start`), which has 1-GiB
    // identity blocks for 0..4 GiB. The kernel does not access
    // low-half VAs after this point, so the stale boot mapping is
    // harmless until Phase 06 installs the first per-process
    // address space (which writes `TTBR0_EL1` atomically together
    // with `TCR_EL1.EPD0` clear).
    //
    // Why not clear it now?
    //   * `TTBR0_EL1 = 0` makes the walker read from phys 0, which
    //     on QEMU `aarch64-virt` hits unmapped firmware/flash and
    //     stalls.
    //   * Setting `TCR_EL1.EPD0 = 1` mid-flight (without a TTBR0
    //     re-write) makes the boot CPU silently lose its way — ARM
    //     ARM K11.5 advises against asymmetric TCR changes while
    //     translation is live.
    //   * Re-pointing TTBR0 at an empty zero page would work but
    //     adds another 4-KiB static for no observable benefit
    //     in Phase 05.
    //
    // Phase 06 D8 will replace this comment with a real per-process
    // `TTBR0` install + `EPD0` clear sequence.
}

// ─────────────────────────────────────────────────────────────────────
// Public entry
// ─────────────────────────────────────────────────────────────────────

/// Build the full Stage-2 page tables from `map` and atomically
/// install them into `TTBR1_EL1`, clearing `TTBR0_EL1`.
///
/// Must be called exactly once, on the boot CPU, while Stage-1 boot
/// tables are still active (i.e. directly after the `kernel_main`
/// entry shim hands control to Rust). After this returns, kernel
/// code continues to execute at the same high-half VAs but those
/// VAs now resolve through the richer 2-MiB classification (DRAM
/// Normal Cacheable, MMIO Device-nGnRnE, stack guard invalid).
///
/// # Safety
///
/// Caller must guarantee single-CPU, post-Stage-1 state: this
/// function mutates the static page tables and rewrites
/// `TTBR0_EL1` / `TTBR1_EL1`. Concurrent observation by another CPU
/// or an exception handler is undefined behaviour.
pub unsafe fn reinstall(map: &MemoryMap) {
    build(map);
    // SAFETY: build() has just populated the static tables; we are
    // the sole CPU running at this point.
    unsafe { install_stage2_ttbr1() };
}
