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
/// = Device-nGnRnE (`0x00`, implicit at bits 8..15); AttrIdx 2 = Normal
/// Non-Cacheable Inner+Outer (`0x44`, bits 16..23) — used for the
/// VideoCore framebuffer so the display DMA sees writes without cache
/// cleaning (see `mmu::remap_normal_nc`). ARM ARM D19.2.124.
const MAIR_VALUE: u64 = (0x44 << 16) | 0xFF;

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

/// `SCTLR_EL1` initial value written during the EL2→EL1 drop: the
/// architecturally-required RES1 bits with MMU and caches **off**
/// (`M`/`C`/`I` clear — the MMU bring-up sets them later). This is the
/// standard ARMv8-A reset constant; without initialising SCTLR_EL1 its
/// value is UNKNOWN on entry to EL1. ARM ARM D19.2.120.
const SCTLR_EL1_INIT: u64 = 0x30D0_0800;

/// Physical address of the PL011 UART0 **data register** (`UART_DR`,
/// offset 0), used for the pre-MMU boot heartbeat. The console driver
/// reconfigures the UART later; the heartbeat relies on the device
/// being usable at reset — on QEMU `virt` the PL011 always is, and on
/// the Pi 4 `enable_uart=1` in `config.txt` leaves UART0 enabled and
/// muxed to GPIO 14/15. A `str` to a not-yet-ready UART is a harmless
/// Device write, so this never faults.
#[cfg(feature = "board-qemu-virt")]
const BOOT_UART_DR: u64 = 0x0900_0000;
/// See [`BOOT_UART_DR`] — BCM2711 PL011 UART0 base (low-peripheral mode).
#[cfg(feature = "board-rpi4")]
const BOOT_UART_DR: u64 = 0xFE20_1000;

/// Whether the boot stub writes its heartbeat straight to [`BOOT_UART_DR`]
/// (`1`) or leaves it to the console (`0`).
///
/// `board-qemu-virt` keeps the pre-MMU write: its serial output is always on
/// (`console::SERIAL_TX_DEFAULT`). On `board-rpi4` the output is off unless
/// the boot arguments say `serial=on` (SER01), and the stub runs before they
/// can be read — a direct store would put five bytes on the line the switch
/// promises to keep silent. There the stub only computes the entry-EL digit
/// and hands it to `kernel_main` in `x1`; `console::configure_serial_tx`
/// replays `RB<EL>` once the switch is known to be on.
#[cfg(feature = "board-qemu-virt")]
const BOOT_HEARTBEAT_PRE_MMU: u64 = 1;
/// See [`BOOT_HEARTBEAT_PRE_MMU`].
#[cfg(feature = "board-rpi4")]
const BOOT_HEARTBEAT_PRE_MMU: u64 = 0;

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

        // ───── pre-MMU boot heartbeat ────────────────────────────
        // Emit "RB\r\n" straight to the PL011 data register before any
        // MMU / stack / BSS work, so the very first sign of life is
        // visible on a real serial console even if a later boot stage
        // faults silently. Primary core only (we already parked the
        // secondaries above). x20/x21 are free here.
        //
        // The entry EL (1/2/3) is kept in x22 (untouched below) and passed
        // to `kernel_main` in x1. Boards whose serial output is switched
        // (`BOOT_HEARTBEAT_PRE_MMU = 0`, board-rpi4, SER01) skip the stores
        // and let the console replay the heartbeat once the switch is on.
        "mrs    x22, CurrentEL",
        "lsr    x22, x22, #2",
        "and    x22, x22, #3",
        "mov    x21, #{heartbeat_pre_mmu}",
        "cbz    x21, 5f",
        "ldr    x20, ={boot_uart_dr}",
        "mov    w21, #0x52",               // 'R'
        "str    w21, [x20]",
        "mov    w21, #0x42",               // 'B'
        "str    w21, [x20]",
        // Emit the current exception level as a digit ('1'/'2'/'3') so a
        // bare-metal boot reveals whether the firmware/QEMU handed us
        // EL1 or EL2 — the MMU bring-up below assumes EL1.
        "add    w21, w22, #0x30",          // '0' + EL number
        "str    w21, [x20]",
        "mov    w21, #0x0d",               // CR
        "str    w21, [x20]",
        "mov    w21, #0x0a",               // LF
        "str    w21, [x20]",
        "5:",

        // ───── drop to EL1 if the firmware handed us EL2 ─────────
        //
        // QEMU `raspi4b` and real RPi4 firmware enter an AArch64 kernel
        // at EL2; QEMU `virt` (no virtualization) enters at EL1. The MMU
        // bring-up below programs the EL1 system registers, so we must be
        // running at EL1. If already at EL1, skip the drop. EL3 is not
        // produced by either platform, so only EL2→EL1 is handled.
        // ARM ARM D1.11 (exception return) + the AArch64 EL2→EL1 idiom.
        "mrs    x0, CurrentEL",
        "lsr    x0, x0, #2",
        "and    x0, x0, #3",
        "cmp    x0, #1",
        "b.eq   3f",                       // already EL1 → no drop needed

        "mov    x0, #(1 << 31)",           // HCR_EL2.RW = 1 → EL1 is AArch64
        "msr    hcr_el2, x0",
        "mrs    x0, cnthctl_el2",
        "orr    x0, x0, #0b11",            // EL1PCTEN|EL1PCEN: EL1 may read
        "msr    cnthctl_el2, x0",          //   the physical counter/timer
        "msr    cntvoff_el2, xzr",         // zero virtual-counter offset
        "ldr    x0, ={sctlr_el1_init}",
        "msr    sctlr_el1, x0",            // sane EL1 SCTLR (RES1 bits; MMU off)
        "mov    x0, #0x3c5",               // SPSR_EL2: DAIF masked, mode EL1h
        "msr    spsr_el2, x0",
        "adr    x0, 3f",                   // EL1 continuation (PC-relative, LMA)
        "msr    elr_el2, x0",
        "eret",                            // → EL1, continues at label 3
        "3:",

        // ───── enable FP/SIMD access at EL0 and EL1 ──────────────
        //
        // CPACR_EL1.FPEN = 0b11: do not trap Advanced SIMD / floating-
        // point at either EL. The kernel itself is soft-float and never
        // issues FP instructions, but EL0 programs built with the C SDK
        // use the hardware-float ABI (clang vectorises loops with NEON);
        // without this their first FP/SIMD instruction takes a trap
        // (ESR EC = 0x07). Reached on both the EL2→EL1 drop and the
        // native-EL1 paths (label 3), so every entry EL is covered.
        // ARM ARM D19.2 (CPACR_EL1); ISB is the mandatory context
        // synchronisation before the enable can be relied upon.
        //
        // NOTE: FP/SIMD register state (V0..V31, FPCR/FPSR) is not yet
        // saved/restored across the Pexec/Pterm trap frame, so a parent
        // that keeps live FP values across a `Pexec` would see them
        // clobbered by the child. The current callers (soft-float kernel
        // launching one program at a time) never do; per-process FP
        // context is a later refinement when nested FP-using Pexec lands.
        "mov    x0, #(0b11 << 20)",        // FPEN = 0b11
        "msr    cpacr_el1, x0",
        "isb",

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

        // The controller DMA block is a separate NOLOAD section outside
        // `.bss` (it lives in the Atari `$E00000` window so the ST-RAM pool
        // can keep its fixed 10 MiB — see the linker scripts), so it needs
        // its own clear. Its statics are zero-initialised Rust statics; they
        // must be zero before any Rust code runs, exactly as if they were
        // still in `.bss`. On the Pi 4 these zeros are also what
        // `usb::map_dma_coherent` cleans to RAM before switching the block
        // to Device-nGnRnE.
        "ldr    x1, ={dma_phys_start}",
        "ldr    x2, ={dma_phys_end}",
        "3:",
        "cmp    x1, x2",
        "b.hs   4f",
        "stp    xzr, xzr, [x1], #16",
        "b      3b",
        "4:",

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
        "mov    x1,  x22",                 // entry EL (1..3) for kernel_main
        "ldr    x9,  ={kernel_main}",      // high-half VA
        "br     x9",

        // ───── secondary-core / fallback park ────────────────────
        "9:",
        "wfe",
        "b      9b",

        stack_end       = sym __boot_core_stack_end_exclusive,
        bss_phys_start  = sym __bss_phys_start,
        bss_phys_end    = sym __bss_phys_end_exclusive,
        dma_phys_start  = sym __dma_phys_start,
        dma_phys_end    = sym __dma_phys_end,
        boot_l0_phys    = sym __boot_l0_phys,
        boot_l1_phys    = sym __boot_l1_phys,
        mair_value      = const MAIR_VALUE,
        tcr_value       = const TCR_VALUE,
        sctlr_bits      = const SCTLR_BITS,
        sctlr_el1_init  = const SCTLR_EL1_INIT,
        boot_uart_dr    = const BOOT_UART_DR,
        heartbeat_pre_mmu = const BOOT_HEARTBEAT_PRE_MMU,
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
    static __dma_phys_start: u8;
    static __dma_phys_end: u8;
    static __boot_l0_phys: u8;
    static __boot_l1_phys: u8;
}
