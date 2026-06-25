//! Minimal PCIe ECAM enumeration (QEMU `virt`) — find the xHCI controller,
//! assign its BAR, enable memory decode + bus mastering, and return its MMIO
//! base. Just enough for PHASE-16 D1; the real Pi-4 uses a fixed MMIO base +
//! VL805 firmware instead (D7).
//!
//! ECAM + windows from `qemu/hw/arm/virt.c` base_memmap (see
//! `docs/api/usb/xhci-bringup.md`). Config access is
//! `ECAM + (bus<<20 | dev<<15 | fn<<12) + reg`, reached high-half like all
//! MMIO. We are the bus driver: QEMU leaves BARs unassigned, so we size +
//! place BAR0 in the 32-bit MMIO window ourselves.

use crate::arch::aarch64::{dtb, mmu};
use crate::drivers::mmio_va;
use crate::kprintln;

// VIRT_PCIE_MMIO (32-bit BAR window). Below 4 GiB, so it is mapped by the
// build-time `layout::MMIO`; BARs get placed here.
const MMIO_WINDOW: u64 = 0x1000_0000;

// Config-space dword offsets (read/written 32-bit, aligned).
const CFG_VENDOR: u16 = 0x00; // device:vendor
const CFG_COMMAND: u16 = 0x04; // status:command
const CFG_CLASS: u16 = 0x08; // class-code:revision
const CFG_BAR0: u16 = 0x10;
const CFG_BAR1: u16 = 0x14;

const CLASS_XHCI: u32 = 0x0C_0330; // serial-bus / USB / xHCI (upper 24 bits)

const CMD_MEM_SPACE: u32 = 1 << 1;
const CMD_BUS_MASTER: u32 = 1 << 2;

#[inline]
fn cfg(ecam: usize, bus: u8, dev: u8, func: u8, off: u16) -> *mut u32 {
    let a = mmio_va(ecam)
        + ((bus as usize) << 20)
        + ((dev as usize) << 15)
        + ((func as usize) << 12)
        + (off as usize & !0x3);
    a as *mut u32
}

/// SAFETY: ECAM is mapped (high-half Device); single-CPU boot context.
unsafe fn r32(ecam: usize, bus: u8, dev: u8, func: u8, off: u16) -> u32 {
    unsafe { core::ptr::read_volatile(cfg(ecam, bus, dev, func, off)) }
}
unsafe fn w32(ecam: usize, bus: u8, dev: u8, func: u8, off: u16, val: u32) {
    unsafe { core::ptr::write_volatile(cfg(ecam, bus, dev, func, off), val) }
}

/// Scan bus 0 for an xHCI controller; assign + enable BAR0; return its MMIO
/// base (physical). `None` if no PCIe / no xHCI.
///
/// The ECAM base comes from the device tree (`dtb_phys`) — on QEMU `virt` it
/// is a high address (`0x40_1000_0000`) above the build-time 4-GiB MMIO map,
/// so we map it on demand with [`mmu::map_device_high`].
///
/// # Safety
///
/// Single-CPU boot context; `dtb_phys` points at the firmware DTB.
pub unsafe fn find_xhci(dtb_phys: usize) -> Option<u64> {
    let pcie = unsafe { dtb::parse_pcie_ecam(dtb_phys) }?;
    let ecam = pcie.ecam_base as usize;
    // SAFETY: a real Device region described by the DTB; mapped once.
    unsafe { mmu::map_device_high(pcie.ecam_base, pcie.ecam_size) };
    kprintln!(
        "[usb] PCIe ECAM at 0x{:x} (size 0x{:x})",
        pcie.ecam_base, pcie.ecam_size
    );

    for dev in 0u8..32 {
        let id = unsafe { r32(ecam, 0, dev, 0, CFG_VENDOR) };
        if (id & 0xFFFF) == 0xFFFF {
            continue; // no function 0 → no device
        }
        let class = unsafe { r32(ecam, 0, dev, 0, CFG_CLASS) } >> 8;
        if class != CLASS_XHCI {
            continue;
        }

        // Size BAR0 (qemu-xhci exposes a 64-bit memory BAR).
        let orig = unsafe { r32(ecam, 0, dev, 0, CFG_BAR0) };
        unsafe { w32(ecam, 0, dev, 0, CFG_BAR0, 0xFFFF_FFFF) };
        let mask = unsafe { r32(ecam, 0, dev, 0, CFG_BAR0) } & 0xFFFF_FFF0;
        let size = (!mask).wrapping_add(1);

        // Place BAR0 at the start of the 32-bit MMIO window (it is far
        // larger than the xHCI register block, so alignment is satisfied).
        let base = MMIO_WINDOW;
        unsafe {
            w32(ecam, 0, dev, 0, CFG_BAR0, (base as u32 & 0xFFFF_FFF0) | (orig & 0x0F));
            w32(ecam, 0, dev, 0, CFG_BAR1, (base >> 32) as u32);
            // Enable memory decode + bus mastering (DMA). Command is the low
            // 16 bits of the status:command dword.
            let cmd_stat = r32(ecam, 0, dev, 0, CFG_COMMAND);
            w32(
                ecam, 0, dev, 0, CFG_COMMAND,
                cmd_stat | CMD_MEM_SPACE | CMD_BUS_MASTER,
            );
        }
        kprintln!(
            "[usb] xHCI at PCI 00:{:02x}.0 → BAR0 0x{:x} (size 0x{:x})",
            dev, base, size
        );
        return Some(base);
    }
    kprintln!("[usb] no xHCI controller found on the PCIe bus");
    None
}
