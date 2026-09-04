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

/// The **fallback / default** framebuffer geometry (32-bit XRGB truecolor),
/// shared by both board framebuffer drivers. It is **not** a forced boot
/// resolution: per **ADR-017 rev 2** (config.txt-driven) the RPi4 VideoCore
/// driver (`bcm2711::framebuffer`) *auto-detects* the display geometry the
/// firmware/`config.txt` (`hdmi_group`/`hdmi_mode`) configured — mailbox
/// get-physical tag `0x00040003` — and renders natively at it; these
/// constants are used only when that query returns 0/none. On the QEMU
/// `ramfb` dev vehicle (`qemu_virt::ramfb`) there is no firmware display to
/// query — the guest defines the geometry — so ramfb always uses this
/// default (keeping the two drivers' fallback/default geometry in one place).
///
/// The whole VDI/AES/desktop stack reads the *live* framebuffer geometry
/// (`RasterBackend::width()/height()`), so whatever geometry the RPi4 driver
/// ends up allocating (detected or fallback) is transparent to the renderers.
pub const BOOT_WIDTH: u32 = 1920;
/// See [`BOOT_WIDTH`].
pub const BOOT_HEIGHT: u32 = 1080;

#[cfg(feature = "board-rpi4")]
pub mod bcm2711;

#[cfg(feature = "board-qemu-virt")]
pub mod qemu_virt;

pub mod gic;

/// ARM generic timer — the 200 Hz system tick behind `_hz_200` (`$4BA`).
/// Board-independent: the EL1 physical timer's PPI is INTID 30 on both
/// targets (see the module docs for the per-board sources).
pub mod timer;

/// USB host stack — keyboard + mouse input (PHASE-16). Board-independent
/// trait + types here; the concrete xHCI controller lands per D1.
pub mod usb;
