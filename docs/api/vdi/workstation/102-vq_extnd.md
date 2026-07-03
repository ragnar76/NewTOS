# `vq_extnd` — VDI opcode 102 (extended workstation inquire)

> Phase 00b spec (full-contract template). Source: EmuTOS @ `1a7484ab` —
> `vdi/vdi_control.c:626` (`vdi_vq_extnd`), `:76-123` (`INQ_TAB_rom`),
> `:193` (`update_rez_dependent`, patches `INQ_TAB[4]/[5]`).
> Calling convention: [`../_dispatcher.md`](../_dispatcher.md).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract:** opcode 102; the `INTIN[0]` mode flag; mode-0 returns the
> same tables as `v_opnvwk`; mode-1 returns the current clip rect in
> `PTSOUT[0..3]` plus the 45-word `INQ_TAB` with the live clip flag at
> index 19. **Reference:** the `INQ_TAB` storage and the `flip_y` write.

Returns extended device information. The `INTIN[0]` flag selects between
re-returning the standard open-workstation tables (mode 0) and returning
the *extended* inquiry table plus the current clipping rectangle (mode 1).

## EmuTOS source

**Primary location:** `vdi/vdi_control.c:626` — `vdi_vq_extnd(Vwk *vwk)`
(reached via `jmptb2[2]`, `vdi_main.c:77`).

**Globals read:** `INTIN[0]` (mode), `SIZ_TAB`, `DEV_TAB`, `INQ_TAB`,
`vwk->xmn_clip/ymn_clip/xmx_clip/ymx_clip`, `vwk->clip`.
**Globals written:** `flip_y = 1`, `INQ_TAB[19]` (updated to the live
clip flag), the caller's `INTOUT`/`PTSOUT`.

## Source extraction summary

`vq_extnd` sets `flip_y = 1`. In **mode 0** (`INTIN[0] == 0`) it copies
the 12-word `SIZ_TAB` into `PTSOUT` and the 45-word `DEV_TAB` into
`INTOUT` — identical to what `v_opnvwk` returns. In **mode 1**
(`INTIN[0] != 0`) it writes the workstation's current clip rectangle into
`PTSOUT[0..3]` (`xmn,ymn,xmx,ymx`), zeroes `PTSOUT[4..11]`, updates
`INQ_TAB[19]` to the queried `Vwk`'s live `clip` flag, and copies the
45-word `INQ_TAB` into `INTOUT`. The dispatcher pre-loads `CONTRL[2]=6`,
`CONTRL[4]=45` (`vdi_main.c:77`).

## Trap interface

| | |
|--|--|
| Opcode | `CONTRL[0] = 102` |
| Handle | `CONTRL[6]` (resolved to the queried `Vwk`) |
| `# INTIN` (`CONTRL[3]`) | 1 — `INTIN[0]` = mode (0 = standard tables, ≠0 = extended) |
| `# INTOUT / # PTSOUT` | 45 / 6 |

Native ABI: GEM trap `svc #2`, selector 115 in `x0`, `VDIPB*` in `x1`.

## Inputs

| Idx | Name | Meaning |
|-----|------|---------|
| `INTIN[0]` | mode | `0` → return `SIZ_TAB`/`DEV_TAB` (as `v_opnvwk`); `≠0` → return the clip rect + `INQ_TAB`. |

## Outputs

### Mode 0 (`INTIN[0] == 0`)

Identical to [`v_opnvwk`](100-v_opnvwk.md): `INTOUT[0..44]` = `DEV_TAB`
(45 WORDs), `PTSOUT[0..11]` = `SIZ_TAB` (12 WORDs). See that spec for the
full byte-index tables.

### Mode 1 (`INTIN[0] != 0`)

- `PTSOUT[0..3]` = the queried workstation's current clipping rectangle
  `xmn_clip, ymn_clip, xmx_clip, ymx_clip`; `PTSOUT[4..11]` = 0.
- `INTOUT[0..44]` = `INQ_TAB` (45 extended-capability values), with
  `INQ_TAB[19]` set to the queried `Vwk`'s live `clip` flag first.

### `INQ_TAB` (45 WORDs, extended capabilities)

ROM template `INQ_TAB_rom` (`vdi_control.c:76-123`); `[4]`/`[5]` patched
by `update_rez_dependent`:

| Idx | Meaning | ROM value | Patched |
|-----|---------|-----------|---------|
| 0 | alpha/graphic controller type | 4 | — |
| 1 | # background colours | 1 | — |
| 2 | text styles supported (bitmask) | `0x1F` | — |
| 3 | scale rasters (0=no) | 0 | — |
| 4 | # planes | 1 | **`v_planes`** |
| 5 | video lookup table | 0 | **0 if 16-plane or mono monitor, else 1** |
| 6 | performance factor | 50 | — |
| 7 | contour-fill capability | 1 | — |
| 8 | character-rotation capability | 1 | — |
| 9 | # writing modes | 4 | — |
| 10 | highest input mode | 2 | — |
| 11 | text alignment flag | 1 | — |
| 12 | inking capability | 0 | — |
| 13 | rubber banding | 0 | — |
| 14 | max vertices | `MAX_VERTICES` (512/1024) | — |
| 15 | max `INTIN` (−1 = unbounded) | −1 | — |
| 16 | # mouse buttons | 1 | — |
| 17 | styles for wide lines | 0 | — |
| 18 | writing modes for wide lines | 0 | — |
| 19 | **clip flag** (filled at query time) | 0 | **`vwk->clip`** |
| 20 | extended-precision pixel-size info | 0 | — |
| 21 | pixel width (1/10..1/1000 micron) | 0 | — |
| 22 | pixel height (1/10..1/1000 micron) | 0 | — |
| 23 | horizontal resolution (dpi) | 0 | — |
| 24 | vertical resolution (dpi) | 0 | — |
| 25–27 | reserved | 0 | — |
| 28 | bezier flag (bit 1) | 0 | — |
| 29 | reserved | 0 | — |
| 30 | raster flag (does `vro_cpyfm` scale?) | 0 | — |
| 31–39 | reserved | 0 | — |
| 40 | unprintable left border (px) | 0 | — |
| 41 | unprintable upper border (px) | 0 | — |
| 42 | unprintable right border (px) | 0 | — |
| 43 | unprintable lower border (px) | 0 | — |
| 44 | page size (printers) | 0 | — |

Indices 40–44 are printer/plotter-only (all 0 on the screen).

## Error conditions

None — VDI has no error return. (A bad `CONTRL[6]` is a no-op in the
dispatcher before `vq_extnd` runs.)

## Verification criteria

- **Mode 0 == v_opnvwk:** `vq_extnd(INTIN[0]=0)` returns `INTOUT`/`PTSOUT`
  byte-identical to a matching `v_opnvwk` open (same `DEV_TAB`/`SIZ_TAB`).
- **Mode 1 clip rect:** with the workstation's clip set to
  `(x1,y1,x2,y2)`, `vq_extnd(INTIN[0]=1)` returns
  `PTSOUT[0..3] = x1,y1,x2,y2` and `PTSOUT[4..11] = 0`.
- **Mode 1 clip flag:** `INTOUT[19]` equals the queried workstation's live
  `clip` flag (0 when clipping disabled, 1 when enabled) — **per-`Vwk`**,
  not a global.
- **Planes:** on a `p`-plane screen, `INTOUT[4] = p`.

## Edge cases discovered in source

1. **`INQ_TAB[19]` is a shared global patched per query** — `vq_extnd`
   writes the *queried* `Vwk`'s clip flag into the one `INQ_TAB[19]` slot
   before copying it out (`vdi_control.c:651`). Two workstations with
   different clip flags each read their own value because the write
   happens per call.
2. **Mode is any non-zero value**, not a specific `1` — `INTIN[0] != 0`
   selects extended mode (`:633`).
3. **`flip_y` is forced to 1** at entry (`:631`), like `init_wk`.

## Dependencies

### Requires
- An open physical workstation (populates `DEV_TAB`/`SIZ_TAB`/`INQ_TAB`);
  a valid queried `Vwk` for the clip fields.

### Used by
- AES (`gsx_1code(EXTENDED_INQUIRE,1)`, `aes/gemgsxif.c:499`) and
  applications probing device capabilities.

## Related data structures

- [`_state.md`](../_state.md) — `DEV_TAB`/`SIZ_TAB`/`INQ_TAB` ownership.
- [`../structures/vwk.md`](../../structures/vwk.md) — the clip fields.

## Cross-references

### Atari Compendium / Profibuch
- Not present in `references/` (no PDFs). Cross-check of the `INQ_TAB`
  index meanings is **deferred**.

## NewTOS implementation notes (Phase 12)

- `INQ_TAB` is per **physical device** (ADR-011): each screen device
  reports its own planes/colours/resolution. The screen bring-up profile
  reports a palette-class device; `[14]` (max vertices) and `[15]` (max
  intin) bound the array sizes the handlers accept.
- Index 19 must reflect the **queried** `Vwk`'s clip flag, not a global.
- **Open question:** confirm `INQ_TAB[4]` (# planes) and `[5]` (video
  lookup table) for the NewTOS 640×480×16-colour framebuffer (4 planes;
  `[5]` = 1 for a colour LUT) — `_discrepancies.md` D-VDI-7.

---

## Pi / BCM2711 binding

> Re-points the *hardware reference* (`INQ_TAB` storage, `v_planes`) at the
> BCM2711 VideoCore framebuffer read through the `RasterBackend`. The
> **contract is unchanged** — opcode 102, the `INTIN[0]` mode flag, the
> mode-0 tables, the mode-1 clip rect + 45-word `INQ_TAB`. Peripheral +
> driver: see `docs/HARDWARE-PROFILE.md` §2/§3.1.

### Open question resolved — `INQ_TAB[4]`/`[5]` for 640×480×16-colour

Confirmed from the code (`tos-api/src/vdi/workstation.rs:155-160`):

| Idx | Meaning | NewTOS value | Source |
|-----|---------|--------------|--------|
| 4 | # planes | **4** | `ncol.trailing_zeros()` = `log2(16)` (`workstation.rs:158`) |
| 5 | video lookup table | **1** | `ncol>2 → 1` (`workstation.rs:159`) |
| 14 | max vertices | **256** | `MAX_VERTICES` (`workstation.rs:42`) |
| 19 | clip flag | live `vwk.clip` | per-query (`workstation.rs:160`) |
| 16 | # mouse buttons | 1 | ROM template (`workstation.rs:49-56`) |

So `INQ_TAB[4] = 4` (four planes for 16 colours) and `[5] = 1` (colour
LUT present) — exactly the values the D-VDI-7 open question predicted.
Mode-0 returns the same `DEV_TAB`/`SIZ_TAB` as
[`100-v_opnvwk`](100-v_opnvwk.md#pi--bcm2711-binding) (`[0]=639`,
`[1]=479`, `[13]=16`, `[35]=1`, `[39]=16`).

### Mismatch flagged (Step-2)

- **`INQ_TAB[14]` max vertices = 256, not 512/1024.** This spec's table
  row 14 lists "MAX_VERTICES (512/1024)". NewTOS reports **256**
  (`workstation.rs:42`, `primitives.rs:115`) — the smaller NewTOS profile
  (same as the handle-count reduction in `100-v_opnvwk`). Not a contract
  violation (the cap is device-reported), but the spec text should read
  256 for NewTOS.

### Pixel-size / resolution `INQ_TAB` fields

`INQ_TAB[20..24]` (extended pixel-size / dpi) stay `0` in the ROM template
(`workstation.rs:53-55`) — NewTOS does not patch them, so pixel width/
height in microns and dpi are reported as "unknown" (0). Consistent with
`DEV_TAB[3]/[4]` staying at the ROM default (see
[`100-v_opnvwk`](100-v_opnvwk.md#pi--bcm2711-binding) mismatch 2); a
truthful value needs the mailbox physical-display-size query, not yet done.

### 1080p later

`INQ_TAB[4]/[5]` derive from `num_colors` (16), which is fixed by the VDI
CLUT model, not the resolution; a larger framebuffer does not change the
plane/LUT answers until a truecolor path is adopted.
