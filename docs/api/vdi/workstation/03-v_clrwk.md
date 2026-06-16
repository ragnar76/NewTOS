# `v_clrwk` — VDI opcode 3 (clear workstation)

> Phase 00b spec. Source: EmuTOS @ `1a7484ab` —
> `vdi/vdi_control.c:599` (`vdi_v_clrwk`).

Clears the entire screen surface to the background colour.

## Source extraction

```c
void vdi_v_clrwk(Vwk *vwk) {
    size = (ULONG)v_lin_wr * V_REZ_VT;      /* bytes per line × height */
    fill = TRUECOLOR_MODE ? 0xff : 0x00;    /* 0 = colour index 0 / white-ish */
    memset(v_bas_ad, fill, size);           /* wipe the framebuffer */
}
```

A plain `memset` of the whole framebuffer (`v_bas_ad` = screen base,
`v_lin_wr` = bytes per scanline, `V_REZ_VT` = vertical resolution).
The fill byte is `0x00` for palette modes (colour index 0) and `0xff`
for the 16-bit truecolor virtual palette.

## Trap interface

| | |
|--|--|
| Opcode | `contrl[0] = 3` |
| Handle | `contrl[6]` |
| INTIN / PTSIN / INTOUT / PTSOUT | 0 / 0 / 0 / 0 |

## Outputs / Errors

None. Always succeeds.

## NewTOS implementation notes

- Clears the whole [`hal::framebuffer`] to the background colour
  (`clear(bg)`); ignores the clipping rectangle (EmuTOS clears the
  full screen, not the clip region).
- `v_updwk` (opcode 4, “update workstation”) is a no-op (`vdi_v_nop`)
  on the screen driver — the screen is immediate-mode — so it needs no
  separate spec.
