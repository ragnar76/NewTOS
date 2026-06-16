//! Concrete hardware drivers, organized by board / peripheral family.

#[cfg(feature = "board-rpi4")]
pub mod bcm2711;

#[cfg(feature = "board-qemu-virt")]
pub mod qemu_virt;

pub mod gic;
