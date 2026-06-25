# Host-controller interface (the `ucdif` contract)

> Extracted from FreeMiNT `sys/usb/src.km/usb_api.h` + `usb.c`. This is the
> contract a host-controller driver (HCD) implements — for NewTOS, the **Pi-4
> xHCI driver** (Phase-16 D1). Spec group: [`_state.md`](_state.md).

## FreeMiNT model (the reference)

A host controller registers a `struct ucdif` (`usb_api.h:77`) with the core
via `ucd_register(ucdif, &usb_device)` (`usb_api.h:137`). The struct is
metadata + four entry points:

```c
struct ucdif {
    struct ucdif *next;
    long  api_version;
    long  class;  char *lname;  char name[UCD_NAMSIZ];  short unit;
    unsigned short flags;
    long (*open) (struct ucdif *);          /* bring the controller up   */
    long (*close)(struct ucdif *);          /* shut it down              */
    long  resrvd1;
    long (*ioctl)(struct ucdif *, short cmd, long arg);  /* everything else */
    long  resrvd2;
    long *ucd_priv;                         /* HCD private data          */
};
```

**All USB transfers go through `ioctl`.** The core's transfer helpers
dispatch to the controller (`usb.c`):

| Core call | Dispatches to | `cmd` (`usb_api.h:44-46`) |
|---|---|---|
| `usb_control_msg` (`usb.c:179`) | `ucd->ioctl(ucd, SUBMIT_CONTROL_MSG, &arg)` | `('U'<<8)\|2` |
| `usb_bulk_msg` (`usb.c:231`)    | `ucd->ioctl(ucd, SUBMIT_BULK_MSG, &arg)`    | `('U'<<8)\|3` |
| `usb_submit_int_msg` (`usb.c:135`) | `ucd->ioctl(ucd, SUBMIT_INT_MSG, &arg)`  | `('U'<<8)\|4` |

`arg` points at a request struct carrying the device, the **pipe** (endpoint
+ direction + type — see `usb.h:322` pipe encoding), the buffer, the length,
and (for control) the 8-byte setup packet. The root hub is a virtual device
the HCD answers for via `ioctl` (port status / reset / power), driven by the
hub code (`hub.c`, `usb_rh_wakeup`).

## NewTOS shape (our own — Rust traits, not function-pointer structs)

NewTOS is one `no_std` binary, so the `ucdif`/`ucd_register` module ABI
collapses into a trait the xHCI driver implements and the USB core consumes:

```rust
/// A USB host controller (Phase-16 D1: the Pi-4 xHCI driver).
pub trait HostController {
    fn open(&mut self) -> Result<(), UsbError>;
    fn close(&mut self);

    /// Control transfer (enumeration: get/set descriptor, set address/config).
    fn control(&mut self, dev: &UsbDevice, pipe: Pipe,
               setup: &SetupPacket, data: &mut [u8]) -> Result<usize, UsbError>;
    /// Bulk transfer (mass storage later; not needed for HID).
    fn bulk(&mut self, dev: &UsbDevice, pipe: Pipe,
            data: &mut [u8]) -> Result<usize, UsbError>;
    /// Interrupt transfer (HID keyboard/mouse reports).
    fn interrupt(&mut self, dev: &UsbDevice, pipe: Pipe,
                 data: &mut [u8]) -> Result<usize, UsbError>;

    /// Root-hub: port count, per-port status, reset, power (the `ioctl`
    /// root-hub commands in FreeMiNT).
    fn root_hub(&mut self, op: RootHubOp) -> Result<RootHubStatus, UsbError>;
}
```

The three transfer methods replace the `SUBMIT_*_MSG` ioctls 1:1; `root_hub`
replaces the root-hub ioctls. `Pipe` mirrors FreeMiNT's pipe encoding
(`usb.h:322`: device, endpoint, direction, max-packet, type). Everything
above this trait (enumeration, hub, HID) is HCD-agnostic.

## What D1 (xHCI) must provide

The xHCI driver implements `HostController` on top of `rust-osdev/xhci`
(register/structure layer) + the xHCI spec (Track-B), with Circle as the
behavioural reference:

- **`open`**: reset the controller; program CRCR/DCBAAP/config; allocate the
  command ring, event ring (+ ERST), and the Device Context Base Address
  Array; enable slots; start (`USBCMD.RS`); on real Pi-4, **load VL805
  firmware via the mailbox first** and use MMIO `0xFE9C0000` (D7). In QEMU,
  the `qemu-xhci` device needs no firmware step.
- **`control`/`interrupt`**: per device-slot endpoint transfer rings; enqueue
  TRBs, ring the doorbell, wait on the event ring for completion.
- **`root_hub`**: PORTSC-based port status / reset / power; feed connect /
  disconnect to the hub layer.

> Open for the D1 source pass: the exact pipe-encoding bit layout
> (`usb.h:322`), the `SetupPacket`/`Pipe` field widths, and the xHCI TRB /
> context structures — captured into `xhci-bringup.md` when D1 begins.
