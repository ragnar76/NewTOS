# VDI dispatcher & calling convention (Phase 00b D1)

> Source: EmuTOS @ `1a7484ab` — `vdi/vdi_main.c` (`screen()` /
> dispatch, jump tables), `vdi/vdi_defs.h` (`Vwk`, opcode constants).
> Cross-refs: Compendium "VDI", Profibuch "VDI". Line-A side channel:
> `docs/api/_line-a.md`.

The VDI (Virtual Device Interface) is **not** register-based like
GEMDOS/BIOS/XBIOS. It is invoked through a single **parameter-block**
trap and selects functions by an **opcode**, with all inputs/outputs
passed in five arrays.

## Trap interface

| | |
|--|--|
| Trap (m68k) | `trap #2`, with `D0 = 0x73` (GEMDIS) — the AES/VDI shared entry; `D1` = pointer to the VDI parameter block (`VDIPB`) when the call is a VDI call (`D0` selects VDI vs AES). |
| Selector | `contrl[0]` = opcode (1..39 in jump-table 1, 100..134 in jump-table 2); `contrl[5]` = sub-opcode for `v_gdp` (11) and `v_escape` (5). |
| Workstation | `contrl[6]` = device handle (the virtual-workstation handle returned by `v_opnvwk`). |

### The VDI parameter block (`VDIPB`)

Five `WORD` arrays, addressed by the five longwords the caller passes
(classically a `WORD *contrl[5]` block; bindings hand the addresses of
the arrays):

| Array | Role |
|-------|------|
| `CONTRL` | control: opcode, counts, sub-opcode, handle (see below) |
| `INTIN`  | integer inputs (colours, modes, character codes, …) |
| `PTSIN`  | coordinate inputs — `(x,y)` vertex pairs |
| `INTOUT` | integer outputs (inquiry results) |
| `PTSOUT` | coordinate outputs |

`CONTRL` layout (the only fixed-meaning entries):

| Index | Meaning |
|-------|---------|
| `contrl[0]` | **opcode** (function selector) |
| `contrl[1]` | **# of input vertices** in `PTSIN` (the count *of pairs*) |
| `contrl[2]` | # of output vertices in `PTSOUT` — **set by the dispatcher** from the jump-table `nptsout`, may be refined by the handler |
| `contrl[3]` | **# of input integers** in `INTIN` |
| `contrl[4]` | # of output integers in `INTOUT` — **set by the dispatcher** from `nintout` |
| `contrl[5]` | **sub-opcode** (GDP id for opcode 11; escape id for opcode 5) |
| `contrl[6]` | **device handle** (virtual workstation) |

## Dispatch (`vdi_main.c:screen()`)

```c
opcode = contrl[0];
contrl[2] = 0; contrl[4] = 0;           /* default output counts */

/* All ops except open need a valid, currently-open workstation. */
if (opcode != V_OPNWK_OP && opcode != V_OPNVWK_OP) { /* resolve handle → vwk */ }

/* Two contiguous opcode ranges, two jump tables. */
if (opcode in [1, 1+JMPTB1_ENTRIES))    jmptab = &jmptb1[opcode - 1];
else if (opcode in [100, 100+JMPTB2_ENTRIES)) jmptab = &jmptb2[opcode - 100];
else return;                            /* unknown opcode → no-op */

contrl[2] = jmptab->nptsout;            /* pre-load output counts */
contrl[4] = jmptab->nintout;

if (opcode == V_OPNWK_OP || opcode == V_OPNVWK_OP) /* set vwk from a fresh slot */;
jmptab->op(vwk);                        /* call the handler */
```

The handler reads `INTIN`/`PTSIN` (counts in `contrl[3]`/`contrl[1]`)
and writes `INTOUT`/`PTSOUT`. Unknown / unimplemented opcodes resolve
to `vdi_v_nop` (a no-op `rts`), **not** an error — VDI has no error
return; failures are silent or reported through the inquiry calls.

## Opcode map

`{nptsout, nintout}` are the output counts the dispatcher pre-loads.
`(nop)` = `vdi_v_nop`, present for ABI completeness but does nothing.

### Jump table 1 — opcodes 1..39 (`V_OPNWK_OP = 1`)

| Op | Fn | n_ptsout / n_intout | Group |
|----|----|---------------------|-------|
| 1  | `v_opnwk`        | 6 / 45 | workstation (physical) |
| 2  | `v_clswk`        | 0 / 0  | workstation |
| 3  | `v_clrwk`        | 0 / 0  | workstation (clear) |
| 4  | `v_updwk` (nop)  | 0 / 0  | — |
| 5  | `v_escape`       | sub-op | escape (`contrl[5]`) |
| 6  | `v_pline`        | 0 / 0  | output (polyline) |
| 7  | `v_pmarker`      | 0 / 0  | output (markers) |
| 8  | `v_gtext`        | 0 / 0  | output (graphic text) |
| 9  | `v_fillarea`     | 0 / 0  | output (filled polygon) |
| 10 | `v_cellarray`(nop)| 0 / 0 | — |
| 11 | `v_gdp`          | sub-op | output (GDP: bar/arc/circle/…, `contrl[5]`) |
| 12 | `vst_height`     | 2 / 0  | attr (text height, abs) |
| 13 | `vst_rotation`   | 0 / 1  | attr (text baseline angle) |
| 14 | `vs_color`       | 0 / 0  | attr (set palette entry RGB) |
| 15 | `vsl_type`       | 0 / 1  | attr (line style) |
| 16 | `vsl_width`      | 1 / 0  | attr (line width) |
| 17 | `vsl_color`      | 0 / 1  | attr (line colour) |
| 18 | `vsm_type`       | 0 / 1  | attr (marker type) |
| 19 | `vsm_height`     | 1 / 0  | attr (marker height) |
| 20 | `vsm_color`      | 0 / 1  | attr (marker colour) |
| 21 | `vst_font`       | 0 / 1  | attr (text face) |
| 22 | `vst_color`      | 0 / 1  | attr (text colour) |
| 23 | `vsf_interior`   | 0 / 1  | attr (fill interior style) |
| 24 | `vsf_style`      | 0 / 1  | attr (fill pattern index) |
| 25 | `vsf_color`      | 0 / 1  | attr (fill colour) |
| 26 | `vq_color`       | 0 / 4  | inquiry (palette entry) |
| 27 | `vq_cellarray`(nop)| 0 / 0| — |
| 28 | `v_locator`      | 0 / 0  | input (mouse position) |
| 29 | `v_valuator`(nop)| 0 / 0  | — |
| 30 | `v_choice`       | 0 / 1  | input (choice/key) |
| 31 | `v_string`       | 0 / 0  | input (string) |
| 32 | `vswr_mode`      | 0 / 1  | attr (writing/raster mode) |
| 33 | `vsin_mode`      | 0 / 1  | input mode (request/sample) |
| 34 | — (nop)          | 0 / 0  | — |
| 35 | `vql_attributes` | 1 / 3  | inquiry (line attrs) |
| 36 | `vqm_attributes` | 1 / 3  | inquiry (marker attrs) |
| 37 | `vqf_attributes` | 0 / 5  | inquiry (fill attrs) |
| 38 | `vqt_attributes` | 2 / 6  | inquiry (text attrs) |
| 39 | `vst_alignment`  | 0 / 2  | attr (text alignment) |

### Jump table 2 — opcodes 100..134 (`V_OPNVWK_OP = 100`)

| Op | Fn | n_ptsout / n_intout | Group |
|----|----|---------------------|-------|
| 100 | `v_opnvwk`       | 6 / 45 | workstation (virtual open) |
| 101 | `v_clsvwk`       | 0 / 0  | workstation (virtual close) |
| 102 | `vq_extnd`       | 6 / 45 | inquiry (extended capabilities) |
| 103 | `v_contourfill`  | 0 / 0  | output (seed fill) |
| 104 | `vsf_perimeter`  | 0 / 1  | attr (fill outline on/off) |
| 105 | `v_get_pixel`    | 0 / 2  | inquiry (read pixel) |
| 106 | `vst_effects`    | 0 / 1  | attr (text effects: bold/italic/…) |
| 107 | `vst_point`      | 2 / 1  | attr (text height in points) |
| 108 | `vsl_ends`       | 0 / 2  | attr (line end styles) |
| 109 | `vro_cpyfm`      | 0 / 0  | raster (opaque blit) |
| 110 | `vr_trnfm`       | 0 / 0  | raster (format transform) |
| 111 | `vsc_form`       | 0 / 0  | mouse (set cursor form) |
| 112 | `vsf_udpat`      | 0 / 0  | attr (user fill pattern) |
| 113 | `vsl_udsty`      | 0 / 0  | attr (user line style) |
| 114 | `vr_recfl`       | 0 / 0  | output (fill rectangle) |
| 115 | `vqin_mode`      | 0 / 1  | inquiry (input mode) |
| 116 | `vqt_extent`     | 4 / 0  | inquiry (text bounding box) |
| 117 | `vqt_width`      | 3 / 1  | inquiry (char cell width) |
| 118 | `vex_timv`       | 0 / 1  | exchange timer vector |
| 119 | `vst_load_fonts` | 0 / 1  | text (load extra fonts) |
| 120 | `vst_unload_fonts`| 0 / 0 | text (unload fonts) |
| 121 | `vrt_cpyfm`      | 0 / 0  | raster (transparent blit) |
| 122 | `v_show_c`       | 0 / 0  | mouse (show cursor) |
| 123 | `v_hide_c`       | 0 / 0  | mouse (hide cursor) |
| 124 | `vq_mouse`       | 1 / 1  | mouse (query position/buttons) |
| 125 | `vex_butv`       | 0 / 0  | exchange button vector |
| 126 | `vex_motv`       | 0 / 0  | exchange motion vector |
| 127 | `vex_curv`       | 0 / 0  | exchange cursor vector |
| 128 | `vq_key_s`       | 0 / 1  | inquiry (shift-key state) |
| 129 | `vs_clip`        | 0 / 0  | attr (clipping rectangle) |
| 130 | `vqt_name`       | 0 / 33 | inquiry (font name) |
| 131 | `vqt_fontinfo`   | 5 / 2  | inquiry (font metrics) |
| 132 | `vqt_justified`(nop)| — | (PC-GEM) |
| 133 | `vs_grayoverride`(nop)| — | (PC-GEM/3) |
| 134 | `vex_wheelv`     | 0 / 0  | exchange mouse-wheel vector (Milan) |

## The virtual workstation (`Vwk`, `vdi_defs.h`)

`v_opnvwk` allocates a `Vwk` (≤16 of them) holding the *attribute
state* a handle draws with — line colour/type/width, marker
type/height/colour, fill interior/style/colour/perimeter, text
font/colour/height/rotation/alignment/effects, writing mode, the
clipping rectangle (`xmn_clip…`), and the `next_work` link. `CUR_WORK`
points at the active one; the dispatcher resolves `contrl[6]` to it.
A physical workstation (`v_opnwk`) maps the screen device once; AES
and applications then open **virtual** workstations (`v_opnvwk`) that
share the screen but carry independent attributes.

## NewTOS implementation notes (Phase 12)

- **Calling convention.** Native AArch64 source ports call VDI through
  the AES/VDI binding (`vdi(VDIPB*)`), which marshals the five array
  pointers. NewTOS keeps the `VDIPB`/`CONTRL`/`INTIN`/… contract
  bit-for-bit so ported bindings work unchanged; the trap is delivered
  as `svc` (class TBD-ADR, parallel to the GEMDOS/BIOS/XBIOS classes)
  with the `VDIPB` pointer in `x0`. The opcode/array semantics above
  are the ABI surface, not the trap mechanics.
- **Device.** The single screen device draws on the Phase-10
  framebuffer (`hal::framebuffer`) via the Line-A primitives
  (`docs/api/_line-a.md`); only one physical workstation (the screen)
  in the bring-up profile (no printer/plotter/metafile).
- **Coordinate system.** VDI uses GEM NDC vs RC depending on the
  workstation; the screen workstation uses raster coordinates (RC),
  origin top-left — matches the framebuffer directly.
- **Per-function specs** land in `docs/api/vdi/*.md`; tracking in
  `docs/api/vdi/_working-list.md`.
