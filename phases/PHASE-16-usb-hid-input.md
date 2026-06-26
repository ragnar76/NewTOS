# Phase 16 — USB-HID input (keyboard + mouse)

> **Status:** planned. Depends on Phase 14 (desktop, the consumer) and the
> Phase-03/13 BIOS/IKBD input path. See [`ADR-015`](../docs/MEMORY.md) for the
> decision (xHCI host + FreeMiNT upper stack).

## Goal

Give NewTOS real keyboard + mouse input through a **USB host stack**, so the
desktop is interactive on real Pi4 (and in QEMU). Plug a standard USB
keyboard/mouse into the Pi-4 USB-A ports.

Per [`docs/COMPATIBILITY.md`](../docs/COMPATIBILITY.md) this is a **clean
reimplementation against an extracted reference**, not a source-port. The USB
stack is **not** a TOS API — it is a FreeMiNT extension we adopt (see
[[freemint-extension-direction]]). The user-observable surface to preserve is
the **TOS keyboard/IKBD behaviour** (scancodes, the keyboard vector, the VDI
mouse), not the USB internals.

## Reference sources

- **Upper stack — spec source (clean-room, like EmuTOS):**
  `references/freemint/sys/usb/src.km` — `usb.c`/`hub.c`/`ucd.c`/`udd.c`
  (core, hub, enumeration), `usb.h`/`usb_api.h` (the `ucdif` host-controller
  interface + `usb_control_msg`/`usb_bulk_msg`/`usb_submit_int_msg`),
  `udd/hid/` (`hidparse.c` report parser, `keyboard/usb_keyboard.c`,
  `mouse/usb_mouse.c`) and the **HID → Atari-scancode → IKBD-vector**
  injection model.
- **Pi-4 host controller — Track B (hardware):** the **xHCI spec** (Intel),
  `rsta2/circle` (working Pi-4 xHCI + VL805 firmware reference), the no_std
  **`rust-osdev/xhci`** crate (register/structure layer), and the Raspberry
  Pi firmware **VL805 mailbox** interface.

## Architecture

```
  [USB device]  USB-A port → VL805 xHCI @ 0xFE9C0000 (real) / qemu-xhci (QEMU)
      │
  D1  xHCI host-controller driver  (the `ucdif` equivalent; rust-osdev/xhci)
      │  control / interrupt transfers
  D2  USB core — enumeration (descriptors, set address/config)
  D3  hub driver  (Pi-4 ports sit behind an internal hub; QEMU has one too)
      │
  D4  HID keyboard class → HID usage → Atari scancode ─┐
  D5  HID mouse class    → HID → relative IKBD packet ─┤
      │                                                 ▼
      └──────────────► NewTOS's existing IKBD sinks: `ikbd.rs`
                       (scancode → console RX) + `vdi::feed_relative` (cursor)
```

Only **D1** is hardware-specific. D2–D5 mirror FreeMiNT's hardware-independent
core/hub/HID. D4/D5 reuse the **existing** NewTOS IKBD path unchanged — the
desktop never learns the input came from USB.

## Deliverables

- **D0** **Source-analysis of the FreeMiNT USB stack** per
  `SOURCE-ANALYSIS-PROTOCOL.md` → spec in [`docs/api/usb/`](../docs/api/usb/) :
  host-controller (`ucdif`) contract, core enumeration + `uddif` binding, hub,
  and the HID keyboard/mouse → IKBD mapping (full HID-usage↔Atari-scancode
  table + the relative-mouse packet). Integration point fixed: both HID
  drivers synthesise raw IKBD bytes into the existing `ikbd::feed`. HID
  report-descriptor parser deferred (boot protocol). Track-B xHCI capture
  happens at D1.  ✅
- **D1** **xHCI controller bring-up in QEMU** (`qemu-virt` + `-device
  qemu-xhci`): PCIe ECAM enumeration (ECAM base from the DTB —
  `dtb::parse_pcie_ecam` — + `mmu::map_device_high` for the high virt ECAM),
  BAR assign, then the xHCI init sequence (reset → max slots → DCBAA →
  command ring → event ring → run) on `rust-osdev/xhci`, then `PORTSC`
  connect status. **Verified:** "xHCI up: 16 slots, 8 root ports"; ports 5/6
  report `CCS=1` (the `usb-kbd` + `usb-mouse`). xtask runs virt with the USB
  devices.  ✅
- **D2** **USB core** — control transfers + enumeration: port reset, Enable
  Slot, Input Context build + Address Device, then GET_DESCRIPTOR(Device) over
  EP0 (Setup/Data/Status TRBs). **Verified in QEMU:** the usb-kbd is reset,
  addressed (slot 1), and returns its 18-byte device descriptor (VID=0627
  PID=0001 — the emulated keyboard). The command/event/transfer ring
  machinery is proven; SET_CONFIGURATION + a `usb_device` model + multi-device
  enumeration are the remaining core pieces (fold into D3/D4).  ◑
- **D3** **Hub driver** — walk the (root + internal) hub, enumerate ports.
- **D4** **HID keyboard** — boot protocol, interrupt-IN polling
  (`queue_interrupt`/`poll_interrupt`, driven from `host_env::pump_input` via
  `usb::poll`), HID-usage→Atari-scancode + make/break diff (`hid.rs`) →
  `ikbd::feed`. **Verified in QEMU (QMP send-key):** space/b/return produce HID
  reports that reach `keyboard_report` → `ikbd::feed` → the console RX ring.
  The whole chain (xHCI → enumeration → interrupt → translate → IKBD →
  console) works.  ✅
- **D5** **HID mouse** — boot mouse report → IKBD relative-mouse packet
  (`mouse_report` → `ikbd::feed` → `vdi::feed_relative`). Implemented;
  Y-direction + the live cursor verified at D6 (interactive desktop).  ◑
- **D6** **Interactive desktop in QEMU** — selecting, double-click, launching
  a program, EmuCON, all driven by emulated USB kbd+mouse. The capstone the
  desktop has been waiting for.
- **D7** **Real Pi-4 bring-up** — load the **VL805 firmware via the mailbox**,
  point the xHCI driver at `0xFE9C0000`, verify a real USB keyboard + mouse on
  the USB-A ports. (8-GiB-SKU / >4 GiB DMA buffers tracked with the Phase-14
  `NUM_L2` note.)

## Architecture decision

See [`ADR-015`](../docs/MEMORY.md): **xHCI** (real USB-A ports + a working
QEMU `qemu-xhci` dev loop), upper stack clean-room from FreeMiNT, HID fed into
the existing IKBD sinks. DWC2 (USB-C/OTG) was rejected — awkward as a product
and no qemu-virt dev loop.

## Exit criteria

- **EC1** QEMU: a `-device usb-kbd` types into the console/shell.  ☐
- **EC2** QEMU: a `-device usb-mouse` drives the desktop cursor; click +
  double-click select/open; a program launches from the desktop.  ☐
- **EC3** Real Pi-4: a USB keyboard + mouse on the USB-A ports drive the
  desktop (VL805 firmware loaded, xHCI at `0xFE9C0000`).  ☐
- **EC4** No regression to the headless/synthetic IKBD test path.  ☐
