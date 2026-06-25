//! Physical memory map and ST-RAM / TT-RAM pool boundaries.
//!
//! # Background — why two pools on uniform RAM
//!
//! Atari TOS programs call `Mxalloc(size, mode)` (GEMDOS 0x44) with a
//! mode flag that distinguishes ST-RAM (mode 0) from TT-RAM (mode 1),
//! plus two preference modes (2, 3). On the original Atari TT/Falcon
//! the two pools were physically distinct: ST-RAM was the chip-shared
//! memory below 14 MiB, TT-RAM was an extension board with CPU-only
//! access.
//!
//! AArch64 / RPi4 / QEMU `virt` all have uniform DRAM. We could
//! collapse the distinction, but source-ported TOS programs would
//! break:
//!   * Programs that probe with `Mxalloc(-1, 0)` to discover available
//!     ST-RAM would see "all of RAM" and over-allocate.
//!   * Programs that pass `MX_STRAM` for hardware-shared buffers
//!     (sound, video) could end up with addresses outside the
//!     low-memory region they assume.
//!   * Programs that use `MX_TTRAM` for "give me lots, fast" would
//!     get the same pool as `MX_STRAM`, defeating the hint.
//!
//! Decision (ADR 2026-05-03): NewTOS keeps the pools logically
//! separate **and additive**: the ST-RAM slot is a fixed 16 MiB
//! region at the low end of DRAM (matching the Atari address space
//! `$000000–$FFFFFF`), and the TT-RAM slot is an *additional*
//! 256 MiB region above it. The 2 MiB at the top of the ST-RAM slot
//! (`$E00000–$FFFFFF` on Atari, the cartridge / hardware-register
//! area) are excluded from the pool so the pool size matches the
//! 14 MiB maximum a real Atari TT could expose. The total managed
//! DRAM is therefore 272 MiB, not 256 MiB. Both pools are served
//! from the same physical allocator (Phase 03 D4) but are queried
//! separately.
//!
//! # Sources
//!
//! * **QEMU `virt`**: `qemu/hw/arm/virt.c`, `static const MemMapEntry
//!   base_memmap[]`. RAM at 0x4000_0000, GIC at 0x0800_0000, PL011 at
//!   0x0900_0000.
//! * **BCM2711 / Raspberry Pi 4**: BCM2711 ARM Peripherals datasheet
//!   §1.2 "Address map", and Raspberry Pi 4 boot documentation
//!   (kernel8.img loaded at physical 0x0000_8000 in 64-bit mode).
//! * **EmuTOS `Mxalloc`**: `bdos/umem.c` (extracted in
//!   `docs/api/gemdos/memory/44-mxalloc.md`).

#![allow(dead_code)] // consumed by frame allocator and Mxalloc in D4/D6.

/// ST-RAM address-space slot size on the Atari (`$000000–$FFFFFF`,
/// 16 MiB). NewTOS reserves the same low-DRAM window so that the
/// ST-RAM pool sits at "Atari-typical" physical offsets relative to
/// `DRAM_START`.
pub const STRAM_SLOT_SIZE: u64 = 16 * 1024 * 1024;

/// Top-of-slot reserve mirroring the Atari hardware-register window
/// (`$E00000–$FFFFFF`). Excluded from the pool so the maximum pool
/// size matches the 14 MiB a real Atari TT could expose.
pub const STRAM_IO_RESERVE: u64 = 2 * 1024 * 1024;

/// Hard cap for the ST-RAM pool. Matches the Atari TT/Falcon maximum
/// (`STRAM_SLOT_SIZE - STRAM_IO_RESERVE` = 14 MiB). Programs expect
/// this upper bound from `Mxalloc(-1, MX_STRAM)`.
pub const STRAM_POOL_MAX: u64 = STRAM_SLOT_SIZE - STRAM_IO_RESERVE;

/// Inclusive-exclusive physical address range.
#[derive(Clone, Copy, Debug)]
pub struct PhysRange {
    /// Inclusive start of the range.
    pub start: u64,
    /// Exclusive end of the range.
    pub end: u64,
}

impl PhysRange {
    /// Construct a half-open `[start, end)` range.
    pub const fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }
    /// Length in bytes.
    pub const fn len(&self) -> u64 {
        self.end - self.start
    }
    /// `true` if the range covers no bytes.
    pub const fn is_empty(&self) -> bool {
        self.start >= self.end
    }
    /// Half-open containment: `start <= addr < end`.
    pub const fn contains(&self, addr: u64) -> bool {
        addr >= self.start && addr < self.end
    }
}

/// Resolved board memory map. Constructed at boot by [`discover`].
#[derive(Clone, Debug)]
pub struct MemoryMap {
    /// The kernel image (`.text`, `.rodata`, `.data`, `.bss`, plus a
    /// 4-KiB-aligned tail). Never allocatable.
    pub kernel: PhysRange,
    /// Logical ST-RAM pool. Up to `STRAM_POOL_MAX`, immediately above
    /// the kernel image. Empty if total DRAM is too small.
    pub stram: PhysRange,
    /// Logical TT-RAM pool. The rest of the usable DRAM above the
    /// ST-RAM pool, up to `dram.end`.
    pub ttram: PhysRange,
    /// The full DRAM range as reported by the board.
    pub dram: PhysRange,
    /// Stack guard page (Phase 03 D6). One 2-MiB block immediately
    /// below the kernel stack; left invalid in the page tables so a
    /// stack overflow takes a synchronous data abort instead of
    /// silently corrupting memory below the stack.
    pub stack_guard: PhysRange,
    /// Memory-mapped I/O regions. Marked Device-nGnRnE in the page
    /// tables (Phase 03 D2).
    pub mmio: &'static [PhysRange],
}

impl MemoryMap {
    /// Total bytes available for `Malloc`/`Mxalloc` across both pools.
    pub const fn total_free(&self) -> u64 {
        self.stram.len() + self.ttram.len()
    }
}

// ─────────────────────────────────────────────────────────────────────
// Board-specific physical layouts
// ─────────────────────────────────────────────────────────────────────

#[cfg(feature = "board-qemu-virt")]
mod board {
    use super::{PhysRange, STRAM_SLOT_SIZE};

    /// QEMU `virt` machine DRAM start. Source: `qemu/hw/arm/virt.c`,
    /// `base_memmap[VIRT_MEM]`.
    pub const DRAM_START: u64 = 0x4000_0000;
    /// TT-RAM slot size: 256 MiB, matching the Atari-style
    /// "TT extension board" sitting above the 16 MiB ST-RAM slot.
    pub const TTRAM_SLOT_SIZE: u64 = 256 * 1024 * 1024;
    /// Total DRAM = ST-RAM slot (16 MiB) + TT-RAM slot (256 MiB) =
    /// 272 MiB. xtask passes `-m 272M` to QEMU to match.
    pub const DRAM_SIZE: u64 = STRAM_SLOT_SIZE + TTRAM_SLOT_SIZE;
    pub const DRAM_END: u64 = DRAM_START + DRAM_SIZE;

    /// MMIO regions we touch in Phase 02. More entries (RTC, PCIe,
    /// virtio-mmio) will be added in later phases.
    pub static MMIO: &[PhysRange] = &[
        // GICv2 distributor + CPU interface. Source: `virt.c`,
        // `VIRT_GIC_DIST`/`VIRT_GIC_CPU`.
        PhysRange::new(0x0800_0000, 0x0801_0000),
        PhysRange::new(0x0801_0000, 0x0802_0000),
        // PL011 UART. Source: `virt.c`, `VIRT_UART`.
        PhysRange::new(0x0900_0000, 0x0900_1000),
        // PCIe 32-bit MMIO window (xHCI BAR lands here). Source: `virt.c`,
        // `VIRT_PCIE_MMIO` = { 0x10000000, 0x2eff0000 }. The PCIe **ECAM**
        // is *not* here — on QEMU `virt` it is a high address (read from the
        // DTB + mapped with `mmu::map_device_high`, PHASE-16 D1).
        PhysRange::new(0x1000_0000, 0x3eff_0000),
    ];
}

#[cfg(feature = "board-rpi4")]
mod board {
    use super::PhysRange;

    /// RPi4 DRAM starts at physical 0. Source: BCM2711 ARM Peripherals
    /// §1.2.
    pub const DRAM_START: u64 = 0x0000_0000;
    /// We treat 1 GiB as the lower bound for usable kernel RAM on any
    /// RPi4 SKU. Real boards have 2/4/8 GiB; the allocator (D4) will
    /// trim or extend at runtime once it queries the actual size from
    /// the firmware. For D1 we hard-cap conservatively.
    pub const DRAM_SIZE: u64 = 1024 * 1024 * 1024;
    pub const DRAM_END: u64 = DRAM_START + DRAM_SIZE;

    /// MMIO regions. Sources: BCM2711 ARM Peripherals §1.2 (low
    /// peripherals at 0xFE00_0000) and §6 (ARM-local + GIC at
    /// 0xFF80_0000).
    pub static MMIO: &[PhysRange] = &[
        // Low peripherals (GPIO, UART, AUX, etc.).
        PhysRange::new(0xFE00_0000, 0xFF00_0000),
        // ARM-local + GICv2 distributor/CPU interface.
        PhysRange::new(0xFF80_0000, 0x1_0000_0000),
    ];
}

// Compile-time guard: exactly one board feature must be active.
#[cfg(not(any(feature = "board-qemu-virt", feature = "board-rpi4")))]
compile_error!("kernel/mm/layout: enable exactly one board feature");

#[cfg(all(feature = "board-qemu-virt", feature = "board-rpi4"))]
compile_error!("kernel/mm/layout: features board-qemu-virt and board-rpi4 are mutually exclusive");

// ─────────────────────────────────────────────────────────────────────
// Linker-provided kernel image symbols
// ─────────────────────────────────────────────────────────────────────

unsafe extern "C" {
    static __kernel_image_start: u8;
    static __kernel_image_end: u8;
    static __stack_guard_start: u8;
    static __stack_bottom_inclusive: u8;
    static __stack_top_exclusive: u8;
    static __emerg_stack_top_exclusive: u8;
}

/// Phase 05 high-half offset. The kernel image is linked at
/// `VA_OFFSET + phys`; addresses we hand to the page-table builder
/// and the frame allocator must be **physical**, so every linker
/// symbol pulled into a [`MemoryMap`] field gets converted via
/// [`va_to_pa`] below.
///
/// Mirrors the `VA_OFFSET` constant in
/// [`crate::arch::aarch64::mmu`] and in the linker scripts; any
/// drift would silently mis-classify blocks (kernel image landing
/// outside DRAM, stack guard outside its 2-MiB block, etc.).
const VA_OFFSET: u64 = 0xFFFF_0000_0000_0000;

#[inline(always)]
const fn va_to_pa(va: u64) -> u64 {
    va.wrapping_sub(VA_OFFSET)
}

#[inline]
fn kernel_image_start() -> u64 {
    va_to_pa((&raw const __kernel_image_start) as u64)
}
#[inline]
fn kernel_image_end() -> u64 {
    va_to_pa((&raw const __kernel_image_end) as u64)
}

/// Physical address of the lowest byte of the stack-guard region.
/// Equal to the start of the 2-MiB block left invalid in the page
/// tables. Exposed for `mmu::build` and the data-abort handler.
#[inline]
pub fn stack_guard_start() -> u64 {
    va_to_pa((&raw const __stack_guard_start) as u64)
}

/// Physical address one past the stack guard (= stack bottom).
#[inline]
pub fn stack_guard_end() -> u64 {
    va_to_pa((&raw const __stack_bottom_inclusive) as u64)
}

/// Top (exclusive) of the main kernel stack used in EL1 with SPSel=0.
#[inline]
pub fn stack_top_exclusive() -> u64 {
    (&raw const __stack_top_exclusive) as u64
}

/// Top (exclusive) of the emergency exception stack used as SP_EL1
/// during exception handling. Sized to hold one nested handler frame
/// plus generous diagnostic state.
#[inline]
pub fn emerg_stack_top_exclusive() -> u64 {
    (&raw const __emerg_stack_top_exclusive) as u64
}

// ─────────────────────────────────────────────────────────────────────
// Discovery
// ─────────────────────────────────────────────────────────────────────

/// Build the [`MemoryMap`] for the running board.
///
/// `dtb_phys` is the physical address of the device-tree blob the
/// firmware (or QEMU) loaded for us, or `None` if we are running
/// without a DTB (e.g. host-side unit tests). When a valid DTB is
/// present and contains a `/memory@…` node, its first bank overrides
/// the compile-time `board::DRAM_END` so the kernel sees the actual
/// installed RAM (Phase 03 D8). The board constants act as a fallback
/// for the no-DTB and parse-failure paths.
///
/// Pure function of board constants, linker symbols and the parsed
/// DTB. No allocation, no MMIO.
pub fn discover(dtb_phys: Option<usize>) -> MemoryMap {
    // ---- DRAM extent: prefer DTB, fall back to board defaults. ----
    let mut dram = PhysRange::new(board::DRAM_START, board::DRAM_END);
    if let Some(phys) = dtb_phys {
        // SAFETY: caller passes the firmware-supplied DTB pointer; the
        // parser bounds-checks the header before any further reads.
        if let Some(mem) = unsafe { crate::arch::aarch64::dtb::parse_memory(phys) } {
            // Only honour the DTB report if it covers the start the
            // board expects — otherwise we'd build an inconsistent map.
            if mem.start == board::DRAM_START && mem.size >= STRAM_SLOT_SIZE {
                dram = PhysRange::new(mem.start, mem.start + mem.size);
            }
        }
    }

    let img_start = kernel_image_start();
    let img_end = kernel_image_end();
    let kernel = PhysRange::new(img_start, img_end);

    // Sanity: kernel image must lie inside DRAM and be 4-KiB-aligned
    // at the upper edge (linker script guarantees this).
    debug_assert!(dram.contains(img_start));
    debug_assert!(img_end <= dram.end);
    debug_assert!(img_end.is_multiple_of(4096));

    // ST-RAM slot: fixed 16 MiB at DRAM_START. The pool is
    // [kernel_end, DRAM_START + (STRAM_SLOT_SIZE - STRAM_IO_RESERVE))
    // — i.e. the Atari `$000000–$DFFFFF` window minus the kernel
    // image. The top 2 MiB of the slot are intentionally left out
    // (Atari hardware-register reserve).
    let stram_slot_end = dram.start + STRAM_SLOT_SIZE;
    let stram_pool_end = dram.start + (STRAM_SLOT_SIZE - STRAM_IO_RESERVE);
    debug_assert!(img_end <= stram_pool_end, "kernel image overruns ST-RAM pool");
    let stram = PhysRange::new(img_end, stram_pool_end);

    // TT-RAM slot: everything from the end of the ST-RAM slot to the
    // end of DRAM. On `board-qemu-virt` this is exactly 256 MiB; on
    // `board-rpi4` it is `DRAM_SIZE - 16 MiB`.
    let ttram = PhysRange::new(stram_slot_end, dram.end);

    // Stack guard: one 2-MiB block whose start address is provided by
    // the linker. Lives inside the kernel image, so it sits in DRAM
    // and would otherwise be classified as Normal Cacheable; the MMU
    // builder consults `MemoryMap::stack_guard` and forces the block
    // to invalid.
    let guard_start = stack_guard_start();
    let guard_end = stack_guard_end();
    debug_assert!(guard_end - guard_start == 2 * 1024 * 1024);
    debug_assert!(guard_start.is_multiple_of(2 * 1024 * 1024));
    let stack_guard = PhysRange::new(guard_start, guard_end);

    MemoryMap { kernel, stram, ttram, dram, stack_guard, mmio: board::MMIO }
}

// ─────────────────────────────────────────────────────────────────────
// Tests (host-runnable: pure-logic unit tests for PhysRange).
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_contains_is_half_open() {
        let r = PhysRange::new(10, 20);
        assert!(r.contains(10));
        assert!(r.contains(19));
        assert!(!r.contains(20));
        assert!(!r.contains(9));
    }

    #[test]
    fn empty_range() {
        let r = PhysRange::new(42, 42);
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
    }
}
