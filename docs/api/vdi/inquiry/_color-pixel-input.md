# VDI colour / pixel / input-mode inquiries (Phase 00b D5)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_col.c` (`vdi_vq_color`),
> `vdi/vdi_fill.c` (`vdi_v_get_pixel`), `vdi/vdi_input.c`
> (`vdi_vqin_mode`, `vdi_vq_key_s`). Calling convention:
> [`../_dispatcher.md`](../_dispatcher.md). `vq_extnd` (102) is covered
> in [Batch 1](../workstation/102-vq_extnd.md).

| Op | Fn | Input | Output |
|----|----|-------|--------|
| 26 | `vq_color`   | `INTIN[0]`=colour idx, `INTIN[1]`=flag | requested/actual RGB |
| 105 | `v_get_pixel`| `PTSIN[0..1]`=x,y | pixel value + colour index |
| 115 | `vqin_mode`  | `INTIN[0]`=device | request/sample mode |
| 128 | `vq_key_s`   | — | keyboard shift state |

## `vq_color` (26)

```
colnum = INTIN[0]
INTOUT[1..3] = 0                       # default RGB
if colnum < 0 or colnum >= numcolors:  # out of range
    INTOUT[0] = -1; return
INTOUT[0] = colnum
if INTIN[1] == 0:   # return last *requested* value
    INTOUT[1..3] = REQ_COL[colnum]      # 0..1000 per component
else:               # return actual *current* hardware value
    hwreg = MAP_COL[colnum] & (numcolors-1)
    INTOUT[1..3] = read DAC(hwreg), scaled to VDI 0..1000
```

The flag (`INTIN[1]`) chooses *requested* (what `vs_color` was told)
vs. *actual* (what the hardware DAC currently holds — may differ after
rounding). The HW-specific scalers (`st2vdi`/`ste2vdi`/`videl2vdi`) are
Atari shifter details; NewTOS substitutes its own DAC→0..1000 scale.

## `v_get_pixel` (105)

Reads the pixel at `PTSIN[0..1]`:
- Palette screen: `INTOUT[0]` = pixel value (raw plane bits / index),
  `INTOUT[1] = REV_MAP_COL[pel]` (VDI colour index).
- Truecolor: `INTOUT[0] = 0`, `INTOUT[1]` = 16-bit RGB.

## `vqin_mode` (115)

Returns the current input mode for the device in `INTIN[0]`
(1=locator, 2=valuator, 3=choice, 4=string) into `INTOUT[0]`:
`loc_mode` / `val_mode` / `chc_mode` / `str_mode`. Mode is REQUEST (1)
or SAMPLE (2), set by `vsin_mode` ([Batch 5](../input/_input.md)).

## `vq_key_s` (128)

`INTOUT[0] = gshift_s()` — the current keyboard shift-key bitmask
(right-shift / left-shift / ctrl / alt …), the same bits BIOS
`Kbshift` reports.

## NewTOS implementation notes

- `vq_color` "actual" path reads back the per-device palette (ADR-011)
  and scales the DAC width to VDI's 0..1000; "requested" returns the
  stored `REQ_COL` verbatim. Bring-up screen is palette-based.
- `v_get_pixel` uses the framebuffer pixel-read path; for the bring-up
  8-bit screen, `INTOUT[0]` and the index coincide before `REV_MAP_COL`.
- `vq_key_s`/`vqin_mode` bridge to the input subsystem — `gshift_s`
  maps to the same shift-state source as BIOS `Kbshift`
  (keyboard driver), and the four mode globals live with the input
  state (Batch 5).
