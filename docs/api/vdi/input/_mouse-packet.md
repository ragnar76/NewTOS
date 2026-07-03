# IKBD relative-mouse packet handler (`mouse_int`) — the VDI mouse input contract

> **Phase-00b extraction (for Phase-13 mouse bring-up).**
> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_asm.S:231` (`_mouse_int`),
> `:187-223` (`scrn_clip`), `bios/ikbd.c:101-103` (packet + button
> constants), `bios/mouse.c` (`Initmous` relative-mode setup),
> `vdi/vdi_mouse.c:565-615` (`vdimouse_init`), `:195-231` (`gloc_key`
> terminators), `:341-352` (`vq_mouse`). Button/`cur_ms_stat` bit meanings:
> `vdi/vdi_mouse.c:183-194` (comment) + `vdi_asm.S:261-300`.
>
> This documents how the 3-byte IKBD relative-mouse packet becomes the VDI
> cursor state (`GCURX`/`GCURY`/`MOUSE_BT`/`cur_ms_stat`) read by `vq_mouse`
> (124), the locator ([`_input.md`](_input.md)) and the AES event manager
> (`evnt_button`/`evnt_mouse`). It is the input source the NewTOS IKBD
> driver must feed; the keyboard decoder already receives these packets on
> UART2 and currently discards them ([[keyboard-via-atari-ikbd]]).
>
> **Contract vs. reference** — see [`../../../COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract** (must match TOS exactly): the 3-byte packet layout, the
> `0xF8` header test, the IKBD→VDI button-bit **swap**, the `MOUSE_BT` and
> `cur_ms_stat` bit meanings, the signed dx/dy accumulate + screen clamp,
> and the button→motion→cursor vector call order. **Reference** (NewTOS
> reimplements freely): that this is an m68k ISR (`_mouse_int`), the
> register usage, and the `Initmous`/`aciavecs` wiring — NewTOS decodes the
> same bytes off its own IKBD/USB-HID path.

## The packet

`SBYTE mouse_packet[3]` (`bios/ikbd.c:110`) — all three bytes are
**signed**:

| Byte | Meaning |
|-----:|---------|
| 0 | header `0xF8` + button bits |
| 1 | relative `dx` (signed, −128..127) |
| 2 | relative `dy` (signed, −128..127) |

Header validation (`vdi_asm.S:244-248`): a packet is a relative-mouse
report iff `(packet[0] & 0xF8) == 0xF8` (i.e. header `0xF8..0xFB`). Anything
else is ignored by `mouse_int`. Constants: `MOUSE_REL_POS_REPORT = 0xF8`
(`bios/ikbd.c:101`).

## Button bits — note the swap

On the **IKBD wire** (`bios/ikbd.c:102-103`) the two low header bits are:

| Bit | Wire meaning | Constant |
|----:|--------------|----------|
| 0 (0x01) | **right** button down | `RIGHT_BUTTON_DOWN` |
| 1 (0x02) | **left** button down | `LEFT_BUTTON_DOWN` |

`mouse_int` **swaps** them into the VDI/GEM convention
(`vdi_asm.S:251-258`: isolate `& 3`, `lsr #1` shifts wire-bit0 (right) into
carry and wire-bit1 (left) down to bit0, then `bset #1` re-adds the carried
right bit at bit1). Result in `MOUSE_BT`:

| Bit | `MOUSE_BT` meaning |
|----:|--------------------|
| 0 (0x01) | **left** button down |
| 1 (0x02) | **right** button down |
| 2-4 | extended buttons 3/4/5 (`CONF_WITH_EXTENDED_MOUSE`, separate status packets — out of scope for bring-up) |

On a button-state change (`vdi_asm.S:268-300`): the previous extended bits
are preserved (`& 0xFFFC`), `MOUSE_BT` is updated, the `user_but` vector is
called, then the **change deltas** are computed — `eor` old vs new, `ror #2`
to place them in bits 6/7 — and stored into `cur_ms_stat` alongside the
live state.

### `cur_ms_stat` bit layout (`vdi_mouse.c:183-194`)

`cur_ms_stat` (`UBYTE`) is the mouse-status snapshot the locator polls:

| Bit | Meaning |
|----:|---------|
| 0 (0x01) | left button state (0 = up) |
| 1 (0x02) | right button state (0 = up) |
| 2-4 | reserved |
| 5 (0x20) | **motion flag** (1 = moved since last poll) |
| 6 (0x40) | **right-button-changed** flag |
| 7 (0x80) | **left-button-changed** flag |

`gloc_key` (locator) consumes bits 6/7: a set change-bit yields terminator
`0x20` (left) / `0x21` (right), and it clears bits 6/7 with `& 0x23`
(keeping live state bits 0/1 and any reserved) after reporting
(`vdi_mouse.c:203-208`). It consumes bit 5 for motion, clearing it after
returning the point (`:221-228`).

## Position

Motion test (`vdi_asm.S:308-320`): if `dx == 0 && dy == 0`, no motion —
clear `cur_ms_stat` bit 5 and leave the coordinates unchanged. Otherwise
(`vdi_asm.S:322-365`) set bit 5 and:

```
GCURX = clip( GCURX + (WORD)(SBYTE)dx )
GCURY = clip( GCURY + (WORD)(SBYTE)dy )
```

`dx`/`dy` are **sign-extended** and **added directly** — EmuTOS does not
invert Y; the cursor moves by whatever signed delta the packet carries
(positive `dy` ⇒ `GCURY` increases ⇒ downward, given the IKBD relative mode
EmuTOS configures with `Initmous(1, {topmode 0, buttons 0, xparam 1,
yparam 1}, mouse_int)`, `vdi_mouse.c:567-572,605`).

`scrn_clip` (`vdi_asm.S:187-223`) clamps against `_DEV_TAB`:

- `GCURX` to `[0, DEV_TAB[0]]` = `[0, xres-1]`
- `GCURY` to `[0, DEV_TAB[1]]` = `[0, yres-1]`

`scrn_clip` is called **twice** — once before and once after `user_mot`, so
a motion vector that repositions the cursor is re-clamped
(`vdi_asm.S:357-362`).

## Vector call order (`vdi_asm.S:271-368`)

Per packet, in order: **`user_but`** (only on a button-state change) →
**`user_mot`** (only on motion, between the two clips) → **`user_cur`**
(only on motion, after saving `GCURX`/`GCURY`). These are the VDI `vex_*`
slots ([`_mouse.md`](_mouse.md)); default `user_cur = mov_cur` schedules the
VBL redraw.

## Initial state (`vdimouse_init`, `vdi_mouse.c:575-600`)

`loc_mode = val_mode = chc_mode = str_mode = 0` (request); `HIDE_CNT = 1`
(cursor initially hidden); `GCURX = xres/2`, `GCURY = yres/2`;
`MOUSE_BT = 0`; `cur_ms_stat = 0`; `user_but = user_mot = just_rts`,
`user_cur = mov_cur`. The default arrow form is installed via
`set_mouse_form(default_mform(), &mouse_cdb)`. Relative mode is armed with
`Initmous(1, &mouse_params, mouse_int)` (`:605`).

## What `vq_mouse` (124) returns (`vdi_mouse.c:341-352`)

Atomic read (interrupts masked): `INTOUT[0] = MOUSE_BT`,
`PTSOUT[0] = GCURX`, `PTSOUT[1] = GCURY`. Already implemented in NewTOS
(`tos-api/src/vdi/mouse.rs`) but currently reads a parked state because
nothing feeds `set_state` yet. Full `vq_mouse` contract:
[`_mouse.md`](_mouse.md#vq_mouse-124--query-position--buttons).

## Verification criteria

- Packet `{0xF9, 0, 0}` (header + wire right-button-down, no motion) →
  `MOUSE_BT` bit 1 (right) set, bit 0 clear; `GCURX`/`GCURY` unchanged;
  `cur_ms_stat` bit 5 clear, bit 6 set (right changed).
- Packet `{0xFA, 5, -3}` (wire left-button-down, motion) → `MOUSE_BT` bit 0
  (left) set; `GCURX += 5`, `GCURY += -3` then clamped; `cur_ms_stat` bit 5
  set, bit 7 set (left changed).
- Packet `{0xFF, 0, 0}` — not a relative-mouse header? `0xFF & 0xF8 = 0xF8`
  → **is** valid (both buttons down, no motion).
- Non-mouse packet (header `& 0xF8 != 0xF8`) → ignored, no state change.
- `dx` driving `GCURX` past `xres-1` → clamps to `xres-1` (no wrap).

## NewTOS implementation notes

- The IKBD driver (`kernel/src/ikbd.rs`) already receives these packets on
  UART2 (`packet_data_len(0xF8..0xFB) = 2`) and discards them. The mouse
  driver decodes header+dx+dy here instead, applies the button swap + signed
  accumulate + screen clamp above, and calls `vdi::mouse::set_state(GCURX,
  GCURY, MOUSE_BT, backend)` — the existing VDI integration point.
- Screen bounds (`xres`/`yres`) come from the active `ScreenInfo` / raster
  backend, not a hard-coded resolution.
- `user_but`/`user_mot`/`user_cur` are the VDI `vex_*` slots
  ([`_mouse.md`](_mouse.md)); the AES installs `user_but`/`user_mot` to
  drive `evnt_button`/`evnt_mouse` — that wiring lands with Phase-13 D3.
  Preserve the button→motion→cursor call order and the double `scrn_clip`.
- The `cur_ms_stat` bit-6/7 change flags are what the locator's terminator
  logic reads — reproduce them so `vrq_locator`/`vsm_locator` report the
  documented `0x20`/`0x21` terminators.
- Extended buttons 3-5 + wheel (`CONF_WITH_EXTENDED_MOUSE`) arrive as
  separate IKBD status packets, not in this 3-byte report — deferred.
