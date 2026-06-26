//! USB host stack — keyboard + mouse input (PHASE-16).
//!
//! **Layer:** a kernel driver subsystem, the same layer as `bcm2711::emmc`
//! / `pl011` — *not* the BIOS/TOS-API. It feeds the existing IKBD decoder
//! ([`crate::ikbd::feed`]) → console RX ring + VDI cursor, which the BIOS
//! keyboard (`Bconin`) and the VDI mouse consume unchanged. (USB is to the
//! keyboard what `emmc` is to the disk.)
//!
//! **Runtime:** polled from [`crate::host_env`]'s `pump_input` via
//! `usb::poll()` once the controller lands — the same hook the removed
//! `ikbd_uart` used. No IRQ for bring-up.
//!
//! **Build-up (QEMU-first):** D1 `xhci` + `pcie` (detect a device) → D2
//! `core` (enumerate) → D3 `hub` → D4/D5 `hid` (keyboard/mouse → `ikbd::feed`)
//! → D6 interactive desktop in QEMU → D7 real Pi-4 (VL805 firmware @
//! `0xFE9C0000`).
//!
//! Decision + scope: `docs/MEMORY.md` ADR-015. Spec extracted from FreeMiNT:
//! `docs/api/usb/` (this module mirrors its architecture, not its C ABI).

// Scaffold: the types + the host-controller contract are in place; the
// concrete modules (xhci, pcie, core, hub, hid) land per PHASE-16 D1..D7.
#![allow(dead_code)]

// Controller discovery is board-specific: QEMU virt = PCIe ECAM; the real
// Pi-4 (D7) = fixed MMIO + VL805 firmware.
#[cfg(feature = "board-qemu-virt")]
mod pcie;
pub mod xhci;

/// Bring up the USB host stack: find the controller (via the DTB-described
/// PCIe ECAM), start it, report its root-hub ports. QEMU-virt for D1; the
/// real Pi-4 path (VL805 firmware + `0xFE9C0000`) is PHASE-16 D7.
pub fn init(dtb_phys: usize) {
    #[cfg(feature = "board-qemu-virt")]
    {
        // SAFETY: boot CPU; `dtb_phys` is the firmware device tree.
        let base = match unsafe { pcie::find_xhci(dtb_phys) } {
            Some(b) => b,
            None => return,
        };
        let mut controller = unsafe { xhci::Xhci::open(base) };
        controller.scan_ports();
        // D2: reset → enable slot → address device on the first connected port.
        controller.enumerate_first_port();
    }
    #[cfg(not(feature = "board-qemu-virt"))]
    let _ = dtb_phys;
}

/// USB transfer / stack error. Mirrors the failure modes the FreeMiNT core
/// distinguishes (timeout, stall, NAK …) plus our controller-discovery cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbError {
    /// No host controller found (PCIe ECAM miss on QEMU / no xHCI on Pi-4).
    NoController,
    /// Transfer timed out waiting on the event ring.
    Timeout,
    /// Endpoint returned STALL (protocol error / unsupported request).
    Stall,
    /// Device not responding / disconnected mid-transfer.
    NoDevice,
    /// Device sent more data than the buffer holds (babble / overrun).
    Overrun,
    /// A descriptor was malformed or shorter than required.
    BadDescriptor,
    /// The operation is not supported (e.g. non-boot HID before the parser).
    Unsupported,
}

/// USB endpoint transfer type (USB spec ch. 9; the low 2 bits of FreeMiNT's
/// pipe encoding, `docs/api/usb/host-controller-interface.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointType {
    Control,
    Isochronous,
    Bulk,
    Interrupt,
}

/// A USB pipe: the endpoint a transfer targets. Carries the device address
/// so a transfer needs only the pipe (mirrors FreeMiNT's packed pipe).
#[derive(Debug, Clone, Copy)]
pub struct Pipe {
    /// Device address (0 during the first enumeration step, then assigned).
    pub dev_addr: u8,
    /// Endpoint number (0 = the default control endpoint).
    pub endpoint: u8,
    /// `true` = IN (device→host), `false` = OUT (host→device).
    pub dir_in: bool,
    pub ep_type: EndpointType,
    /// Endpoint max packet size (`bMaxPacketSize`).
    pub max_packet: u16,
}

/// The 8-byte USB SETUP packet (USB spec §9.3) carried by a control transfer.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SetupPacket {
    pub request_type: u8,
    pub request: u8,
    pub value: u16,
    pub index: u16,
    pub length: u16,
}

/// Root-hub operations a controller answers directly (the FreeMiNT root-hub
/// `ioctl`s). Ports are 0-based.
#[derive(Debug, Clone, Copy)]
pub enum RootHubOp {
    /// Number of downstream ports the controller exposes.
    PortCount,
    /// Read a port's status.
    PortStatus(u8),
    /// Reset a port (and enable it).
    PortReset(u8),
    /// Power a port on/off.
    PortPower(u8, bool),
}

/// Per-port status from a [`RootHubOp`].
#[derive(Debug, Clone, Copy, Default)]
pub struct RootHubStatus {
    /// For `PortCount`: the port count. Otherwise unused.
    pub port_count: u8,
    pub connected: bool,
    pub enabled: bool,
    /// Connection-status-change bit (a device was just (un)plugged).
    pub connect_changed: bool,
    /// `0` low, `1` full, `2` high, `3` super speed.
    pub speed: u8,
}

/// A USB host controller. The Pi-4 **xHCI** driver (PHASE-16 D1) implements
/// this; the USB core / hub / HID layers consume it. Contract extracted from
/// FreeMiNT's `struct ucdif` — see
/// `docs/api/usb/host-controller-interface.md`. All transfers replace the
/// `SUBMIT_*_MSG` ioctls 1:1; `root_hub` replaces the root-hub ioctls.
pub trait HostController {
    /// Bring the controller up (reset, rings, slots, start). On real Pi-4
    /// the caller loads the VL805 firmware via the mailbox first.
    fn open(&mut self) -> Result<(), UsbError>;

    /// Shut the controller down.
    fn close(&mut self);

    /// Control transfer (enumeration). Returns the number of data bytes
    /// transferred.
    fn control(
        &mut self,
        pipe: Pipe,
        setup: &SetupPacket,
        data: &mut [u8],
    ) -> Result<usize, UsbError>;

    /// Interrupt transfer (HID keyboard/mouse reports).
    fn interrupt(&mut self, pipe: Pipe, data: &mut [u8]) -> Result<usize, UsbError>;

    /// Bulk transfer (mass storage later; unused by HID).
    fn bulk(&mut self, pipe: Pipe, data: &mut [u8]) -> Result<usize, UsbError>;

    /// Root-hub port management.
    fn root_hub(&mut self, op: RootHubOp) -> Result<RootHubStatus, UsbError>;
}
