//! Concrete hardware drivers, organized by board / peripheral family.

/// High-half VA offset for MMIO. The kernel reaches every peripheral
/// register through the high-half (TTBR1) Device mapping built by
/// `arch::aarch64::mmu`, so it never depends on the low-half identity in
/// TTBR0. That frees TTBR0 to hold a per-process address space (or
/// nothing) without a stale identity block shadowing it — the fix for the
/// real-HW Instruction-Abort on the first `Pexec`. Mirrors
/// `arch::aarch64::mmu::VA_OFFSET`.
pub const MMIO_VA_OFFSET: usize = 0xFFFF_0000_0000_0000;

/// Convert a peripheral physical base to its high-half MMIO VA.
pub const fn mmio_va(pa: usize) -> usize {
    pa + MMIO_VA_OFFSET
}

#[cfg(feature = "board-rpi4")]
pub mod bcm2711;

#[cfg(feature = "board-qemu-virt")]
pub mod qemu_virt;

pub mod gic;
