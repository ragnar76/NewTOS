# HID keyboard → Atari scancode → IKBD path

> Extracted from FreeMiNT `sys/usb/src.km/udd/hid/keyboard/usb_keyboard.c`.
> This is the **heart of the TOS integration**: USB HID boot-keyboard reports
> become Atari scancodes that flow into the **same input path a real IKBD
> feeds**. Spec group: [`_state.md`](_state.md).

## Boot keyboard report (USB HID boot protocol)

The driver requests the **boot protocol** (8-byte report, fixed layout) so no
full HID-report parsing is needed for a keyboard:

```
byte 0   modifiers (bitmask)   bit0 LCtrl bit1 LShift bit2 LAlt bit3 LGui
                               bit4 RCtrl bit5 RShift bit6 RAlt bit7 RGui
byte 1   reserved
byte 2..7  up to 6 simultaneously-pressed HID usage codes (0 = empty)
```

Polled via an **interrupt-IN** transfer (`usb_keyboard.c:357` uses
`usb_bulk_msg` on the IRQ pipe, ≤8 bytes; `kbd_int` runs ~every 40 ms).

## HID usage → Atari scancode (the table)

`translate_key()` (`usb_keyboard.c:219-322`). Atari **make** code as below;
**break** = `make | 0x80`. Complete US mapping:

```
Letters A..Z (HID 0x04..0x1d):
  A 0x1e B 0x30 C 0x2e D 0x20 E 0x12 F 0x21 G 0x22 H 0x23 I 0x17 J 0x24
  K 0x25 L 0x26 M 0x32 N 0x31 O 0x18 P 0x19 Q 0x10 R 0x13 S 0x1f T 0x14
  U 0x16 V 0x2f W 0x11 X 0x2d Y 0x15 Z 0x2c
Digits 1..0 (HID 0x1e..0x27): 1 0x02 2 0x03 3 0x04 4 0x05 5 0x06
  6 0x07 7 0x08 8 0x09 9 0x0a 0 0x0b
Return 0x1c  Esc 0x01  Backspace 0x0e  Tab 0x0f  Space 0x39
- 0x0c  = 0x0d  [ 0x1a  ] 0x1b  \ 0x2b  non-US# 0x2b  ; 0x27  ' 0x28
` 0x29  , 0x33  . 0x34  / 0x35  CapsLock 0x3a
F1..F10 (HID 0x3a..0x43): 0x3b..0x44
F11→Help 0x62  F12→Undo 0x61  ScrollLock→KP( 0x63  Pause→KP) 0x64
Insert 0x52  Home→ClrHome 0x47  Delete 0x53
Arrows: Right 0x4d Left 0x4b Down 0x50 Up 0x48
Keypad: / 0x65 * 0x66 - 0x4a + 0x4e Enter 0x72
  1 0x6d 2 0x6e 3 0x6f 4 0x6a 5 0x6b 6 0x6c 7 0x67 8 0x68 9 0x69 0 0x70 . 0x71
non-US \ 0x60
default → 0 (ignored)
```

### Modifiers

`handle_modifiers()` (`usb_keyboard.c:184`) maps modifier bits → scancodes,
with `+offset` (0x00 make, 0x80 break):

| modifier | Atari scancode |
|---|---|
| Ctrl (either) | `0x1d` |
| LShift | `0x2a` |
| RShift | `0x36` |
| Alt | `0x38` |
| CapsLock toggle | `0x3a` make+break |

## Report processing (make/break diff) — `kbd_int` (`usb_keyboard.c:324`)

Keep the previous report; each poll diff old vs new:

1. **Modifiers:** `released = (old^new) & old` → break (`|0x80`);
   `pressed = (old^new) & new` → make. Emit modifier scancodes.
2. **Keys:** every `old.keys[]` value **not** in `new.keys[]` → **break**;
   every `new.keys[]` value **not** in `old.keys[]` → **make**. Run each
   through `translate_key`, emit `scancode` (make) or `scancode|0x80` (break).
3. Save `new` as `old`.

Roll-over and the all-zero "no keys" report fall out naturally.

## NewTOS integration (our own — cleaner than FreeMiNT's vector inject)

FreeMiNT injects scancodes into the IKBD keyboard vector via
`send_data(kbd_entry, iokbd, scancode)` (`SEND_SCAN`, `usb_keyboard.c:182`,
TOS≥2 KBDVECS). NewTOS does **not** need that: `kernel/src/ikbd.rs::feed(byte)`
already consumes the **raw IKBD byte stream** — key scancodes with bit 7 =
break (`ikbd.rs:103`) — and drives scancode→ASCII→console RX / AES
`evnt_keybd`. So the USB-HID keyboard driver simply:

```rust
// for each make/break event from the report diff:
ikbd::feed(translate_key(usage));            // make
ikbd::feed(translate_key(usage) | 0x80);     // break
ikbd::feed(0x1d); ikbd::feed(0x1d | 0x80);   // modifiers, same table
```

i.e. it **synthesises exactly the bytes a real IKBD would send**, into the
existing, already-verified decoder. No new downstream code. The mouse driver
([`hid-mouse.md`](hid-mouse.md)) does the same with relative-mouse packets.

> Caps-Lock LED + state sync (`set_led`, `get_capslock_state`) and the US-only
> table are EmuTOS-parity details; non-US layouts are a later refinement (the
> scancode set is layout-independent — translation to ASCII happens in
> `ikbd.rs`, already layout-aware for US).
