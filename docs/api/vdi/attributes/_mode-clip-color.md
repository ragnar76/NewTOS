# VDI mode / clip / colour setters (Phase 00b D3)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_control.c` (`vswr_mode`,
> `vs_clip`), `vdi/vdi_col.c` (`vs_color`). Setter shape as in
> [`_line-marker.md`](_line-marker.md). No error return.

| Op | Fn | Input | Effect → `Vwk` / device | Output |
|----|----|-------|-------------------------|--------|
| 32 | `vswr_mode` | `INTIN[0]` mode | `MIN..MAX_WRT_MODE` else `DEF_WRT_MODE`; `wrt_mode = mode - 1`; `CONTRL[4]=1` | `INTOUT[0]` = accepted mode (1-based) |
| 129 | `vs_clip`  | `INTIN[0]` flag; `PTSIN[0..3]` rect | `clip = INTIN[0]`; see below | none |
| 14 | `vs_color`  | `INTIN[0]` colour idx; `INTIN[1..3]` RGB | see below; updates hardware palette | none |

### `vswr_mode` — writing modes (`INTIN[0]`, 1-based)

| Value | `wrt_mode` | Mode |
|-------|-----------|------|
| 1 | 0 | replace |
| 2 | 1 | transparent |
| 3 | 2 | XOR |
| 4 | 3 | reverse-transparent |

`wrt_mode` is stored 0-based (`mode-1`); the drawing primitives switch
on it. `DEF_WRT_MODE` = replace.

### `vs_clip` — clipping rectangle

`clip = INTIN[0]` (0 = off, ≠0 = on).

- **On:** read the rectangle from `PTSIN[0..3]`, normalise corners
  (`arb_corner` — sort so x1≤x2, y1≤y2), then **intersect with the
  screen**:
  `xmn = max(0, x1)`, `ymn = max(0, y1)`,
  `xmx = min(xres, x2)`, `ymx = min(yres, y2)`.
- **Off:** clip to the full device — `[0,0]` … `[xres, yres]`.

Stored in `xmn_clip`/`ymn_clip`/`xmx_clip`/`ymx_clip`; every primitive
clips against this rect.

### `vs_color` — set palette entry

```
colnum = INTIN[0]
if 0 <= colnum < numcolors:
    rgb[i] = clamp(INTIN[1+i], 0, 1000)   # i=0..2, R/G/B in 1/1000
    REQ_COL[colnum] = rgb                 # remember the request
    set_color(colnum, rgb)                # program the hardware palette
```

Out-of-range `colnum` is ignored. RGB components are VDI's 0–1000
range; the driver scales to the hardware DAC width. `vq_color` (op 26,
Batch 4) reads `REQ_COL` back.

## NewTOS implementation notes

- `wrt_mode` maps to the framebuffer blit op: replace = store,
  transparent = store where pattern set, XOR = `^=`, reverse-transparent
  = store background where pattern clear.
- `set_color` writes the per-device palette (ADR-011) and, for a
  truecolor framebuffer, also caches the packed RGB so PEL→RGB lookup
  stays O(1). Bring-up screen is palette-based, so this is a direct DAC
  write.
- Clip bounds come from the device's `xres`/`yres`
  (`vwk->device`), not a global — multi-screen safe (ADR-011).
