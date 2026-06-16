//! Boot stub: AArch64 entry point + Stage-1 MMU bring-up.
//!
//! ─── Phase 01 (initial responsibility) ────────────────────────────
//!
//! 1. Read `MPIDR_EL1`; let only the primary core (Aff0 == 0) proceed.
//! 2. Park secondary cores in a `wfe` loop.
//! 3. Set the stack pointer to `__boot_core_stack_end_exclusive`
//!    (defined in the linker script, low-half = LMA).
//! 4. Zero the `.bss` region.
//! 5. Jump to `kernel_main` (Rust entry).
//!
//! ─── Phase 05 (high-half migration) ───────────────────────────────
//!
//! Steps 1..4 run pre-MMU at the kernel image's LMA (low-half phys).
//! Pre-MMU code reaches **PC-relative** symbols by their image-internal
//! offset (B/BL within `.text` work whether resolved at LMA or
//! high-half VA, because the offset is the same). It reaches
//! **absolute** symbols by their link-time value, which is the
//! high-half VA — so absolute references to BSS, page tables, or any
//! kernel data must be converted to phys via the `__*_phys` aliases
//! the linker script provides.
//!
//! After step 4 the boot stub builds a Stage-1 page-table tree
//! (`__boot_l0` + `__boot_l1`, 8 KiB total) covering the first 4 GiB
//! of physical memory in 1-GiB block descriptors with per-board
//! attributes (DRAM Normal Cacheable Inner Shareable, MMIO
//! Device-nGnRnE). The same root is installed in **both** TTBR0 and
//! TTBR1, so the identity (low-half) and high-half VA windows resolve
//! to the same physical pages. This satisfies two requirements:
//!
//! - The very next instruction after `SCTLR_EL1.M = 1` still runs
//!   from low-half VAs (the PC has not moved), so the identity
//!   window in TTBR0 must be valid until we explicitly switch to
//!   high-half by absolute branch.
//! - `kernel_main`'s absolute address loaded via `LDR =sym` is a
//!   high-half VA, so the TTBR1 mapping must be live before the
//!   branch.
//!
//! Once `kernel_main` (high-half) starts, Stage 2 (`mmu::reinstall`,
//! Phase 05 D3-bis) replaces the boot tables with the full mapping
//! (per-2-MiB-block classification, stack guard, MMIO segregation)
//! and clears TTBR0 so user-space TTBR0_EL1 changes in Phase 06 take
//! effect cleanly.
//!
//! ─── Risk control (Phase 05 R1) ───────────────────────────────────
//!
//! Wrong sequencing of MMU-enable / barrier / branch is "silent
//! death" territory. The body below mirrors ARM ARM K11.5 ("MMU
//! programming") line for line; every change to the order or to the
//! barrier set must be reviewed against that section.

use core::arch::naked_asm;

// ─── Stage-1 boot page tables ─────────────────────────────────────
//
// Two 4-KiB pages, naturally aligned. `__boot_l1` is initialised at
// link time with four 1-GiB block descriptors (per-board flags
// below). `__boot_l0` is zero-on-entry; the boot stub writes one
// entry at `[0]` pointing at `__boot_l1` (post-zero, pre-MMU-enable).

/// 512-entry, 4-KiB-aligned page-table page.
#[repr(C, align(4096))]
struct BootPageTable([u64; 512]);

// AArch64 page-descriptor bits (subset used here; see ARM ARM D8.3).
const VALID: u64 = 1 << 0;
const BLOCK_L1: u64 = 0 << 1; // L1 block descriptor (vs. table at this level)
// L0[0] is written in asm with the literal `0b11` (VALID|TABLE);
// the symbolic `TABLE = 1 << 1` is documented in `mmu.rs`.
const AF: u64 = 1 << 10; // Access Flag — set so the CPU never raises an AF fault
const AP_RW_EL1: u64 = 0b00 << 6; // R/W at EL1, no EL0 access
const SH_INNER: u64 = 0b11 << 8; // Inner Shareable
const ATTRIDX_NORMAL: u64 = 0 << 2; // MAIR slot 0 — Normal Cacheable WBWA
const ATTRIDX_DEVICE: u64 = 1 << 2; // MAIR slot 1 — Device-nGnRnE
const PXN: u64 = 1 << 53; // Privileged eXecute Never — set on Device blocks
const UXN: u64 = 1 << 54; // Unprivileged eXecute Never — set on every block

/// Block descriptor flag-set for Normal Cacheable DRAM (kernel image,
/// heap, stack). Code is fetched from these blocks, so PXN is **not**
/// set; UXN is on as future-proofing for Phase 06 EL0 exec policy.
const NORMAL_BLOCK: u64 =
    VALID | BLOCK_L1 | AF | AP_RW_EL1 | ATTRIDX_NORMAL | SH_INNER | UXN;

/// Block descriptor flag-set for Device-nGnRnE MMIO (PL011, GIC,
/// VC4 mailbox). Strict ordering, no speculation, no execution.
const DEVICE_BLOCK: u64 =
    VALID | BLOCK_L1 | AF | AP_RW_EL1 | ATTRIDX_DEVICE | UXN | PXN;

// Per-board L1 entries. Each entry is `phys_base | flags` for the
// 1-GiB block starting at that base. Index N covers
// `[N << 30, (N+1) << 30)`.
//
// QEMU `aarch64-virt`: PL011 + GIC at 0x09xx_xxxx (block 0); kernel
// image + DRAM at 0x4008_0000 onward (block 1). Blocks 2/3 are
// nominally unused (DRAM may extend into block 2 with `-m ≥ 2G`,
// but Stage 1 doesn't depend on that — Stage 2 rebuilds with the
// real DTB-discovered map).
//
// `clippy::identity_op` would flag `0x0000_0000 | flags` as a no-op
// `OR` with zero, but the literal `0x0000_0000` is the documentation
// of "this entry covers the 0..1-GiB block". Keep the symmetric
// `phys_base | flags` form across all four entries.
#[allow(clippy::identity_op)]
#[cfg(feature = "board-qemu-virt")]
const BOOT_L1_ENTRIES: [u64; 4] = [
    0x0000_0000 | DEVICE_BLOCK, // 0..1 GiB: MMIO (PL011, GIC, fwcfg)
    0x4000_0000 | NORMAL_BLOCK, // 1..2 GiB: kernel image + DRAM
    0x8000_0000 | NORMAL_BLOCK, // 2..3 GiB: DRAM extension
    0xC000_0000 | DEVICE_BLOCK, // 3..4 GiB: spare; Device is the safer default
];

// Raspberry Pi 4 (BCM2711, low-peripheral mode): DRAM 0..3 GiB
// covers all the ARM-visible RAM on 1/2/4-GiB SKUs and the
// "low" segment of the 8-GiB SKU. MMIO lives at 0xFE00_0000 (block
// 3). The 8-GiB SKU exposes additional ARM RAM above 4 GiB which
// Stage 1 does **not** map; Stage 2 (`mmu::reinstall`) rebuilds
// from the DTB and covers the full installed DRAM.
#[allow(clippy::identity_op)] // see comment above
#[cfg(feature = "board-rpi4")]
const BOOT_L1_ENTRIES: [u64; 4] = [
    0x0000_0000 | NORMAL_BLOCK, // 0..1 GiB: DRAM (kernel image is here)
    0x4000_0000 | NORMAL_BLOCK, // 1..2 GiB: DRAM
    0x8000_0000 | NORMAL_BLOCK, // 2..3 GiB: DRAM (up to 3 GiB ARM-visible)
    0xC000_0000 | DEVICE_BLOCK, // 3..4 GiB: BCM2711 MMIO at 0xFE00_0000
];

const fn build_boot_l1() -> BootPageTable {
    let mut t = [0u64; 512];
    let mut i = 0;
    while i < BOOT_L1_ENTRIES.len() {
        t[i] = BOOT_L1_ENTRIES[i];
        i += 1;
    }
    BootPageTable(t)
}

/// Stage-1 L1 table. Initialised at link time with four 1-GiB block
/// descriptors (per-board); the rest of the entries stay at zero
/// (= invalid). Linked into `.rodata` because the boot stub does
/// not need to mutate it. The linker script exposes its phys
/// address as `__boot_l1_phys`.
#[unsafe(no_mangle)]
#[unsafe(link_section = ".rodata.boot_pgtables")]
static __BOOT_L1: BootPageTable = build_boot_l1();

/// Stage-1 L0 table. Zero on entry (BSS). The boot stub writes one
/// entry at index 0, pointing to `__boot_l1`. Linked into `.bss`
/// (NOLOAD) so the ELF stays small. The linker script exposes its
/// phys address as `__boot_l0_phys`.
#[unsafe(no_mangle)]
#[unsafe(link_section = ".bss.boot_pgtables")]
static mut __BOOT_L0: BootPageTable = BootPageTable([0; 512]);

// ─── MMU register values ──────────────────────────────────────────
//
// Match the values in `mmu.rs` for Stage 2 — when the Rust reinstall
// runs, only TTBR1's payload changes (and TTBR0 is cleared); MAIR /
// SCTLR / TCR stay the same. Keeping the constants here in sync with
// `mmu.rs` is checked by visual review (and by the fact that any
// drift between Stage 1 and Stage 2 would manifest as a hung Phase-02
// selftest after the reinstall barrier).

/// `MAIR_EL1`. AttrIdx 0 = Normal WBWA Inner+Outer (`0xFF`); AttrIdx 1
/// = Device-nGnRnE (`0x00`, implicit at bits 8..15).
const MAIR_VALUE: u64 = 0xFF;

/// `TCR_EL1`. Both halves enabled, 48-bit VA in each (`T0SZ = T1SZ =
/// 16`), 4-KiB granule, Inner Shareable Normal cacheable for the
/// page-table walks, 40-bit IPS (1 TiB).
///
/// Note the asymmetric granule encoding: TG0 = 0b00 means 4 KiB,
/// TG1 = 0b10 means 4 KiB. ARM ARM D19.2.131.
const TCR_VALUE: u64 = 16                       // T0SZ
    | (0b01 << 8)                               // IRGN0 = WBWA
    | (0b01 << 10)                              // ORGN0 = WBWA
    | (0b11 << 12)                              // SH0   = Inner
    // TG0 = 0b00 → 4 KiB (implicit zero)
    | (16 << 16)                                // T1SZ
    // EPD1 = 0 (bit 23) → enable TTBR1 walks
    | (0b01 << 24)                              // IRGN1 = WBWA
    | (0b01 << 26)                              // ORGN1 = WBWA
    | (0b11 << 28)                              // SH1   = Inner
    | (0b10u64 << 30)                           // TG1   = 4 KiB
    | (0b010u64 << 32);                         // IPS   = 40-bit

/// `SCTLR_EL1` bits to set when enabling the MMU: `M` (MMU), `C`
/// (data cache), `I` (instruction cache).
const SCTLR_BITS: u64 = (1 << 0) | (1 << 2) | (1 << 12);

/// AArch64 boot entry. Linked into `.text.boot` so the linker script
/// places it at the start of the `.text` section, which (after the
/// VA/LMA split, Phase 05 D1) lives at LMA = `KERNEL_PHYS`.
///
/// On entry the firmware (or QEMU `-kernel`) places the physical
/// address of the device-tree blob in `x0` (Linux ARM64 boot
/// protocol). The boot stub preserves it across BSS-zero and the
/// MMU bring-up in `x19`, and forwards it as the AAPCS64 first
/// argument to `kernel_main`.
///
/// # Safety
///
/// Must only be called by the CPU reset / firmware. Never call from
/// Rust — the function never returns and assumes specific machine
/// state on entry (DTB in `x0`, EL1, MMU off, single CPU).
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        // ───── core selection (pre-MMU @ LMA) ────────────────────
        "mrs    x1, mpidr_el1",
        "and    x1, x1, #0xff",          // Aff0 (low byte) selects core
        "cbnz   x1, 9f",                  // non-zero → secondary, park

        // ───── primary-core setup (still pre-MMU) ────────────────
        "mov    x19, x0",                  // preserve DTB phys ptr

        // SP := __boot_core_stack_end_exclusive (low-half = LMA).
        // The boot stack symbol is linked at low-half by the linker
        // script (`segment_boot_core_stack`), so a plain LDR =sym
        // resolves to a directly-addressable phys address.
        "ldr    x1, ={stack_end}",
        "mov    sp, x1",

        // BSS zero. Use the *_phys aliases — `__bss_start` itself is a
        // high-half VA which doesn't resolve until the MMU is on.
        "ldr    x1, ={bss_phys_start}",
        "ldr    x2, ={bss_phys_end}",
        "1:",
        "cmp    x1, x2",
        "b.hs   2f",
        "stp    xzr, xzr, [x1], #16",
        "b      1b",
        "2:",

        // ───── Stage-1 page tables ───────────────────────────────
        //
        // L0[0] := phys(__BOOT_L1) | TABLE | VALID. L1 is already
        // populated at link time. Both TTBR0 and TTBR1 will point to
        // L0; with VA[55:48] of high-half all zero (top tag),
        // L0[0] is the relevant entry for both halves.
        "ldr    x10, ={boot_l0_phys}",
        "ldr    x11, ={boot_l1_phys}",
        "orr    x12, x11, #0b11",          // VALID | TABLE
        "str    x12, [x10]",

        // ───── MMU register configuration ────────────────────────
        "ldr    x9,  ={mair_value}",
        "msr    mair_el1, x9",

        "ldr    x9,  ={tcr_value}",
        "msr    tcr_el1, x9",

        "msr    ttbr0_el1, x10",            // identity covers low-half
        "msr    ttbr1_el1, x10",            // same root → high-half live too

        // Publish table writes; flush stale TLB; gate before SCTLR.M.
        // ARM ARM K11.5 "MMU programming" sequence.
        "dsb    ishst",
        "tlbi   vmalle1",
        "dsb    ish",
        "isb",

        // SCTLR_EL1 |= M | C | I — MMU on, both caches on.
        "mrs    x9,  sctlr_el1",
        "ldr    x10, ={sctlr_bits}",
        "orr    x9,  x9, x10",
        "msr    sctlr_el1, x9",
        "isb",

        // ───── absolute branch to high-half kernel_main ─────────
        //
        // Up to here we ran from low-half VAs (PC was at LMA pre-MMU,
        // identity-translates through TTBR0). After this branch PC
        // is at the high-half VA and translation goes through TTBR1.
        // Both maps point at the same physical RAM, so the same
        // machine code keeps executing.
        "mov    x0,  x19",                 // restore DTB ptr for kernel_main
        "ldr    x9,  ={kernel_main}",      // high-half VA
        "br     x9",

        // ───── secondary-core / fallback park ────────────────────
        "9:",
        "wfe",
        "b      9b",

        stack_end       = sym __boot_core_stack_end_exclusive,
        bss_phys_start  = sym __bss_phys_start,
        bss_phys_end    = sym __bss_phys_end_exclusive,
        boot_l0_phys    = sym __boot_l0_phys,
        boot_l1_phys    = sym __boot_l1_phys,
        mair_value      = const MAIR_VALUE,
        tcr_value       = const TCR_VALUE,
        sctlr_bits      = const SCTLR_BITS,
        kernel_main     = sym crate::kernel_main,
    )
}

// Linker-script-provided symbols. Their *addresses* are the relevant
// values; the storage itself is never read or written through these
// declarations. `__boot_core_stack_end_exclusive` is at low-half =
// LMA (boot stack lives in pre-MMU-addressable RAM); the `*_phys`
// aliases are explicitly defined as `<symbol> - VA_OFFSET` in the
// linker script so they yield phys addresses for pre-MMU access.
unsafe extern "C" {
    static __boot_core_stack_end_exclusive: u8;
    static __bss_phys_start: u8;
    static __bss_phys_end_exclusive: u8;
    static __boot_l0_phys: u8;
    static __boot_l1_phys: u8;
}
