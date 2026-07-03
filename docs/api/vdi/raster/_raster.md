# VDI raster operations (Phase 00b D6)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_raster.c:1470` (`vdi_vro_cpyfm`),
> `:1492` (`vdi_vrt_cpyfm`), `:269` (`vdi_vr_trnfm`), `:1330`
> (`cpy_raster`, the shared opaque/transparent driver), `:588` (`bit_blt`
> C fallback), `vdi/vdi_blit.S` (`fast_bit_blt`, the m68k fast path).
> MFDB type: `vdi_raster.c:166-176`, public ABI struct `FDB`
> `include/gsxdefs.h:65-76`. Logic-op + `PTSIN`-offset constants:
> `vdi_raster.c:94-124`. `MD_*` write modes: `vdi/vdi_defs.h:77-80`,
> `include/obdefs.h:41-44`. Dispatcher entries: `vdi/vdi_main.c:84-96`.
> Calling convention: [`../_dispatcher.md`](../_dispatcher.md).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract (must match TOS exactly):** the opcodes; the **MFDB layout**
> (`fd_addr`, `fd_w`, `fd_h`, `fd_wdwidth`, `fd_stand`, `fd_nplanes`, three
> reserved WORDs); the MFDB pointer slots in `CONTRL[7..10]`; the source/
> destination rectangle `PTSIN` layout; the **16-entry logic-op table**
> (`INTIN[0]`, esp. 3 = replace, 6 = XOR, 7 = OR) + the `PAT_FLAG` (bit 5)
> and `mode & ~0x1f` invalid rule; the `vrt_cpyfm` `MD_*` codes and its
> fg/bg colours; the standard↔device form transform direction (`vr_trnfm`
> toggles `fd_stand` per the source). **Reference:** the BitBlt pixel
> loops, skew/endmask machinery, the blitter register programming, the
> in-place transform algorithm — NewTOS reimplements the pixel work with
> its own `RasterBackend` (`tos-api/src/vdi/backend.rs`), reproducing the
> observable result.

## Opcodes (`vdi_main.c:84-96`)

| Op | Fn | `# PTSOUT` | `# INTOUT` | Effect |
|----|----|:----------:|:----------:|--------|
| 109 | `vro_cpyfm` | 0 | 0 | copy raster **opaque** (colour→colour, all planes, one of 16 logic ops) |
| 110 | `vr_trnfm`  | 0 | 0 | transform a form between **standard** and **device** layout |
| 121 | `vrt_cpyfm` | 0 | 0 | copy raster **transparent** (1-plane mono → colour, fg/bg expand) |

All three produce **no output array** and have **no error return**;
invalid parameters are a silent no-op. Native ABI (all): GEM trap
`svc #2`, selector 115 in `x0`, `VDIPB*` in `x1`; opcode in `CONTRL[0]`,
handle in `CONTRL[6]`. `v_get_pixel` (105) is in
[`../inquiry/_color-pixel-input.md`](../inquiry/_color-pixel-input.md).

> **Not a raster opcode.** `vsc_form` (111, `vdi_vsc_form`) is **not** a
> mono→colour expand — it transforms a user-defined *mouse cursor* to
> device format (`vdi_mouse.c:513`, `linea_transform_mouse`). There is no
> separate `vsc_expand` opcode in EmuTOS; monochrome→colour expansion is
> the `vrt_cpyfm` (121) transparent path below. `vsc_form` belongs to the
> cursor subsystem ([`../input/_mouse.md`](../input/_mouse.md)) and is out
> of raster scope.

---

## MFDB — memory form definition block (shared raster data structure)

The source and destination forms are described by an **MFDB** (a.k.a.
`FDB`). Its pointers are passed in the control array:
**`CONTRL[7..8]` = source MFDB, `CONTRL[9..10]` = destination MFDB**
(each a 32-bit pointer split across two WORDs on m68k;
`vdi_raster.c:278-279,915-916`). The struct
(`vdi_raster.c:166-176` = `include/gsxdefs.h:65-76`):

### m68k byte layout (the ABI a ported program builds)

| Offset | Size | Field | Meaning |
|:------:|:----:|-------|---------|
| 0 | 4 | `fd_addr` | pixel-data address; **NULL ⇒ the physical screen** |
| 4 | 2 | `fd_w` | form width in **pixels** |
| 6 | 2 | `fd_h` | form height in **pixels** |
| 8 | 2 | `fd_wdwidth` | form width in **words** = `(fd_w + 15) / 16` |
| 10 | 2 | `fd_stand` | `0` = device-specific (interleaved) form, `1` = standard (plane-sequential) form |
| 12 | 2 | `fd_nplanes` | number of bit-planes |
| 14 | 2 | `fd_r1` | reserved (0) |
| 16 | 2 | `fd_r2` | reserved (0) |
| 18 | 2 | `fd_r3` | reserved (0) |

Total 20 bytes on m68k. **Contract:** field order, sizes, and semantics.

- **Plane size in words** = `fd_h × fd_wdwidth` (used by `vr_trnfm`,
  `vdi_raster.c:300`).
- **Screen MFDB:** when `fd_addr == NULL`, the blit uses `v_bas_ad`
  (framebuffer base), `v_planes` planes, and `v_lin_wr` bytes/line
  instead of the MFDB's own geometry (`setup_info`,
  `vdi_raster.c:925-950`). Clipping is applied **only** when the
  *destination* is the screen and `vwk->clip` is set (`:947-949`).
- **`s_nxwd`/`d_nxwd`** (bytes to next word in a line) = `fd_nplanes × 2`
  for an MFDB, `v_planes × 2` for the screen (`:922,937,944`). Equal
  `nxwd` on both sides ⇔ equal plane counts — the `vro_cpyfm` match test.

### Native (AArch64) variant

Per [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md#native-vs-m68k-struct-variants):
NewTOS-recompiled C sees `fd_addr` as a **64-bit** pointer (the eight
WORD fields keep their meaning and 16-bit size). The m68k marshaling shim
(Phase 10) translates the 32-bit-pointer layout above for emulated
binaries. Both expose the same fields.

## Logic operations — `INTIN[0]` (16-entry BitBlt table)

`vro_cpyfm` takes a raster op in `INTIN[0]`. The low 4 bits select one of
16 boolean combinations of source (S) and destination (D)
(`vdi_raster.c:94-110`, applied in `do_blit`/`bit_blt`
`:495-544,746`):

| # | Name | `D' =` | # | Name | `D' =` |
|:-:|------|--------|:-:|------|--------|
| 0 | `ALL_WHITE` | `0` | 8 | `NOT_SORD` | `~(S \| D)` |
| 1 | `S_AND_D` | `S & D` | 9 | `NOT_SXORD` | `~(S ^ D)` |
| 2 | `S_AND_NOTD` | `S & ~D` | 10 | `NOT_D` | `~D` |
| **3** | **`S_ONLY`** | **`S`  (replace)** | 11 | `S_OR_NOTD` | `S \| ~D` |
| 4 | `NOTS_AND_D` | `~S & D` | 12 | `NOT_S` | `~S` |
| 5 | `D_ONLY` | `D` (no-op) | 13 | `NOTS_OR_D` | `~S \| D` |
| **6** | **`S_XOR_D`** | **`S ^ D`  (XOR)** | 14 | `NOT_SANDD` | `~S \| ~D`* |
| **7** | **`S_OR_D`** | **`S \| D`  (OR)** | 15 | `ALL_BLACK` | `1` |

*`BM_NOT_SANDD` is coded as `~S | ~D` in EmuTOS (`vdi_raster.c:539`;
De Morgan of `~(S & D)`). **Bit 5 (`0x10` = `PAT_FLAG`)** additionally
enables pattern blitting (`vdi_raster.c:113,1346-1349`); **any bit above
bit 5 set (`mode & ~0x1f`) is invalid → silent no-op**
(`cpy_raster:1341-1342`).

## Source / destination rectangle — `PTSIN` layout

Both rectangles are `arb_corner`-sorted to `(min,max)` order before use
(`cpy_raster:1336-1337`; `vdi_raster.c:116-124`):

| Slot | Meaning |
|:----:|---------|
| `PTSIN[0]`, `PTSIN[1]` | source rect upper-left `(x1, y1)` |
| `PTSIN[2]`, `PTSIN[3]` | source rect lower-right `(x2, y2)` |
| `PTSIN[4]`, `PTSIN[5]` | destination upper-left `(x1, y1)` |
| `PTSIN[6]`, `PTSIN[7]` | destination lower-right `(x2, y2)` |

The blit copies a source rectangle of size `(x2−x1+1)×(y2−y1+1)` to the
destination upper-left; the destination extent is derived from the source
size, not from `PTSIN[6..7]` directly (`dont_clip`,
`vdi_raster.c:882-900`).

---

## `vro_cpyfm` (109) — copy raster opaque

Source: `vdi_vro_cpyfm` (`vdi_raster.c:1470-1483`) → `cpy_raster`
(`transparent = 0`).

1. `arb_corner` both rectangles; `mode = INTIN[0]`. If `mode & ~0x1f` →
   **no-op** (`:1341-1342`).
2. If bit 5 (`PAT_FLAG`) set, strip it and set up the `Vwk` pattern
   (`:1346-1349`).
3. `setup_info` fills the blit frame from both MFDBs (screen substitution
   for `fd_addr == NULL`); returns "nothing to do" if the plane count is
   invalid (`> 8`, or `> 16` under `CONF_WITH_VDI_16BIT`) or clipping took
   everything (`:908-967`).
4. **Source and destination plane counts must match** (`s_nxwd !=
   d_nxwd` → **no-op**, `:1360-1361`).
5. The single logic op goes into `op_tab[0]` with `fg_col = bg_col = 0`
   (so only `op_tab[0]` is referenced), then all planes are blitted via
   `bit_blt`/`fast_bit_blt` (`:1363-1461`), clipped to the `Vwk` clip
   rect **iff the destination is the screen** and `vwk->clip` is set.

**Inputs:** src/dst MFDB in `CONTRL[7..10]`; `PTSIN[0..7]` (src+dst
rects); `INTIN[0]` = one of the 16 logic ops (`| 0x10` for pattern).
**Output:** the destination form is modified in place; no `INTOUT`/
`PTSOUT`.

## `vrt_cpyfm` (121) — copy raster transparent (mono → colour expand)

Source: `vdi_vrt_cpyfm` (`vdi_raster.c:1492-1505`) → `cpy_raster`
(`transparent = 1`).

Copies a **1-plane (monochrome)** source into a colour destination,
expanding 1-bits and 0-bits to colours according to a write mode:

- `INTIN[0]` = **write mode**: `MD_REPLACE` (**1**), `MD_TRANS` (**2**),
  `MD_XOR` (**3**), `MD_ERASE` (**4**); **any other value → no-op**
  (`cpy_raster:1397-1430`, `default: return`).
- `INTIN[1]` = **foreground** colour index, `INTIN[2]` = **background**
  colour index; both pass through `validate_color_index` (out of
  `[0,numcolors)` → 1) then `MAP_COL[]` to a PEL
  (`vdi_raster.c:1390-1395`).
- The **source MFDB must be one plane** (`s_nxwd != 2` → **no-op**,
  `:1384-1385`); only that one plane is read (`s_nxpl = 0`).

Each mode fills a 4-entry `op_tab` (indexed by the (fg-bit, bg-bit) of the
destination plane) with per-plane logic ops (`vdi_raster.c:1397-1430`):

| `INTIN[0]` mode | Effect of a source 1-bit | Effect of a source 0-bit | `op_tab` (fg/bg) |
|-----------------|--------------------------|--------------------------|------------------|
| `MD_REPLACE` (1) | write **fg** colour | write **bg** colour | `[00,12,03,15]`, both colours |
| `MD_TRANS` (2)   | write **fg** colour | leave destination | `[04,·,07,·]`, fg only |
| `MD_XOR` (3)     | XOR the destination | leave destination | `[06,·,·,·]`, no colour |
| `MD_ERASE` (4)   | leave destination | write **bg** colour | `[01,13,·,·]`, bg only |

> **Colour-argument order.** The variable names in EmuTOS make the intent
> explicit: `fg_col` is built from **`INTIN[1]`** and `bg_col` from
> **`INTIN[2]`** (`vdi_raster.c:1390-1395`) — `INTIN[1]` is the
> foreground (1-bit) colour, `INTIN[2]` the background (0-bit) colour.
> (The inline `/* CHECK: bug, ... colors reversed? */` comment at `:1406`
> is an EmuTOS self-note; the behaviour is preserved as-is and is the
> TOS-compatible contract.)

**Inputs:** src (mono) + dst MFDB in `CONTRL[7..10]`; `PTSIN[0..7]`;
`INTIN[0]` = `MD_*` mode; `INTIN[1]`/`INTIN[2]` = fg/bg colour indices.
**Output:** destination modified in place; no `INTOUT`/`PTSOUT`.

## `vr_trnfm` (110) — transform form (standard ↔ device)

Source: `vdi_vr_trnfm` (`vdi_raster.c:269-356`). Converts a bitmap
between **standard** (plane-sequential, machine-independent) and
**device-dependent** (plane-interleaved) layout.

- MFDBs from `CONTRL[7..8]` (source) / `CONTRL[9..10]` (destination).
- **Direction is driven by the source's `fd_stand`, and the destination's
  `fd_stand` is forced to the opposite** (`vdi_raster.c:303-314`):
  - `src->fd_stand == 1` (standard) → `dst->fd_stand = 0` (device);
    outer loop = planes, inner = plane size.
  - `src->fd_stand == 0` (device) → `dst->fd_stand = 1` (standard);
    outer loop = plane size, inner = planes.
- Plane size = `fd_h × fd_wdwidth` words (`:300`), read from the
  **source** MFDB; `fd_nplanes` also from the source (`:299`).
- **In-place** (`src->fd_addr == dst->fd_addr`) is supported via a slow
  element-rotation path (`:329-355`); for a **1-plane** form the standard
  and device layouts are identical, so an in-place mono transform is a
  **no-op** (`:330-331`).

**Inputs:** source MFDB (its `fd_stand` selects the direction) + dest
MFDB (its `fd_stand` is overwritten). **Output:** the destination pixel
buffer holds the transformed form; `dst->fd_stand` is set to `!src->fd_stand`.
No `INTOUT`/`PTSOUT`.

**Truecolor (`CONF_WITH_VDI_16BIT`, `> 8` planes):** delegates to
`vr_trnfm16` (`:190-256`) — out of the palette bring-up scope.

---

## Verification criteria

- **`vro_cpyfm` replace (op 3):** an S→D copy of a source MFDB rect
  (`PTSIN[0..3]`) to a dest rect origin (`PTSIN[4..5]`) with
  `INTIN[0]=3` reproduces the source pixels in the destination
  (matching plane counts; `s_nxwd == d_nxwd`).
- **`vro_cpyfm` XOR (op 6):** `INTIN[0]=6` XORs source into destination —
  applying it twice restores the original (rubber-band idiom).
- **`vro_cpyfm` plane-mismatch no-op:** source `fd_nplanes=1`, dest
  `fd_nplanes=4` → the call does nothing (`s_nxwd != d_nxwd`).
- **`vro_cpyfm` invalid mode no-op:** `INTIN[0]=0x20` (`& ~0x1f` set) →
  no change.
- **`vrt_cpyfm` replace (mode 1):** a 1-plane source with `INTIN=[1, fg,
  bg]` writes `fg` where the source bit is 1 and `bg` where it is 0.
- **`vrt_cpyfm` transparent (mode 2):** `INTIN[0]=2` writes `fg` on
  1-bits and leaves 0-bit pixels untouched.
- **`vrt_cpyfm` non-mono source no-op:** a source MFDB with
  `fd_nplanes != 1` → no change (`s_nxwd != 2`).
- **`vr_trnfm` toggles `fd_stand`:** given `src->fd_stand=1`, after the
  call `dst->fd_stand == 0` and the pixel buffer is the interleaved
  (device) form; given `src->fd_stand=0`, `dst->fd_stand == 1`.
- **`vr_trnfm` in-place mono:** `src==dst`, `fd_nplanes=1` → buffer
  unchanged (no-op).
- **Screen MFDB:** an MFDB with `fd_addr == NULL` targets the physical
  framebuffer (`v_bas_ad`/`v_planes`/`v_lin_wr`), and clipping applies
  only when it is the *destination*.

## Edge cases discovered in source

1. **`MD_*` codes: `MD_REPLACE = 1`, `MD_TRANS = 2`, `MD_XOR = 3`,
   `MD_ERASE = 4`** (`vdi_defs.h:77-80`). (An earlier revision of this
   spec had `MD_TRANS=1`/`MD_REPLACE=2` — **wrong**; corrected.)
2. **`vro_cpyfm` requires equal plane counts** (`s_nxwd == d_nxwd`) — an
   opaque copy between forms of different depth is a silent no-op
   (`vdi_raster.c:1360-1361`), *not* an error and *not* a depth
   conversion.
3. **`vrt_cpyfm` source must be exactly 1 plane** (`s_nxwd == 2`);
   otherwise no-op (`:1384-1385`). It is the only mono→colour expander.
4. **Only bits 0-5 of `INTIN[0]` are meaningful** for the copy ops; bit 5
   is `PAT_FLAG`, and `mode & ~0x1f` is invalid (`:1341-1349`).
5. **Overlapping src/dst is handled by direction choice.** `bit_blt`
   picks a right-to-left / bottom-to-top scan when `s_addr < d_addr`
   (or equal with non-negative skew), so an overlapping opaque copy does
   not corrupt itself (`vdi_raster.c:667-704`). NewTOS must reproduce the
   overlap-safe result.
6. **`vr_trnfm` direction comes from the *source* `fd_stand`**, and it
   **overwrites** `dst->fd_stand` — a caller cannot force a direction via
   the destination flag (`:303-314`). `fd_nplanes`/plane size are read
   from the source MFDB only.
7. **In-place `vr_trnfm` for mono is a no-op**; for multi-plane it uses a
   slow rotation that Atari TOS also has (`:329-355`).
8. **Clipping is destination-screen-only.** A blit into an off-screen
   MFDB is never clipped to the `Vwk` clip rect, even with clipping
   enabled (`setup_info:947-949`).

## Dependencies

### Requires
- An open workstation (dispatcher resolves `CONTRL[6]`); the clip
  rectangle + `clip` flag (for screen destinations); `MAP_COL[]` +
  `validate_color_index` + `numcolors` (for `vrt_cpyfm` colours);
  `v_bas_ad`/`v_planes`/`v_lin_wr` for screen forms.
- `CONF_WITH_VDI_16BIT` (machine-dependent, requires `CONF_WITH_VIDEL`)
  for the truecolor `vro_cpyfm16`/`vrt_cpyfm16`/`vr_trnfm16` paths.

### Used by
- AES icon/image drawing, `objc_draw` blits, screen save/restore, and any
  program moving bitmaps (`vro_cpyfm` for opaque, `vrt_cpyfm` for
  transparent/monochrome sprites, `vr_trnfm` to unpack resource images
  authored in standard form).

## Related data structures

- **MFDB** — laid out above; the fixed ABI form parsed by ported
  programs. Also referenced by
  [`../workstation/102-vq_extnd.md`](../workstation/102-vq_extnd.md)
  (`INQ_TAB[30]` = "does `vro_cpyfm` scale?", 0 on the screen).
- [`../_state.md`](../_state.md) — the `Vwk` clip rect + `clip` flag and
  `multifill` these read; the `MAP_COL[]` pen→PEL map.
- Logic-op constants `BM_*` (`vdi_raster.c:94-110`), `PAT_FLAG`
  (`:113`); `MD_*` (`vdi_defs.h:77-80`).

## Cross-references

### Atari Compendium / Profibuch
- Not present in `references/` (no PDFs). Cross-check of the MFDB byte
  layout, the 16-entry logic-op numbering, the `MD_*` code values (esp.
  the `MD_REPLACE=1`/`MD_TRANS=2` correction), and the `vrt_cpyfm`
  fg/bg (`INTIN[1]`/`INTIN[2]`) order against the documented ABI is
  **deferred**.

## Configuration / version notes

- `CONF_WITH_VDI_16BIT` (needs `CONF_WITH_VIDEL`; 1 on Falcon/FireBee, 0
  on ST-class) adds the `*16` truecolor variants and raises the valid
  plane cap from 8 to 16. NewTOS bring-up is palette (≤ 8 planes); the
  truecolor paths are deferred.
- The fast m68k blit (`fast_bit_blt`, `vdi_blit.S`) vs the C `bit_blt`
  vs the hardware blitter (`CONF_WITH_BLITTER`) are all **reference**
  implementations of the same op table — NewTOS uses its own backend.

## NewTOS implementation notes (Phase 12)

- `bit_blt` maps to the `RasterBackend` block-copy primitive
  (`tos-api/src/vdi/backend.rs`). Implement the **full 16-entry logic-op
  table** (not just replace) — programs rely on XOR (op 6) for
  rubber-banding and OR (op 7) for overlays. Target/source is the MFDB
  buffer, or `vwk->device` when `fd_addr == NULL` (ADR-011).
- Keep `vro_cpyfm`'s equal-plane requirement, `vrt_cpyfm`'s mono-source
  requirement, and the overlap-safe copy direction as observable
  contract.
- `vr_trnfm` matters for unpacking icon/image data authored in standard
  form; keep both directions and the source-driven `fd_stand` toggle.
- The MFDB is a **fixed ABI** parsed by ported programs — match the field
  order/size exactly (unlike the internal `Vwk`); honour the native
  64-bit `fd_addr` variant vs the m68k 32-bit one via the header set /
  Phase-10 shim.
- Truecolor paths (`CONF_WITH_VDI_16BIT`) are out of bring-up scope
  (8-bit palette screen first).

---

## Pi / BCM2711 binding

> Re-points the *hardware reference* for the **screen MFDB** (`v_bas_ad` /
> `v_planes` / `v_lin_wr` Shifter screen vars) at the BCM2711 VideoCore
> framebuffer. The **contract is unchanged** — the MFDB layout, the
> `fd_addr == NULL ⇒ screen` rule, the 16-op logic table, the `MD_*` codes.
> Peripheral + driver: see `docs/HARDWARE-PROFILE.md` §2/§3.1.

### The screen MFDB = the VideoCore framebuffer

When a program passes `fd_addr == 0` (NULL MFDB), the source/destination is
the physical screen (`tos-api/src/vdi/raster.rs:39-42,59-61`). NewTOS
substitutes the live framebuffer rather than `v_bas_ad`/`v_planes`/
`v_lin_wr`: the `RasterBackend` reads/writes the VideoCore buffer directly
(`kernel/src/vdi_backend.rs` `copy_rect`/`blit_mono`), so raster ops never
dereference the MFDB geometry for screen forms — they use the backend's
`width()`/`height()`/`stride_px()`. The equivalent screen-MFDB values for
the current mode:

| MFDB field | Atari screen var | NewTOS 640×480×16 value | Source |
|-----------|------------------|-------------------------|--------|
| `fd_addr` (screen) | `v_bas_ad` | the VideoCore fb base (CPU high-half VA) | `vdi_backend.rs:405-407`; `host_env.rs:218-229` |
| `fd_w` | `V_REZ_HZ` | **640** | `framebuffer.rs:23` |
| `fd_h` | `V_REZ_VT` | **480** | `framebuffer.rs:24` |
| `fd_wdwidth` | — | **40** = `(640+15)/16` | words/row for the 640-px logical raster |
| `fd_nplanes` | `v_planes` | **4** (16-colour VDI model) | `vdi_backend.rs:21`, `NUM_COLORS=16` |
| `fd_stand` | — | 0 (device) for the screen | — |

### Important: the logical raster is 4-plane, the store is 32 bpp

The MFDB `nplanes = 4` / `wdwidth = 40` describe the **VDI logical raster**
(16 colours = 4 planes), *not* the physical pixel store. The physical
VideoCore buffer is **32 bpp XRGB8888** with pitch 640 × 4 = 2560 B/line
(`framebuffer.rs:23-25`; VC-reported pitch at `framebuffer.rs:53,192`). The
`RasterBackend` bridges the two: PEL indices from the 4-plane model are
expanded through the 16-entry CLUT to packed XRGB when written to the 32 bpp
store (`vdi_backend.rs:107-110`, `pel_to_rgb`). So a program that computes a
screen-MFDB plane offset from `nplanes/wdwidth` gets the *logical* raster
geometry; NewTOS does not expose the raw 32 bpp bytes as planes.

- **Screen-to-screen ops use the backend.** `vro_cpyfm` screen→screen maps
  to `RasterBackend::copy_rect` (replace op 3 / XOR op 6), overlap-safe
  (`vdi_backend.rs:345-387`); the cache clean to PoC
  (`vdi_backend.rs:118-125`) makes the copy visible to the VideoCore
  display DMA on real hardware.
- **Mono→colour `vrt_cpyfm`** expands a 1-plane source to fg/bg PEL via
  `blit_mono` (`vdi_backend.rs:311-343`); the mono-source (`s_nxwd==2`)
  requirement is a contract kept above the backend.
- **Native 64-bit MFDB pointer.** NewTOS carries the source MFDB in
  `CONTRL[7..10]` and the destination in `CONTRL[11..14]` (four WORDs each)
  because AArch64 pointers are 64-bit (`raster.rs:4-9`) — a documented
  NewTOS-native ABI extension of the 32-bit m68k `CONTRL[7..8]`/`[9..10]`
  layout; the field *meanings* are unchanged.

### Mismatch note (Step-2)

- **Off-screen (memory-MFDB) copies are bring-up-limited.** The current
  backend implements the screen-form cases (screen↔screen copy, mono→colour
  expand into the screen); full off-screen colour MFDB↔MFDB copies and the
  complete 16-op path across arbitrary buffers are noted as later work in
  the module docs. Flagged so Step-2 can check coverage against the 16-op
  contract; no code change in this pass.

### 1080p later

`fd_w`/`fd_h`/`fd_wdwidth` for the screen MFDB derive from the live
framebuffer geometry via the `RasterBackend`; a larger negotiated mode
flows through without changing the raster-op code. `nplanes = 4` is tied to
the 16-colour CLUT model, independent of resolution.
