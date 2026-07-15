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

/// The boot framebuffer geometry (32-bit XRGB truecolor), the single source
/// of truth shared by **both** board framebuffer drivers — the RPi4 VideoCore
/// mailbox (`bcm2711::framebuffer`, set-physical/set-virtual tags) and the
/// QEMU `ramfb` dev vehicle (`qemu_virt::ramfb`). Keeping the geometry in one
/// place stops the two drivers drifting and is the seed the Stage-2 runtime
/// resolution switch will parameterise (ADR-017).
///
/// ADR-017 Stage 1: NewTOS boots natively at 1920×1080. This is only an
/// *implementation* value fed into the parametric mailbox / ramfb tags — the
/// RPi4 firmware already forces a 1920×1080@60 HDMI signal
/// (`boot/rpi4/config.txt`), and the whole VDI/AES/desktop stack reads the
/// live framebuffer geometry (`RasterBackend::width()/height()`), so raising
/// it here is transparent to the renderers.
pub const BOOT_WIDTH: u32 = 1920;
/// See [`BOOT_WIDTH`].
pub const BOOT_HEIGHT: u32 = 1080;

#[cfg(feature = "board-rpi4")]
pub mod bcm2711;

#[cfg(feature = "board-qemu-virt")]
pub mod qemu_virt;

pub mod gic;

/// USB host stack — keyboard + mouse input (PHASE-16). Board-independent
/// trait + types here; the concrete xHCI controller lands per D1.
pub mod usb;
