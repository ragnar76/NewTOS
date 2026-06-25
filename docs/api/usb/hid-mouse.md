# HID mouse → IKBD relative-mouse packet

> Extracted from FreeMiNT `sys/usb/src.km/udd/hid/mouse-boot/usb_mouse.c`.
> The mouse counterpart of [`hid-keyboard.md`](hid-keyboard.md): USB HID
> boot-mouse reports become **IKBD relative-mouse packets** fed into the same
> decoder the desktop cursor already uses. Spec group: [`_state.md`](_state.md).

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
- **Wheel + middle button.** No Atari equivalent in the relative packet; drop
  for now (the desktop uses left/right only).
