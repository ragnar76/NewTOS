# `vq_extnd` — VDI opcode 102 (extended workstation inquire)

> Phase 00b spec. Source: EmuTOS @ `1a7484ab` —
> `vdi/vdi_control.c:624` (`vdi_vq_extnd`), `:147` (`INQ_TAB_rom`).

Returns extended device information. A flag in `INTIN[0]` selects
between re-returning the standard open-workstation tables and returning
the *extended* inquiry table plus the current clipping rectangle.

## Source extraction

```c
void vdi_vq_extnd(Vwk *vwk) {
    flip_y = 1;
    if (INTIN[0] == 0) {                  /* mode 0: same as v_opnvwk */
        PTSOUT[0..11] = SIZ_TAB[0..11];   /* sizes */
        INTOUT[0..44] = DEV_TAB[0..44];   /* capabilities */
    } else {                              /* mode 1: extended */
        PTSOUT[0] = vwk->xmn_clip;        /* current clip rect */
        PTSOUT[1] = vwk->ymn_clip;
        PTSOUT[2] = vwk->xmx_clip;
        PTSOUT[3] = vwk->ymx_clip;
        PTSOUT[4..11] = 0;
        INQ_TAB[19] = vwk->clip;          /* live clip flag */
        INTOUT[0..44] = INQ_TAB[0..44];   /* extended table */
    }
}
```

## Trap interface

| | |
|--|--|
| Opcode | `contrl[0] = 102` |
| Handle | `contrl[6]` |
| `# INTIN` | 1 (`INTIN[0]` = mode: 0 = standard tables, ≠0 = extended) |
| `# INTOUT` / `# PTSOUT` | 45 / 6 |

## Outputs

### Mode 0 (`INTIN[0] == 0`)

Identical to [`v_opnvwk`](100-v_opnvwk.md): `INTOUT` = `DEV_TAB` (45),
`PTSOUT` = `SIZ_TAB` (12).

### Mode 1 (`INTIN[0] != 0`)

`PTSOUT[0..3]` = the workstation's current clipping rectangle
(`xmn,ymn,xmx,ymx`), `PTSOUT[4..11]` = 0. `INTOUT` = `INQ_TAB` (45
extended-capability values), with `INQ_TAB[19]` updated to the live
clip flag (`vwk->clip`).

### `INQ_TAB` (45 WORDs, extended capabilities)

ROM template highlights (`INQ_TAB_rom`):

| Idx | Meaning | Idx | Meaning |
|-----|---------|-----|---------|
| 0 | alpha/graphic controller type (4) | 11 | text alignment flag |
| 1 | # background colours | 14 | max vertices (`MAX_VERTICES`) |
| 2 | text styles supported (0x1F) | 15 | max `INTIN` (−1 = unbounded) |
| 3 | scale rasters (0=no) | 16 | # mouse buttons |
| 4 | # planes | 19 | **clip flag** (live: `vwk->clip`) |
| 5 | video lookup table | 28 | bezier flag |
| 6 | performance factor | 30 | raster scaling flag |
| 7 | contour-fill capability | 40–43 | unprintable borders (printers) |
| 8 | character-rotation capability | 44 | page size (printers) |
| 9 | # writing modes (4) | | |
| 10 | highest input mode (2) | | |

(Indices 20–27, 29, 31–39 are 0 in the screen template; 40–44 are
printer/plotter-only.)

## Errors

None (no VDI error return).

## NewTOS implementation notes

- `INQ_TAB` is per **physical device** (ADR-011): each screen device
  reports its own planes/colours/resolution. The screen bring-up
  profile reports an 8-bit/palette-class device; `[14]` (max vertices)
  and `[15]` (max intin) bound the array sizes our handlers accept.
- Index 19 must reflect the *queried* `Vwk`'s clip flag (not a global).
