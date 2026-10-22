# HID mouse → IKBD relative-mouse packet

> Extracted from FreeMiNT `sys/usb/src.km/udd/hid/mouse-boot/usb_mouse.c`.
> The mouse counterpart of [`hid-keyboard.md`](hid-keyboard.md): USB HID
> boot-mouse reports become **IKBD relative-mouse packets** fed into the same
> decoder the desktop cursor already uses. Spec group: [`_state.md`](_state.md).

> **Scope note (USB01, 2026-10-22):** this file covers the *boot* mouse only.
> Report-only, composite and absolute mice, the status of the 2026-09-09
> defects and hotplug are analysed in [`hid-mouse-protocols.md`](hid-mouse-protocols.md).

## Boot mouse report (USB HID boot protocol)

Boot protocol → fixed 3–4 byte report (request the boot protocol so no HID
report-descriptor parsing is needed):

```
byte 0   buttons   bit0 = left, bit1 = right, bit2 = middle
byte 1   dx        signed 8-bit relative X (+ = right)
byte 2   dy        signed 8-bit relative Y (+ = down, USB convention)
byte 3   wheel     signed 8-bit (optional; wheel-mouse reports)
```

Polled via an **interrupt-IN** transfer (FreeMiNT `mouse_int` / `mouse_poll`).
FreeMiNT builds an IKBD packet (`mouse_packet[]`) and injects it through the
IKBD vector (`mouse_int.S` `send_packet`); NewTOS does the equivalent into
`ikbd::feed` (below).

## IKBD relative-mouse packet (the target format)

NewTOS's `kernel/src/ikbd.rs` already decodes the Atari **relative-mouse
packet** (`ikbd.rs:88`): a 3-byte sequence

```
byte 0   0xF8 | buttons_wire      header; low 2 bits = buttons, IKBD WIRE order
byte 1   dx   (signed)
byte 2   dy   (signed)
```

**IKBD wire button order is `bit0 = right, bit1 = left`** — the *opposite* of
USB. `ikbd.rs` then swaps wire→VDI (bit0 = left, bit1 = right) and calls
`vdi::feed_relative(dx, dy, buttons, …)` → cursor. So the USB driver must emit
**wire order**, i.e. swap the USB button bits:

```rust
// usb_btn: bit0 left, bit1 right (USB).  wire: bit0 right, bit1 left.
let wire = ((usb_btn & 0x01) << 1) | ((usb_btn & 0x02) >> 1);
let header = 0xF8 | (wire & 0x03);
ikbd::feed(header);
ikbd::feed(dx as u8);
ikbd::feed(dy_corrected as u8);
```

The double swap is correct: USB(left=bit0) → wire(left=bit1) → `ikbd.rs`
swap → VDI(left=bit0).

## NewTOS integration

Identical strategy to the keyboard: **synthesise the exact bytes a real IKBD
relative-mouse packet contains** and push them through `ikbd::feed`. The whole
downstream — packet assembly (`ikbd.rs:88`), `vdi::feed_relative`, cursor
position accumulation + clamp — is already built and verified (the headless
synthetic-packet test that opened a directory window). No new cursor code.

## Open details (carry into D5)

- **Y direction.** USB `+dy` = down; the Atari IKBD's default relative report
  has `+dy = up` (Y-at-bottom). Likely need `dy_corrected = -dy` (or set the
  IKBD "Y at top" mode on a real IKBD — N/A here since we synthesise). Confirm
  against the cursor moving the right way; see [[keyboard-via-atari-ikbd]].
- **Clamping / acceleration.** `vdi::feed_relative` owns position + clamp; the
  driver just passes signed deltas. High-resolution USB mice may need delta
  scaling (later refinement).
- **Wheel.** RESOLVED (GUI04, 2026-09-26). The relative packet has no room for
  it, but the IKBD does: `CONF_WITH_EXTENDED_MOUSE` gives the wheel its own
  **scancodes** — `0x59` up, `0x5a` down, `0x5c` left, `0x5d` right
  (`references/emutos/bios/ikbd.c:748-760`). `hid.rs`'s `wheel()` emits
  `0x59`/`0x5a` from report byte 3, make and break, one pair per detent.
  Whether byte 3 exists at all is decided by the **transfer length**, not by
  its value: the boot protocol's own report descriptor is three bytes (USB HID
  1.11 Appendix B.2), and FreeMiNT's boot-mouse driver reads byte 3 only when
  `actlen > 3`
  (`references/freemint/sys/usb/src.km/udd/hid/mouse-boot/usb_mouse.c:239-261`).
  `Controller::poll_interrupt` therefore reports the length, derived from the
  Transfer Event's residual (xHCI 1.2 §4.11.5.1). A mouse that sends only
  three bytes produces no wheel events at all.
  A **horizontal** wheel is *not* synthesised: a fifth byte is commonly AC Pan,
  but nothing in the boot protocol says so and FreeMiNT does not read it —
  knowing would need the HID report descriptor, which the boot protocol exists
  to avoid. The IKBD side handles `0x5c`/`0x5d` already, so a source that can
  identify one travels the same road.
- **Middle button (and 4/5).** The IKBD extension carries them as scancodes
  `0x37`/`0x5e`/`0x5f`, and `vdi_mousex_handler` turns them into `MOUSE_BT`
  bits 2..4 (`references/emutos/vdi/vdi_mouse.c:529-545`); that half is
  implemented. The USB driver does **not** translate byte-0 bits 2..4 yet —
  bit 2 (middle) is in the boot protocol, bits 3/4 are not, and no ticket has
  asked for them.
