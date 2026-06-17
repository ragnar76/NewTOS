# VDI mouse cursor + exchange-vector functions (Phase 00b D6)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_mouse.c`, `vdi/vdi_misc.c`
> (`vdi_vex_timv`). Calling convention:
> [`../_dispatcher.md`](../_dispatcher.md). The cursor calls delegate to
> the Line-A mouse machinery; the `vex_*` calls swap an interrupt-vector
> pointer. No error return.

| Op | Fn | Input | Effect |
|----|----|-------|--------|
| 122 | `v_show_c`  | `INTIN[0]` reset flag | show mouse cursor (`linea_show_mouse`) |
| 123 | `v_hide_c`  | — | hide mouse cursor (`linea_hide_mouse`) |
| 124 | `vq_mouse`  | — | `INTOUT[0]`=buttons, `PTSOUT[0..1]`=x,y |
| 111 | `vsc_form`  | `INTIN[]`=cursor form (37 WORDs) | set cursor shape (`linea_transform_mouse`) |
| 125 | `vex_butv`  | `CONTRL[7..8]`=routine | install button-change vector |
| 126 | `vex_motv`  | `CONTRL[7..8]`=routine | install mouse-move vector |
| 127 | `vex_curv`  | `CONTRL[7..8]`=routine | install cursor-draw vector |
| 134 | `vex_wheelv`| `CONTRL[7..8]`=routine | install wheel vector (`CONF_WITH_EXTENDED_MOUSE`) |
| 118 | `vex_timv`  | `CONTRL[7..8]`=routine | install timer-tick vector |

## Cursor visibility (`v_show_c` 122 / `v_hide_c` 123)

Maintain a hide-count: `v_hide_c` hides and increments; `v_show_c`
decrements and shows when it reaches zero. `v_show_c`'s `INTIN[0]`
reset flag forces the count to "shown" regardless. (EmuTOS routes both
through the Line-A `linea_show_mouse`/`linea_hide_mouse`.)

## `vq_mouse` (124)

With interrupts disabled (atomic read): `INTOUT[0] = MOUSE_BT` (button
bitmask), `PTSOUT[0]=GCURX`, `PTSOUT[1]=GCURY` (current cursor pos).

## `vsc_form` (111)

Reads a 37-WORD cursor definition from `INTIN[]` (hotspot x/y, mask
colour, fg colour, then 16 background-mask + 16 foreground rows) and
installs it as the current cursor sprite, constraining the hotspot and
mapping the colours.

## Exchange vectors (`vex_*`, 125–127, 134, 118)

All share one shape: the new routine pointer is in `CONTRL[7..8]`, the
**previous** pointer is returned in `CONTRL[9..10]`, and the global is
swapped:

| Fn | Global |
|----|--------|
| `vex_butv` | `user_but` — called on button-state change |
| `vex_motv` | `user_mot` — called on cursor-coordinate change |
| `vex_curv` | `user_cur` — called to draw the cursor |
| `vex_wheelv` | `user_wheel` — called on wheel movement |
| `vex_timv` | `tim_addr` — timer-C tick handler; also returns `INTOUT[0]` = ms between ticks (`Tickcal()`) |

## NewTOS implementation notes

- Cursor + button + position come from the **input subsystem** (mouse
  driver), not the framebuffer. For QEMU bring-up the mouse source is
  the virtio/USB-HID or PS/2 input device; until that exists, these
  return a parked cursor and zero buttons (documented limitation — NOT
  a stub returning fake motion).
- `vex_*` vectors are real function-pointer slots the IRQ/timer path
  calls; install/return semantics must be exact (programs chain the old
  vector). Pointer ABI is 32-bit TOS — see the trap pointer mapping.
- `vex_timv` ties into the system tick (Phase 07 timer); `Tickcal`
  returns the tick period in ms.
