# `v_opnvwk` — VDI opcode 100 (open virtual workstation)

> Phase 00b spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `vdi/vdi_control.c:407` (`vdi_v_opnvwk`), `:269`
> (`init_wk`), `:390` (`build_vwk_chain`), `:128-174` (`DEV_TAB_rom`),
> `:58-71` (`SIZ_TAB_rom`), `:193` (`update_rez_dependent`),
> `vdi/vdi_defs.h:36-115` (constants, `CONTRL[]` indices).
> Calling convention: [`../_dispatcher.md`](../_dispatcher.md).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract:** opcode 100, the 11-word `work_in` (`INTIN`) input layout
> and its validation/clamping rules, the returned handle in `CONTRL[6]`,
> and the `DEV_TAB` (`INTOUT`, 45 WORDs) / `SIZ_TAB` (`PTSOUT`, 12 WORDs)
> byte-index contents. **Reference:** the `Mxalloc`'d `Vwk` heap block, the
> `vwk_ptr[]`/`CUR_WORK`/`next_work` chain, `MAP_COL[]`, `st_fl_ptr` —
> NewTOS reimplements these.

Opens a **virtual workstation** on the screen device: finds a free
handle, allocates a `Vwk` attribute area, initialises it from the
caller's `work_in` inputs, links it into the workstation chain, and
returns a fresh **handle** plus the device's capability tables. AES and
applications open a virtual workstation to draw with independent
attributes while sharing the one physical screen.

## EmuTOS source

**Primary location:** `vdi/vdi_control.c:407` — `vdi_v_opnvwk(Vwk *vwk)`
(reached via `jmptb2[0]`, `vdi_main.c:75`).

**Installed handler / vector:** (none) — direct jump-table entry.

**Helpers called:**
- `vdi/vdi_control.c:269` — `init_wk()` — validate `work_in`, set `Vwk`
  attribute defaults, and copy `DEV_TAB`→`INTOUT`, `SIZ_TAB`→`PTSOUT`.
- `vdi/vdi_control.c:390` — `build_vwk_chain()` — link all open `Vwk`s.
- `vdi/vdi_control.c:224` — `validate_color_index()` — clamp a palette
  index (out-of-range → 1).
- `st_fl_ptr()` / `text_init2()` — set the fill pattern and text metrics
  (`vdi_defs.h:305,329`).
- `Mxalloc(size, MX_SUPER)` (GEMDOS 0x44) — allocate the `Vwk`.

**Headers / type definitions used:**
- `vdi/vdi_defs.h:171-221` — `Vwk` struct (attribute area).
- `vdi/vdi_defs.h:45-82` — `MIN/MAX/DEF_*` validation constants.
- `vdi/vdi_control.c:35` — `MX_SUPER = (3<<4)` (Mxalloc mode; ignored by
  EmuTOS, only meaningful under FreeMiNT).

**System variables / globals read:** `vwk_ptr[]` (free-handle scan),
`DEV_TAB`/`SIZ_TAB` (`bios/lineavars.S`), `xres`/`yres`
(`= DEV_TAB[0]/[1]`), `MAP_COL[]`, `ROM_UD_PATRN`, `LINE_STYLE[0]`.
**System variables / globals written:** `vwk_ptr[handle]`, `CONTRL[6]`,
`CUR_WORK`, `flip_y` (set to 1 by `init_wk`), the new `Vwk`.

**Build configuration affecting this function:**
- `CONF_WITH_VDI_16BIT` — when the mode is TrueColor (`v_planes > 8`),
  the allocation is enlarged by `sizeof(VwkExt)` and a pseudo-palette is
  built (`vdi_control.c:439-453`, `:354-363`). **Out of NewTOS bring-up
  scope** (8-bit/palette first).
- `HAVE_BEZIER` (0 here) — would init `vwk->bez_qual` (`:365-375`).

---

## Source extraction summary

`v_opnvwk` first sets `CUR_WORK = &phys_work` so `CUR_WORK` stays valid
even on an early exit. It scans `vwk_ptr[VDI_PHYS_HANDLE+1 ..
LAST_VDI_HANDLE]` for a free slot; if none is free it sets
`CONTRL[6] = 0` and returns (no error code). It then `Mxalloc`s a `Vwk`
in supervisor-accessible memory; on allocation failure it likewise sets
`CONTRL[6] = 0` and returns. On success it stores the pointer in
`vwk_ptr[handle]`, records the handle in both `vwk->handle` and
`CONTRL[6]`, runs `init_wk`, rebuilds the workstation chain, and makes
the new `Vwk` current (`CUR_WORK = vwk`).

`init_wk` reads the 11-word `work_in` array (`INTIN[0..10]`) and stores a
validated attribute into the `Vwk`. Each attribute is range-checked: an
out-of-range value falls back to a documented default (`DEF_*`), never an
error. Line/marker/fill *style* indices are stored **0-based** (`value-1`)
in the `Vwk`; colour indices are validated and mapped through `MAP_COL[]`.
`work_in[5]` (text font) is **ignored** — you always get the default
font. `init_wk` also sets attributes not carried in `work_in` (marker
height/scale, replace write mode, default line width and squared ends,
fill-perimeter on, clip disabled covering the whole screen, the ROM
default fill pattern, `multifill=0`, `ud_ls`). Finally it copies the live
`DEV_TAB` (45 WORDs) into `INTOUT` and `SIZ_TAB` (12 WORDs) into `PTSOUT`,
and sets `flip_y = 1`.

The virtual workstations are chained together (`build_vwk_chain`) because
some programs (notably Warp9) depend on the chain existing — a
**reference** detail NewTOS keeps for compatibility but need not build the
same way.

---

## Trap interface

- **Opcode:** `CONTRL[0] = 100`
- **Sub-opcode:** none
- **Handle:** `CONTRL[6]` — **ignored on input**; set to the new handle
  on output (`0` on failure).
- **Input counts:** `# INTIN = CONTRL[3] = 11` (`work_in`);
  `# PTSIN = CONTRL[1] = 0`.
- **Output counts:** `# INTOUT = CONTRL[4] = 45` (pre-loaded by the
  dispatcher), `# PTSOUT = CONTRL[2] = 6` (pre-loaded; `init_wk` writes
  12 WORDs = 6 pairs).
- **Native (AArch64) ABI:** GEM trap `svc #2`, selector 115 in `x0`,
  `VDIPB*` in `x1` (see [`../_dispatcher.md`](../_dispatcher.md)). No
  register return value; results are in `CONTRL[6]`/`INTOUT`/`PTSOUT`.

---

## Inputs — `INTIN` = `work_in[11]` (`init_wk`, `vdi_control.c:269-316`)

| Idx | `work_in` field | Validation → stored value (else default) |
|-----|-----------------|------------------------------------------|
| 0 | device id | Driver/device id (screen). `v_opnwk` uses `rez+2` here; for `v_opnvwk` it selects the screen driver. Not range-checked by `init_wk`. |
| 1 | line type | `MIN_LINE_STYLE(1)..MAX_LINE_STYLE(7)` → `line_index = v-1`; else `DEF_LINE_STYLE(1)` (`:279-281`). |
| 2 | line colour | `validate_color_index` → `line_color = MAP_COL[v]` (out-of-range → index 1) (`:283-284`). |
| 3 | marker type | `MIN_MARK_STYLE(1)..MAX_MARK_STYLE(6)` → `mark_index = v-1`; else `DEF_MARK_STYLE(3)` (`:286-289`). |
| 4 | marker colour | `validate_color_index` → `mark_color = MAP_COL[v]` (`:291-292`). |
| 5 | text font | **ignored** — pointer advanced, value discarded; you always get the default font (`:294-295`). |
| 6 | text colour | `validate_color_index` → `text_color = MAP_COL[v]` (`:297-298`). |
| 7 | fill interior style | `MIN_FILL_STYLE(0)..MAX_FILL_STYLE(4)` → `fill_style`; else `DEF_FILL_STYLE(0=hollow)` (`:303-304`). |
| 8 | fill style index | if `fill_style==FIS_PATTERN(2)`: `1..MAX_FILL_PATTERN(24)` else `DEF_FILL_PATTERN(1)`; otherwise `1..MAX_FILL_HATCH(12)` else `DEF_FILL_HATCH(1)`. Stored in `fill_index` (`:306-311`). |
| 9 | fill colour | `validate_color_index` → `fill_color = MAP_COL[v]` (`:313-314`). |
| 10 | coordinate system | stored verbatim in `vwk->xfm_mode` (`:316`): `0` = NDC (normalised), `2` = RC (raster). |

Non-`work_in` defaults `init_wk` also sets (`:300-343`):
`mark_height = DEF_MKHT`, `mark_scale = 1`, `wrt_mode = WM_REPLACE`,
`line_width = DEF_LWID`, `line_beg = line_end = SQUARED`, `fill_per = TRUE`,
clip rect `[0,0]..[xres,yres]` with `clip = FALSE`, the ROM default fill
pattern copied into `ud_patrn[]`, `multifill = 0`, `ud_ls = LINE_STYLE[0]`,
`next_work = NULL`, `flip_y = 1`.

---

## Outputs

### Return

- `CONTRL[6]` = the new handle on success (in `[2 .. NUM_VDI_HANDLES]`,
  since handle 1 is the physical workstation); **`0`** = open failed.
- `INTOUT[0..44]` = the device capability table (`DEV_TAB`, 45 WORDs).
- `PTSOUT[0..11]` = the size table (`SIZ_TAB`, 12 WORDs = 6 pairs).

There is **no register/`D0` return value** — VDI results live in the
parameter-block arrays.

### `INTOUT` = `DEV_TAB` (45 WORDs, device capabilities)

The ROM template `DEV_TAB_rom` (`vdi_control.c:128-174`) is copied into
`DEV_TAB` at `v_opnwk` time, then patched with the live screen geometry
by `update_rez_dependent` (`:193-214`). `v_opnvwk`/`init_wk` return the
already-patched `DEV_TAB`. Byte-exact template values and their patched
overrides:

| Idx | Capability | ROM value | Patched by `update_rez_dependent` |
|-----|-----------|-----------|-----------------------------------|
| 0 | x resolution − 1 | 639 | **`V_REZ_HZ - 1`** |
| 1 | y resolution − 1 | 399 | **`V_REZ_VT - 1`** |
| 2 | device precision (0=exact) | 0 | — |
| 3 | pixel width | 372 | **`get_pixel_size()` (x)** |
| 4 | pixel height | 372 | **`get_pixel_size()` (y)** |
| 5 | # text font heights | 3 | — |
| 6 | # line styles | `MAX_LINE_STYLE` (7) | — |
| 7 | line-width flag | 0 | — |
| 8 | # marker types | 6 | — |
| 9 | # marker sizes | 8 | — |
| 10 | # text fonts | 1 | — |
| 11 | # area (fill) patterns | `MAX_FILL_PATTERN` (24) | — |
| 12 | # crosshatch patterns | `MAX_FILL_HATCH` (12) | — |
| 13 | # simultaneous colours | 2 | **`(v_planes<8)?(1<<v_planes):256`** |
| 14 | # GDPs | 10 | — |
| 15–24 | GDP ids | 1..10 | — (bar, arc, pie, circle, ellipse, ell-arc, ell-pie, rounded-rect, filled-rounded-rect, justified-text) |
| 25–34 | GDP attribute codes (#1..#10) | 3,0,3,3,3,0,3,0,3,2 | — |
| 35 | colour capability (0=mono) | **0** | **`(v_planes==1)?0:1`** |
| 36 | text rotation | 1 | — |
| 37 | polygon fill | 1 | — |
| 38 | cell array | 0 | — |
| 39 | palette size | 2 | **`get_palette()`** |
| 40 | # locator devices (mouse) | 2 | — |
| 41 | # valuator devices | 1 | — |
| 42 | # choice devices | 1 | — |
| 43 | # string devices | 1 | — |
| 44 | workstation type (2 = in/out) | 2 | — |

> Note the ROM `[35]` = **0**; the colour-capability flag a program reads
> is the *patched* value (1 for any multi-plane screen). Index 39 is
> `get_palette()`, not the literal ROM `2`. (An earlier revision labelled
> `[35]` as if the ROM value were the contract — clarified.)

> **Re-validation 2026-07-09 — `DEV_TAB[39]`'s real semantics (`get_palette`,
> `bios/screen.c:831-864`).** Despite the source's own "Palette size"
> comment (`vdi_control.c:168`), `get_palette()` does **not** return the
> number of simultaneously displayable colours — that is `DEV_TAB[13]`.
> It returns the **RGB precision available to `vs_color`** (how many
> distinct values the hardware DAC can hold per gun, expressed as a total
> choice count): **`4096`** for any STe/TT-style colour shifter or a Videl
> mode with `VIDEL_COMPAT`/4bpp (`screen.c:850,843-845`), **`2`** for a
> monochrome screen (`ST_HIGH`/`TT_HIGH`, `screen.c:853-857`), **`512`**
> for plain-ST colour modes without an STe shifter (`screen.c:858-864`),
> or **`0`** for a Videl truecolor mode without `VIDEL_COMPAT`
> (`screen.c:846`, "no discrete palette"). None of these values equal
> `DEV_TAB[13]`'s colour-count for the corresponding mode (e.g. STe
> 16-colour ST-medium: `[13]=16`, `[39]=4096`). See the "Mismatch flagged"
> note in the Pi/BCM2711 binding section below — the current NewTOS code
> substitutes the colour count (`ncol`) for this field, which is a
> different EmuTOS quantity, not an equivalent one.

### `PTSOUT` = `SIZ_TAB` (12 WORDs = 6 pairs, device coords)

ROM template `SIZ_TAB_rom` (`vdi_control.c:58-71`); `text_init` fills the
font-metric entries at `v_opnwk` (`0/2` char width, `1/3` char height):

| Idx | Size | ROM value |
|-----|------|-----------|
| 0 | min char width | 0 (set by `text_init`) |
| 1 | min char height | 7 (set by `text_init`) |
| 2 | max char width | 0 (set by `text_init`) |
| 3 | max char height | 7 (set by `text_init`) |
| 4 | min line width | 1 |
| 5 | reserved | 0 |
| 6 | max line width | `MAX_LINE_WIDTH` (40) |
| 7 | reserved | 0 |
| 8 | min marker width | 15 |
| 9 | min marker height | 11 |
| 10 | max marker width | 120 |
| 11 | max marker height | 88 |

### Side effects on global state

- Allocates one `Vwk` (`Mxalloc`), fills `vwk_ptr[handle]`, sets
  `CUR_WORK` and `CONTRL[6]`, rebuilds the `next_work` chain, sets
  `flip_y = 1`.

### Side effects on hardware

- None directly (no resolution change; that is `v_opnwk`).

---

## Error conditions

VDI has **no error return.** The two failure paths both set `CONTRL[6] = 0`
and return with no `Vwk` allocated:

| Condition | Source location | Result |
|-----------|-----------------|--------|
| No free handle (all `NUM_VDI_HANDLES` virtual slots in use) | `vdi_control.c:425-428` | `CONTRL[6] = 0` |
| `Mxalloc` out of memory | `vdi_control.c:444-447` | `CONTRL[6] = 0` |

**Error paths not in Compendium:** the "no free handle" cap is
`NUM_VDI_HANDLES` (build-config: 64 or 128), not a fixed 16.

---

## Verification criteria

- **Nominal open:** `v_opnvwk` with a valid `work_in` returns a
  `CONTRL[6]` handle in `[2 .. NUM_VDI_HANDLES]`, `CONTRL[4]=45`,
  `CONTRL[2]=6`.
- **DEV_TAB geometry:** for a 640×480 framebuffer, `INTOUT[0]=639`,
  `INTOUT[1]=479`; `INTOUT[44]=2` (workstation type in/out);
  `INTOUT[14]=10` (# GDPs).
- **Colour capability:** on a multi-plane (≥2-plane) screen `INTOUT[35]=1`;
  on a 1-plane mono screen `INTOUT[35]=0`.
- **SIZ_TAB:** `PTSOUT[6]=40` (max line width), `PTSOUT[8]=15`,
  `PTSOUT[10]=120` (marker widths).
- **Input clamping:** `work_in[1]=99` (invalid line type) yields
  `line_index = DEF_LINE_STYLE-1 = 0`; `work_in[5]` (font) is ignored.
- **Exhaustion:** after `NUM_VDI_HANDLES` successful opens without closes,
  the next `v_opnvwk` returns `CONTRL[6]=0`.

---

## Edge cases discovered in source

1. **`work_in[5]` (text font) is silently ignored** — you always get the
   default font (`vdi_control.c:294-295`).
2. **Style indices are stored 0-based** in the `Vwk` (`line_index`,
   `mark_index` = `value-1`), while `fill_index` keeps the 1-based value
   (`:281,289,311`). This matters for the inquiry round-trip (see
   `_discrepancies.md` D-VDI-1 for the marker asymmetry).
3. **Handle count is `NUM_VDI_HANDLES`** (64 or 128), not 16
   (`vdi_control.c:25-27`; `config.h:391-393,1660-1662`). Handle 1 is the
   physical workstation, so virtual handles run `2..NUM_VDI_HANDLES`.
4. **`CUR_WORK` is set to `&phys_work` on entry** so it stays valid even
   if the open fails early (`vdi_control.c:417`).
5. **`flip_y` is set to 1** by `init_wk` (`:379`) — the driver's
   coordinate/magnitude flag.

---

## Dependencies

### Requires
- `Mxalloc`/`Mfree` (GEMDOS 0x44/0x49) for the `Vwk`.
- A prior `v_opnwk` (the physical workstation supplies `DEV_TAB`/`SIZ_TAB`
  and the palette via `init_colors`/`text_init`).
- `validate_color_index`/`MAP_COL[]`, `st_fl_ptr`, `text_init2`.

### Used by
- AES (`gsx_1code`/`gsx2`) and every application opening a virtual
  workstation to draw.

---

## Related data structures

- [`../structures/vwk.md`](../../structures/vwk.md) — the `Vwk` attribute area.
- [`_state.md`](../_state.md) — the `vwk_ptr[]`/`CUR_WORK`/chain model,
  `DEV_TAB`/`SIZ_TAB`/`INQ_TAB` ownership.

---

## Cross-references

### Atari Compendium / Profibuch
- Not present in `references/` (no PDFs). Cross-check of the `work_in`/
  `work_out` layouts and `DEV_TAB` index meanings is **deferred**.

---

## Configuration / version notes

- `NUM_VDI_HANDLES` = 64 (small/STe target) or 128 (default) — build-config,
  see [`../_dispatcher.md`](../_dispatcher.md). `MAX_VERTICES` is **1024**
  for the NewTOS profile (resolved 2026-09-04, same file).
- `CONF_WITH_VDI_16BIT` (TrueColor) enlarges the allocation and builds a
  pseudo-palette — out of NewTOS bring-up scope.

---

## NewTOS implementation notes (Phase 12)

- The `Vwk` is a real heap allocation (`Malloc`/`Mxalloc` from Phase
  04/06). Keep the handle cap and the `CUR_WORK`/chain model for
  compatibility with programs that walk the chain (Warp9-class), but the
  chain storage itself is **reference**, not contract.
- **Per-device / multi-screen (ADR-011).** The *physical* workstation is a
  per-device struct (own `Framebuffer` + geometry + palette) in a
  screen-device registry, not EmuTOS's global `phys_work` / Line-A screen
  vars. Each `Vwk` links to its physical device; the `RasterBackend`
  (`tos-api/src/vdi/backend.rs`) draws into that device's framebuffer.
- `DEV_TAB`/`SIZ_TAB` are filled from the framebuffer geometry
  (`hal::framebuffer`) and the bring-up palette: `[0]=width-1`,
  `[1]=height-1`, `[13]=#colours`, `[35]=1` (multi-plane), `[39]`=palette
  size. **Open question:** confirm the exact `[13]/[35]/[39]` values
  NewTOS reports for a 640×480×16-colour (4-plane) framebuffer, and
  whether `[3]/[4]` (pixel size in microns) are reported truthfully or
  left at the ROM `372` — see `_discrepancies.md` D-VDI-7.
- `work_in[10]` (NDC vs RC): the screen workstation is RC (raster, origin
  top-left); NDC scaling is a later refinement.

---

## Pi / BCM2711 binding

> Re-points the *hardware reference* (`V_REZ_HZ`/`V_REZ_VT`/`v_planes`
> Line-A screen vars, the `Mxalloc`'d `Vwk` chain) at the BCM2711
> VideoCore framebuffer read through the `RasterBackend`. The **contract is
> unchanged** — opcode 100, the `work_in` layout/validation, the returned
> handle, the 45-word `DEV_TAB` / 12-word `SIZ_TAB` byte indices.
> Peripheral + driver: see `docs/HARDWARE-PROFILE.md` §2/§3.1.

> **ADR-017 cross-reference (2026-07-09, `docs/MEMORY.md`).** The
> 640×480×16-colour numbers below are the **currently staged bring-up
> geometry**, not a permanent fixed target: NewTOS's resolution policy is
> now native/geometry-independent (default 1920×1080, switchable
> 1280×800), and per ADR-017's Staging rule the physical framebuffer
> deliberately stays at 640×480 until geometry independence is verified
> end-to-end — only then does the target geometry change. This spec's
> "the fixed 640×480 is only in the driver, not in the VDI table code"
> point below is exactly the property ADR-017 relies on: `init_wk`
> already reads `backend.width()/height()/num_colors()` fresh
> (`workstation.rs:62-64,101-106`), so `DEV_TAB[0,1,13,35,39]` will follow
> the framebuffer driver's geometry the moment it changes, with **no VDI
> code change required**. The concrete `639`/`479`/`16` values below hold
> for *today's* staged geometry only.

### Resolved `DEV_TAB` / `SIZ_TAB` values for the 640×480×16-colour framebuffer

The ROM template `DEV_TAB_ROM`/`SIZ_TAB_ROM` lives in
`tos-api/src/vdi/workstation.rs:24-39`; `init_wk` patches the
geometry/colour entries from the live `RasterBackend`
(`workstation.rs:61-117`). The backend reports `width=640`, `height=480`,
`num_colors=16` (`kernel/src/vdi_backend.rs:130-140`, `NUM_COLORS=16` at
`:21`). Concrete patched values:

| Idx | Capability | NewTOS value | Source |
|-----|-----------|--------------|--------|
| 0 | x res − 1 | **639** | `xres-1`, `width()=640` (`workstation.rs:102`) |
| 1 | y res − 1 | **479** | `yres-1`, `height()=480` (`workstation.rs:103`) |
| 3 | pixel width (µm) | **372** (ROM default, **not** patched) | see mismatch below |
| 4 | pixel height (µm) | **372** (ROM default, **not** patched) | see mismatch below |
| 13 | # simultaneous colours | **16** | `ncol` (`workstation.rs:104`) |
| 14 | # GDPs | 10 | ROM (`workstation.rs:27`) |
| 35 | colour capable (0=mono) | **1** | `ncol>2 → 1` (`workstation.rs:105`) |
| 39 | palette size | **16** | `ncol` (`workstation.rs:106`) |
| 44 | workstation type | 2 (in/out) | ROM |

`SIZ_TAB` (`workstation.rs:35-39`) is fixed: min/max char `0..7`, min
line width `1` / max `40`, marker widths `15/120`, heights `11/88`. Note
NewTOS does **not** run a `text_init` that overwrites the char-metric
entries — they come straight from the template.

### Handle count and vertex cap (resolved TBVs)

- **`NUM_VDI_HANDLES` → 16.** NewTOS uses `HANDLE_COUNT = 16`
  (`tos-api/src/vdi/vwk.rs:20`): handle 1 = physical workstation, 2..=16
  virtual (a **static slot table**, not `Mxalloc`). This is the small
  NewTOS profile, **not** the EmuTOS 64/128. The spec's "no free handle"
  cap and the exhaustion criterion are therefore at **16**, not
  `NUM_VDI_HANDLES = 64/128` — see the mismatch note.
- **`MAX_VERTICES` → 1024** (resolved 2026-09-04, was 256 then 512).
  Polygon vertex cap and `INQ_TAB[14]` share one constant
  (`tos-api/src/vdi/primitives.rs`, `MAX_VERTS`, referenced from
  `workstation.rs`). 1024 is the EmuTOS default profile value —
  `config.h` never tests `TARGET_512`, so the `make 512` reference build
  takes `include/config.h:1652-1655` — and it is what the measured TT
  reference reports. See [`../_dispatcher.md`](../_dispatcher.md).

### Mismatches flagged (Step-2 candidates)

1. **Handle cap 16, not 64/128.** This spec (§ Error conditions, § Edge
   cases 3, § Configuration) states the cap is `NUM_VDI_HANDLES = 64/128`.
   NewTOS pins it at **16** (`vwk.rs:20`). Not a contract violation (the
   VDI cap is build-config), but the spec text should read 16 for NewTOS.
2. **Pixel size `[3]/[4]` left at ROM `372`.** `init_wk` never patches
   `DEV_TAB[3]/[4]` (`workstation.rs:101-106` patches only `[0],[1],[13],
   [35],[39]`), so both report the Atari ROM default `372` (≈ 0.372 mm ≈
   the ST's pixel pitch), **not** the true Pi/HDMI pixel size. The profile
   §3.1 said "report truthfully or leave at ROM default"; NewTOS leaves it
   at ROM. Acceptable for bring-up (few programs read pixel size); a
   truthful value needs the physical display dimensions, which the mailbox
   can report but NewTOS does not yet query.
3. **`text_init` char metrics not applied.** EmuTOS's `text_init` fills
   `SIZ_TAB[0..3]`; NewTOS uses the template `0/7/0/7` directly. Values
   happen to match the common case; flagged for completeness.
4. **`DEV_TAB[39]` reports the colour count, not `get_palette()`'s
   semantics (re-validation 2026-07-09).** `workstation.rs:106` sets
   `dev[39] = ncol as i16` (`16`) — but per the source analysis above,
   EmuTOS's `DEV_TAB[39]` is the DAC's RGB-choice count (`4096`/`2`/`512`/
   `0`), a different quantity than `DEV_TAB[13]`'s colour count, which
   NewTOS already reports correctly and separately at `[13]`. Concretely:
   for a 16-colour palette screen with a real RGB DAC, the EmuTOS-faithful
   `[39]` would be a colour-precision figure (the closest analogue,
   `4096, ` since NewTOS's `vs_color` also accepts 0..1000-per-gun input
   like STe/TT), not `16`. **Open question, not decided here:** whether
   any real-world program branches on `DEV_TAB[39]` for a palette (as
   opposed to truecolor) screen — if not, the mismatch is cosmetic; if a
   program uses it to decide "is this greyscale/2-colour vs. full RGB", a
   wrong `16` could misinform it. Flagged for the Developer's VDI code
   audit, not fixed here (docs-only, CLAUDE.md Rule to not edit `.rs`).

### Palette / colour model

The 16 `DEV_TAB[13]`/`[39]` colours are the fixed GEM 16-colour palette
(`vdi_backend.rs:26-43`, index 0 = white/background, 1 = black/foreground)
emulated over the 32 bpp VideoCore framebuffer; `vs_color` mutates entries
in the 0..1000-per-gun range (`vdi_backend.rs:142-151`).

### 1080p later

`DEV_TAB[0]/[1]` already derive from the live `RasterBackend` geometry, so
a larger negotiated mode flows through automatically once the framebuffer
reports it; the fixed 640×480 is only in the driver
(`framebuffer.rs:23-25`), not in the VDI table code.
