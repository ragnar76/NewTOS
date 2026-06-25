# USB-HID spec extraction — state

> Spec group for **Phase 16** (USB-HID input). Unlike the other `docs/api/`
> groups this is **not a TOS API** — it is the **FreeMiNT USB stack**
> (`references/freemint/sys/usb`), a sanctioned extension source, extracted
> per `SOURCE-ANALYSIS-PROTOCOL.md`. Decision + scope: [`ADR-015`](../../MEMORY.md),
> plan: [`PHASE-16`](../../../phases/PHASE-16-usb-hid-input.md).

## Contract vs reference (per COMPATIBILITY.md)

- **Observable / preserved:** the **TOS keyboard/IKBD behaviour** the desktop
  and GEMDOS already consume — Atari scancodes, the keyboard vector, the VDI
  mouse cursor. USB HID is translated *into* this; nothing above the input
  layer changes.
- **Internal / our own:** the entire USB stack implementation. FreeMiNT uses
  C `struct`s of function pointers + loadable modules (`ucdif`/`uddif`);
  NewTOS uses **Rust traits** in one binary. We mirror the *architecture and
  the HID→scancode mapping*, not the C ABI.

## Source map (FreeMiNT `sys/usb/src.km/`)

| Area | FreeMiNT file | NewTOS spec entry | Status |
|---|---|---|---|
| Host-controller contract (`ucdif`) | `usb_api.h`, `usb.h` | [`host-controller-interface.md`](host-controller-interface.md) | ◑ drafted |
| Core + enumeration | `usb.c`, `usb.h` | `core-enumeration.md` | ☐ |
| Hub | `hub.c`, `hub.h` | `hub.md` | ☐ |
| Class-driver contract (`uddif`) | `usb_api.h`, `udd.c` | `device-driver-interface.md` | ☐ |
| HID report parser | `udd/hid/hid-common/hidparse.c` | `hid-report-parser.md` | ☐ |
| HID keyboard → scancode → IKBD | `udd/hid/keyboard/usb_keyboard.c` | `hid-keyboard.md` | ☐ |
| HID mouse → relative IKBD packet | `udd/hid/mouse/usb_mouse.c` | `hid-mouse.md` | ☐ |

Track-B (hardware, for D1): xHCI spec + `rust-osdev/xhci` + Circle —
extracted into `docs/api/usb/xhci-bringup.md` when D1 starts.
