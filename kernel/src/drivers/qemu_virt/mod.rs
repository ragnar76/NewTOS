//! QEMU `aarch64-virt` machine peripheral drivers.
//!
//! The virt machine emulates an ARM PL011 UART at `0x0900_0000`
//! (see QEMU source `hw/arm/virt.c`, `VIRT_UART` entry).

pub mod pl011;
