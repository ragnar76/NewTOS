# VDI dispatcher & calling convention (Phase 00b D1)

> Source: EmuTOS @ `1a7484ab`. Verified against:
> `vdi/vdi_asm.S:34` (`_vditrap`), `:64` (`_GSX_ENTRY`),
> `vdi/vdi_main.c:121` (`screen()` + the two jump tables),
> `aes/gsx2.c:20` (`gsx2()` binding), `aes/gsx2.h:17` (`VDIPB`),
> `vdi/vdi_defs.h:36-39,108-115` (opcode + `CONTRL[]` index `#define`s),
> `include/config.h:391,1660` (`NUM_VDI_HANDLES`), `:388,1654`
> (`MAX_VERTICES`).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> **Contract** (must match TOS exactly): the trap selector, the five-array
> `VDIPB` parameter block, the `CONTRL[]` index meanings, the opcode→function
> map, the output-count pre-load, the `PTSIN`/`MAX_VERTICES` clamp, and the
> `d0` query/return values. **Reference** (NewTOS reimplements freely): the
> `jmptb1`/`jmptb2` table storage, `_vdishare` local `PTSIN` buffer, and the
> Line-A mirror writes at the end of `screen()`.

The VDI (Virtual Device Interface) is **not** register-per-argument like
GEMDOS/BIOS/XBIOS. It is invoked through a single **parameter-block** trap
and selects functions by an **opcode**, with all inputs and outputs passed
in five WORD arrays.

---

## Trap interface (m68k)

| | |
|--|--|
| Trap | `trap #2` |
| `d0` selector | `0x73` = VDI call (`vdi_asm.S:35`; set by `gsx2.c:27`) |
| `d1` | pointer to the `VDIPB` (`vdi_asm.S:74`; set by `gsx2.c:26`) |
| Return `d0` | `flip_y` (0/1) — "coordinate vs magnitude" flag (`vdi_asm.S:124-125`) |

`trap #2` is shared with the AES: the AES hooks the trap-2 vector and
recognises its own selector; `_vditrap` handles only `d0 == 0x73`
(VDI), the `d0 == -1` query, and passes everything else through
(`vdi_asm.S:34-52`). AES uses `d0 == 0xC8`; that is documented in the
AES dispatcher, not here.

### `d0` special values (`vdi_asm.S:40-52`)

| `d0` on entry | Effect |
|---------------|--------|
| `0x73` | VDI call — dispatch via `_GSX_ENTRY` → `screen()` |
| `-1` (`0xffff`) | query: return the address of `_GSX_ENTRY` in `d0` (the "jsr-able" VDI dispatcher address) and `rte` — **no** parameter block consulted |
| `-2` | GDOS-absence probe: `d0` left unchanged, `rte` (signals "no GDOS", exactly like Atari TOS) |
| anything else | `rte`, no action |

### Native (AArch64) call shape

NewTOS delivers the VDI call as the GEM trap `svc #2`, selector **115**
in `x0`, and the `VDIPB` pointer in `x1`. (This selector value is a
NewTOS ABI choice recorded in the task charter / a TBD-ADR; it plays the
role the m68k `d0 = 0x73` selector plays on trap #2.) The five array
pointers and every `CONTRL[]`/`INTIN[]`/… index meaning below are the
**contract** and are bit-identical across both worlds — a ported VDI
binding marshals the same `VDIPB`.

> Open question (charter): confirm selector 115 vs. the 200 mentioned in
> `COMPATIBILITY.md` ("GEM `svc #2` sel 200") — the two docs disagree on
> the native GEM-trap selector number. This is a NewTOS-side ABI number,
> not an EmuTOS fact; resolve before Phase-12 wiring. Recorded in
> [`_discrepancies.md`](_discrepancies.md) D-VDI-6.

---

## The VDI parameter block (`VDIPB`)

`aes/gsx2.h:17-24`: five longword pointers, in this fixed order.

| Offset | Field | Points at | Role |
|--------|-------|-----------|------|
| +0 | `contrl` | `CONTRL[]` | control: opcode, counts, sub-opcode, handle |
| +4 | `intin`  | `INTIN[]`  | integer inputs (colours, modes, char codes, …) |
| +8 | `ptsin`  | `PTSIN[]`  | coordinate inputs — `(x,y)` vertex pairs |
| +12 | `intout` | `INTOUT[]` | integer outputs (inquiry results) |
| +16 | `ptsout` | `PTSOUT[]` | coordinate outputs |

`_GSX_ENTRY` copies the five pointers from the caller's `VDIPB` into the
driver's private `_local_pb` (`vdi_asm.S:74-83`). `CONTRL`/`INTIN`/
`INTOUT`/`PTSOUT` are used **in place** (the caller's arrays); only
`PTSIN` is copied into a driver-local buffer (`_vdishare`) — see the
clamp below. This local-copy mechanism is a **reference** detail; the
observable contract is that the handler reads the caller's `INTIN`/
`PTSIN` and writes the caller's `INTOUT`/`PTSOUT`.

### `CONTRL[]` layout (`vdi_defs.h:108-115`)

Named indices `ROUTINE/N_PTSIN/N_PTSOUT/N_INTIN/N_INTOUT/SUBROUTINE/VDI_HANDLE`:

| Index | Name | Meaning |
|-------|------|---------|
| `CONTRL[0]` | `ROUTINE` | **opcode** (function selector) |
| `CONTRL[1]` | `N_PTSIN` | **# input vertices** in `PTSIN` (count of `(x,y)` *pairs*) |
| `CONTRL[2]` | `N_PTSOUT` | # output vertices in `PTSOUT` — **set by the dispatcher** from the jump-table `nptsout`; a handler may overwrite it (e.g. escape) |
| `CONTRL[3]` | `N_INTIN` | **# input integers** in `INTIN` |
| `CONTRL[4]` | `N_INTOUT` | # output integers in `INTOUT` — **set by the dispatcher** from `nintout`; a handler may overwrite it |
| `CONTRL[5]` | `SUBROUTINE` | **sub-opcode** (GDP id for opcode 11; escape id for opcode 5) |
| `CONTRL[6]` | `VDI_HANDLE` | **device handle** (physical or virtual workstation) |

The AES-side `contrl[]` is declared `WORD contrl[12]` (`include/gsxdefs.h:100`)
and `intin[128]`, `ptsin[20]`; only indices `CONTRL[0..6]` carry
fixed-meaning VDI control fields. Indices `CONTRL[7..]` are used by AES
bindings to smuggle 32-bit pointers (`gsx2.h:32-34` `i_ptr`), which is an
AES-binding convention, not part of the screen-driver contract.

### `PTSIN` vertex clamp — observable ABI (`vdi_asm.S:89-108`)

Before calling `screen()`, `_GSX_ENTRY`:

1. reads the vertex count from `CONTRL[1]`;
2. if it exceeds `MAX_VERTICES`, **clamps `CONTRL[1]` to `MAX_VERTICES`
   and overwrites the caller's `CONTRL[1]`** for the duration of the call;
3. copies exactly that many `(x,y)` pairs into the local `PTSIN` buffer;
4. after `screen()` returns, **restores the caller's original `CONTRL[1]`**.

So a program passing more than `MAX_VERTICES` vertices has the excess
silently dropped; `CONTRL[1]` is transiently clamped during the call and
restored on exit. `MAX_VERTICES` is **512** on the small/STe-class target
profile and **1024** by default (`config.h:388-390,1653-1655`) — a
build-config value, see the open question below.

---

## Dispatch (`screen()`, `vdi_main.c:121-192`)

Behaviour in prose (not the C body):

1. Read the handle from `CONTRL[6]`; zero the default output counts
   `CONTRL[2] = CONTRL[4] = 0`; clear `flip_y = 0`; read `opcode = CONTRL[0]`.
2. **Handle resolution.** For every opcode *except* `v_opnwk` (1) and
   `v_opnvwk` (100), resolve `CONTRL[6]` → `Vwk *` via
   `get_vwk_by_handle()` (`vdi_control.c:178`). If the handle is out of
   range or its slot is empty, **`screen()` returns immediately doing
   nothing** — this is the closest thing VDI has to an error (a silent
   no-op, not an error code). `v_opnwk`/`v_opnvwk` are called with
   `vwk == NULL` and set `CUR_WORK` themselves.
3. **Multifill reset.** For a resolved `vwk`, if `fill_style != 4`
   (user pattern), `vwk->multifill` is cleared (`vdi_main.c:146-147`) —
   a Line-A/assembler-support detail (**reference**).
4. **Table select.** Two contiguous opcode ranges map to two jump tables:
   opcodes `[1 .. 1+JMPTB1_ENTRIES)` → `jmptb1[opcode-1]`; opcodes
   `[100 .. 100+JMPTB2_ENTRIES)` → `jmptb2[opcode-100]`. Any opcode
   outside both ranges → **return, no-op** (`vdi_main.c:155-156`).
5. **Output-count pre-load.** `CONTRL[2] = jmptab->nptsout`,
   `CONTRL[4] = jmptab->nintout` (`vdi_main.c:158-159`). Inquiry handlers
   that return a variable count overwrite these (e.g. escape sets
   `CONTRL[4]` itself; `vswr_mode` sets `CONTRL[4]=1`).
6. **Call** `jmptab->op(vwk)` (`vdi_main.c:160`).
7. For `v_opnwk`/`v_opnvwk`, `vwk` was NULL; the handler set `CUR_WORK`,
   so `screen()` recovers `vwk = CUR_WORK` (`vdi_main.c:168-171`).
8. **Line-A mirror** (`vdi_main.c:177-191`, **reference**): for every
   opcode except `v_clswk` (2) and `v_clsvwk` (101), the current
   workstation's font/write-mode/clip fields are copied into the Line-A
   globals (`CUR_FONT`, `WRT_MODE`, `CLIP`, `XMINCL`…`YMAXCL`,
   `CUR_WORK`). EmuTOS notes these assignments are not required by its own
   code but mirror Atari TOS so Line-A callers see consistent state.

Unknown / unimplemented opcodes resolve to `vdi_v_nop`
(`vdi_main.c:23` — an alias for `just_rts`), present for ABI
completeness but doing nothing. **VDI has no error return**; failures are
silent no-ops or are reported through the inquiry calls.

---

## Opcode map

`{nptsout, nintout}` are the output counts the dispatcher pre-loads into
`CONTRL[2]`/`CONTRL[4]`. `(nop)` = `vdi_v_nop` (present for ABI
completeness, does nothing).

### Jump table 1 — opcodes 1..39 (`jmptb1[]`, `vdi_main.c:32-72`)

| Op | Fn | `nptsout` / `nintout` | Group |
|----|----|:---------------------:|-------|
| 1  | `v_opnwk`         | 6 / 45 | workstation (physical open) |
| 2  | `v_clswk`         | 0 / 0  | workstation (physical close) |
| 3  | `v_clrwk`         | 0 / 0  | workstation (clear) |
| 4  | `v_updwk` **(nop)** | 0 / 0 | — (not implemented on the screen driver) |
| 5  | `v_escape`        | 0 / 0† | escape (`CONTRL[5]` selects; see [`_escape.md`](_escape.md)) |
| 6  | `v_pline`         | 0 / 0  | output (polyline) |
| 7  | `v_pmarker`       | 0 / 0  | output (markers) |
| 8  | `v_gtext`         | 0 / 0  | output (graphic text) |
| 9  | `v_fillarea`      | 0 / 0  | output (filled polygon) |
| 10 | `v_cellarray` **(nop)** | 0 / 0 | — (not usually implemented by drivers) |
| 11 | `v_gdp`           | 0 / 0† | output (GDP bar/arc/circle/…, `CONTRL[5]`) |
| 12 | `vst_height`      | 2 / 0  | attr (text height, absolute) |
| 13 | `vst_rotation`    | 0 / 1  | attr (text baseline angle) |
| 14 | `vs_color`        | 0 / 0  | attr (set palette entry RGB) |
| 15 | `vsl_type`        | 0 / 1  | attr (line style) |
| 16 | `vsl_width`       | 1 / 0  | attr (line width) |
| 17 | `vsl_color`       | 0 / 1  | attr (line colour) |
| 18 | `vsm_type`        | 0 / 1  | attr (marker type) |
| 19 | `vsm_height`      | 1 / 0  | attr (marker height) |
| 20 | `vsm_color`       | 0 / 1  | attr (marker colour) |
| 21 | `vst_font`        | 0 / 1  | attr (text face) |
| 22 | `vst_color`       | 0 / 1  | attr (text colour) |
| 23 | `vsf_interior`    | 0 / 1  | attr (fill interior style) |
| 24 | `vsf_style`       | 0 / 1  | attr (fill pattern index) |
| 25 | `vsf_color`       | 0 / 1  | attr (fill colour) |
| 26 | `vq_color`        | 0 / 4  | inquiry (palette entry) |
| 27 | `vq_cellarray` **(nop)** | 0 / 0 | — |
| 28 | `v_locator`       | 0 / 0  | input (mouse position) |
| 29 | `v_valuator` **(nop)** | 0 / 0 | — (not implemented by drivers) |
| 30 | `v_choice`        | 0 / 1  | input (choice/key) |
| 31 | `v_string`        | 0 / 0  | input (string) |
| 32 | `vswr_mode`       | 0 / 1  | attr (writing/raster mode) |
| 33 | `vsin_mode`       | 0 / 1  | input mode (request/sample) |
| 34 | — **(nop)**       | 0 / 0  | — (does not exist) |
| 35 | `vql_attributes`  | 1 / 3  | inquiry (line attrs) |
| 36 | `vqm_attributes`  | 1 / 3  | inquiry (marker attrs) |
| 37 | `vqf_attributes`  | 0 / 5  | inquiry (fill attrs) |
| 38 | `vqt_attributes`  | 2 / 6  | inquiry (text attrs) |
| 39 | `vst_alignment`   | 0 / 2  | attr (text alignment) |

† Opcodes 5 (`v_escape`) and 11 (`v_gdp`) carry `{0,0}` in the jump
table; the sub-function handler sets the real output counts itself.

### Jump table 2 — opcodes 100..131/134 (`jmptb2[]`, `vdi_main.c:74-112`)

| Op | Fn | `nptsout` / `nintout` | Group |
|----|----|:---------------------:|-------|
| 100 | `v_opnvwk`        | 6 / 45 | workstation (virtual open) |
| 101 | `v_clsvwk`        | 0 / 0  | workstation (virtual close) |
| 102 | `vq_extnd`        | 6 / 45 | inquiry (extended capabilities) |
| 103 | `v_contourfill`   | 0 / 0  | output (seed fill) |
| 104 | `vsf_perimeter`   | 0 / 1  | attr (fill outline on/off) |
| 105 | `v_get_pixel`     | 0 / 2  | inquiry (read pixel) |
| 106 | `vst_effects`     | 0 / 1  | attr (text effects: bold/italic/…) |
| 107 | `vst_point`       | 2 / 1  | attr (text height in points) |
| 108 | `vsl_ends`        | 0 / 2  | attr (line end styles) |
| 109 | `vro_cpyfm`       | 0 / 0  | raster (opaque blit) |
| 110 | `vr_trnfm`        | 0 / 0  | raster (format transform) |
| 111 | `vsc_form`        | 0 / 0  | mouse (set cursor form) |
| 112 | `vsf_udpat`       | 0 / 0  | attr (user fill pattern) |
| 113 | `vsl_udsty`       | 0 / 0  | attr (user line style) |
| 114 | `vr_recfl`        | 0 / 0  | output (fill rectangle) |
| 115 | `vqin_mode`       | 0 / 1  | inquiry (input mode) |
| 116 | `vqt_extent`      | 4 / 0  | inquiry (text bounding box) |
| 117 | `vqt_width`       | 3 / 1  | inquiry (char cell width) |
| 118 | `vex_timv`        | 0 / 1  | exchange timer vector |
| 119 | `vst_load_fonts`  | 0 / 1  | text (load extra fonts) |
| 120 | `vst_unload_fonts`| 0 / 0  | text (unload fonts) |
| 121 | `vrt_cpyfm`       | 0 / 0  | raster (transparent blit) |
| 122 | `v_show_c`        | 0 / 0  | mouse (show cursor) |
| 123 | `v_hide_c`        | 0 / 0  | mouse (hide cursor) |
| 124 | `vq_mouse`        | 1 / 1  | mouse (query position/buttons) |
| 125 | `vex_butv`        | 0 / 0  | exchange button vector |
| 126 | `vex_motv`        | 0 / 0  | exchange motion vector |
| 127 | `vex_curv`        | 0 / 0  | exchange cursor vector |
| 128 | `vq_key_s`        | 0 / 1  | inquiry (shift-key state) |
| 129 | `vs_clip`         | 0 / 0  | attr (clipping rectangle) |
| 130 | `vqt_name`        | 0 / 33 | inquiry (font name) |
| 131 | `vqt_fontinfo`    | 5 / 2  | inquiry (font metrics) |
| 132 | `vqt_justified` **(nop)** | 0 / 0 | (PC-GEM) — **only present `#if CONF_WITH_EXTENDED_MOUSE`** |
| 133 | `vs_grayoverride` **(nop)** | 0 / 0 | (PC-GEM/3) — **only present `#if CONF_WITH_EXTENDED_MOUSE`** |
| 134 | `vex_wheelv`      | 0 / 0  | exchange mouse-wheel vector (Milan) — **only present `#if CONF_WITH_EXTENDED_MOUSE`** |

**Build-config gate (`vdi_main.c:107-111`):** opcodes **132, 133, 134
only exist when `CONF_WITH_EXTENDED_MOUSE` is defined.** Without it,
`JMPTB2_ENTRIES` ends at opcode 131, and 132–134 fall through to the
"outside both ranges → no-op" path in `screen()`. (Older revisions of
this doc listed 132–134 unconditionally — corrected.)

---

## The workstation `Vwk`

`v_opnwk` (physical) / `v_opnvwk` (virtual) allocate/initialise a `Vwk`
holding the *attribute state* a handle draws with — line/marker/fill/text
attributes, writing mode, the clipping rectangle, and the `next_work`
chain link. `CUR_WORK` points at the active one; the dispatcher resolves
`CONTRL[6]` to it. Struct layout and per-field semantics are documented
in [`../structures/vwk.md`](../structures/vwk.md); the workstation
lifecycle is in [`workstation/`](workstation/); the shared workstation
state is summarised in [`_state.md`](_state.md).

**Handle count.** `NUM_VDI_HANDLES` handles exist, numbered
`FIRST_VDI_HANDLE = 1` (the physical workstation) through
`LAST_VDI_HANDLE = NUM_VDI_HANDLES` (`vdi_control.c:25-27`).
`NUM_VDI_HANDLES` is a **build-config** value: **64** on the small/STe
target, **128** by default (`config.h:391-393,1660-1662`) — **not 16**.
(An earlier revision of the workstation docs said "16" — corrected here
and in `_discrepancies.md` D-VDI-3.)

---

## NewTOS implementation notes (Phase 12)

- **Calling convention.** Native AArch64 source ports call VDI through the
  AES/VDI binding (`vdi(VDIPB*)`), which marshals the five array pointers.
  NewTOS keeps the `VDIPB`/`CONTRL`/`INTIN`/… contract bit-for-bit so
  ported bindings work unchanged; the trap is delivered as `svc #2`
  selector 115 with the `VDIPB` pointer in `x1`. The opcode/array
  semantics above are the ABI surface, not the trap mechanics.
- **Device.** The single screen device draws on the Phase-10 framebuffer
  (`hal::framebuffer`) via the NewTOS `RasterBackend`
  (`tos-api/src/vdi/backend.rs`) — a hardware-independent reimplementation
  of the pixel loops, **not** an EmuTOS Line-A port. Only one physical
  workstation (the screen) in the bring-up profile (no printer/plotter/
  metafile).
- **Coordinate system.** The screen workstation is raster-coordinate (RC),
  origin top-left — matches the framebuffer directly. `work_in[10]`
  (`xfm_mode`) NDC scaling is a later refinement (see
  [`workstation/100-v_opnvwk.md`](workstation/100-v_opnvwk.md)).
- **`MAX_VERTICES` / `NUM_VDI_HANDLES`.** Pick the target profile
  explicitly (open question, `_discrepancies.md` D-VDI-3/D-VDI-7); the
  `PTSIN` clamp and the handle-range checks must use the chosen values.
- **Per-function specs** land in `docs/api/vdi/<group>/*.md`; tracking in
  [`_working-list.md`](_working-list.md).
