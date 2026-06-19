# IKBD relative-mouse packet handler (`mouse_int`)

> **Phase-00b extraction (added for Phase-13 mouse bring-up).**
> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_asm.S:231` (`_mouse_int`),
> `bios/ikbd.c:101-103` (packet + button constants), `bios/mouse.c:40`
> (`Initmous` relative-mode setup), `vdi/vdi_mouse.c:565-615`
> (`vdimouse_init`), `vdi/vdi_mouse.c:340-349` (`vq_mouse`).
>
> This documents how the 3-byte IKBD relative-mouse packet becomes the
> VDI cursor state (`GCURX`/`GCURY`/`MOUSE_BT`) read by `vq_mouse` (124)
> and the AES event manager (`evnt_button`/`evnt_mouse`). It is the input
> source the NewTOS IKBD driver must feed; the keyboard decoder already
> receives these packets on UART2 and currently discards them
> ([[keyboard-via-atari-ikbd]]).

## The packet

`SBYTE mouse_packet[3]` (`bios/ikbd.c:110`) — all three bytes are
**signed**:

| Byte | Meaning |
|-----:|---------|
| 0 | header `0xF8` + button bits |
| 1 | relative `dx` (signed, −128..127) |
| 2 | relative `dy` (signed, −128..127) |

Header validation (`vdi_asm.S:244-248`): a packet is a relative-mouse
report iff `(packet[0] & 0xF8) == 0xF8` (i.e. `0xF8..0xFB`). Anything
else is ignored by `mouse_int`.

## Button bits — note the swap

On the **IKBD wire** (`bios/ikbd.c:102-103`), the two low header bits are:

| Bit | Wire meaning |
|----:|--------------|
| 0 (0x01) | **right** button down |
| 1 (0x02) | **left** button down |

`mouse_int` **swaps** them into the VDI/GEM convention
(`vdi_asm.S:251-258`: isolate `& 3`, `lsr #1` moves wire-bit1→bit0, then
`bset #1` re-adds wire-bit0). Result in `MOUSE_BT`:

| Bit | `MOUSE_BT` meaning |
|----:|--------------------|
| 0 (0x01) | **left** button down |
| 1 (0x02) | **right** button down |
| 2–4 | extended buttons 3/4/5 (`CONF_WITH_EXTENDED_MOUSE`, separate status packets — out of scope for bring-up) |

On a button-state change, the previous extended bits are preserved
(`& 0xFFFC`), `MOUSE_BT` is updated, and the `user_but` vector is called
(`vdi_asm.S:271-300`); `cur_ms_stat` keeps change-deltas in bits 6/7 and
the live state in bits 0/1.

## Position

Motion test (`vdi_asm.S:308-320`): if `dx == 0 && dy == 0`, no motion
(clear `cur_ms_stat` bit 5) and the coordinates are left unchanged.
Otherwise (`vdi_asm.S:322-365`):

```
GCURX = clip( GCURX + (WORD)(SBYTE)dx )
GCURY = clip( GCURY + (WORD)(SBYTE)dy )
```

`dx`/`dy` are **sign-extended** and **added directly** — EmuTOS does not
invert Y; the cursor moves by whatever signed delta the packet carries
(positive `dy` ⇒ `GCURY` increases ⇒ downward, given the IKBD relative
mode EmuTOS configures with `Initmous(1, {0,0,1,1}, …)`,
`bios/mouse.c:52-56`).

`scrn_clip` (`vdi_asm.S:187-211`) clamps:

- `GCURX` to `[0, DEV_TAB[xmax]]` = `[0, xres-1]`
- `GCURY` to `[0, DEV_TAB[ymax]]` = `[0, yres-1]`

After clipping, `user_mot` then `user_cur` (the cursor-draw vector) are
called (`vdi_asm.S:357-368`).

## Initial state (`vdimouse_init`, `vdi_mouse.c:580-600`)

`GCURX = xres/2`, `GCURY = yres/2`, `MOUSE_BT = 0`, `cur_ms_stat = 0`,
cursor initially hidden (`HIDE_CNT = 1`). Relative mode is set via
`Initmous(1, {topmode 0, buttons 0, xparam 1, yparam 1}, mouse_int)`.

## What `vq_mouse` (124) returns (`vdi_mouse.c:340-349`)

Atomic read: `INTOUT[0] = MOUSE_BT`, `PTSOUT[0] = GCURX`,
`PTSOUT[1] = GCURY`. This is already implemented in NewTOS
(`tos-api/src/vdi/mouse.rs`) but currently reads a parked state because
nothing feeds `set_state` yet.

## NewTOS implementation notes

- The IKBD driver (`kernel/src/ikbd.rs`) already receives these packets
  on UART2 (`packet_data_len(0xF8..0xFB) = 2`) and discards them. The
  mouse driver decodes header+dx+dy here instead, applies the button
  swap + signed accumulate + screen clamp above, and calls
  `vdi::mouse::set_state(GCURX, GCURY, MOUSE_BT, backend)` — the existing
  VDI integration point.
- Screen bounds (`xres`/`yres`) come from the active `ScreenInfo` /
  raster backend, not a hard-coded resolution.
- `user_but`/`user_mot`/`user_cur` vectors are the VDI `vex_*` slots
  (already present, [[../_mouse.md]]); the AES installs `user_but`/
  `user_mot` to drive `evnt_button`/`evnt_mouse` — that wiring lands with
  Phase-13 D3.
- Extended buttons 3–5 + wheel (`CONF_WITH_EXTENDED_MOUSE`) arrive as
  separate IKBD status packets, not in this 3-byte report — deferred.
