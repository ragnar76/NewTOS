# `v_opnvwk` — VDI opcode 100 (open virtual workstation)

> Phase 00b spec. Source: EmuTOS @ `1a7484ab` —
> `vdi/vdi_control.c:407` (`vdi_v_opnvwk`), `:269` (`init_wk`),
> `:128` (`DEV_TAB_rom`), `:58` (`SIZ_TAB_rom`). Calling convention:
> [`../_dispatcher.md`](../_dispatcher.md).

Opens a **virtual workstation** on the screen device: allocates a
`Vwk` attribute area, initialises it from the caller's `work_in`
inputs, and returns a fresh **handle** plus the device's capability
tables. AES and applications open a virtual workstation to draw with
independent attributes while sharing the one physical screen.

## Source extraction

```c
void vdi_v_opnvwk(Vwk *vwk) {
    CUR_WORK = &phys_work;                 /* keep CUR_WORK valid */
    /* find a free handle in (VDI_PHYS_HANDLE, LAST_VDI_HANDLE] */
    if (none) { CONTRL[6] = 0; return; }   /* no handle → fail */
    vwk = Mxalloc(sizeof(Vwk), MX_SUPER);
    if (!vwk) { CONTRL[6] = 0; return; }   /* no memory → fail */
    vwk_ptr[handle] = vwk;
    vwk->handle = CONTRL[6] = handle;      /* return the handle */
    init_wk(vwk);                          /* attrs + INTOUT/PTSOUT */
    build_vwk_chain();                     /* link all Vwks */
    CUR_WORK = vwk;
}
```

There are **at most 16 handles** (`VDI_PHYS_HANDLE` = the physical
workstation; virtual ones use the rest up to `LAST_VDI_HANDLE`). The
`Vwk` is heap-allocated (`Mxalloc`, supervisor-accessible). On failure
the function returns with `CONTRL[6] = 0` and no error code (VDI has
no error return — see dispatcher).

## Trap interface

| | |
|--|--|
| Opcode | `contrl[0] = 100` |
| Handle | `contrl[6]` — **ignored on input**, set to the new handle on output (`0` on failure) |
| `# INTIN` (`contrl[3]`) | 11 (`work_in`) |
| `# PTSIN` (`contrl[1]`) | 0 |
| `# INTOUT` (`contrl[4]`) | 45 (set by dispatcher) |
| `# PTSOUT` (`contrl[2]`) | 6 (set by dispatcher; `init_wk` writes 12 WORDs = 6 pairs) |

## Inputs — `INTIN` (`work_in[11]`)

| Idx | Field | Meaning / validation (else default) |
|-----|-------|-------------------------------------|
| 0 | device id | Driver/“Getrez+2”. For a virtual ws the screen driver id; `v_opnwk` uses `rez + 2`. |
| 1 | line type | `1..MAX_LINE_STYLE` → `line_index = v-1`; else `DEF_LINE_STYLE`. |
| 2 | line colour | validated palette index → `MAP_COL[]`. |
| 3 | marker type | `1..MAX_MARK_STYLE`; else `DEF_MARK_STYLE`. |
| 4 | marker colour | validated → `MAP_COL[]`. |
| 5 | text font | **ignored** — you always get the default font. |
| 6 | text colour | validated → `MAP_COL[]`. |
| 7 | fill interior style | `MIN..MAX_FILL_STYLE`; else `DEF_FILL_STYLE`. |
| 8 | fill style index | if interior == pattern: `1..MAX_FILL_PATTERN`; else hatch range; else default. |
| 9 | fill colour | validated → `MAP_COL[]`. |
| 10 | coordinate system | `xfm_mode`: `0` = NDC (normalised), `2` = RC (raster). |

`init_wk` also sets the non-`work_in` defaults: `mark_height = DEF_MKHT`,
`mark_scale = 1`, `wrt_mode = WM_REPLACE`, `line_width = DEF_LWID`,
`line_beg/line_end = SQUARED`, `fill_per = TRUE`, clip rect =
`[0,0]..[xres,yres]` with `clip = FALSE`, the ROM default fill pattern
copied into `ud_patrn[16]`, `multifill = 0`, `ud_ls = LINE_STYLE[0]`.

## Outputs

`CONTRL[6]` = the new handle (`0` = open failed). `INTOUT[0..44]` =
the device capability table (`DEV_TAB`), `PTSOUT[0..11]` = the size
table (`SIZ_TAB`).

### `INTOUT` = `DEV_TAB` (45 WORDs, device capabilities)

ROM template (`DEV_TAB_rom`), patched at init with the real screen
geometry (`[0]=V_REZ_HZ-1`, `[1]=V_REZ_VT-1`, `[13]=#colours`,
`[35]`=colour-capable, `[39]`=palette size, `[10]`=#fonts):

| Idx | Capability | Idx | Capability |
|-----|-----------|-----|-----------|
| 0 | x resolution − 1 | 23 | GDP filled rounded rect |
| 1 | y resolution − 1 | 24 | GDP justified text |
| 2 | device precision (0=exact) | 25–34 | GDP attribute codes (#1..#10) |
| 3 | pixel width | 35 | colour capability (0=mono) |
| 4 | pixel height | 36 | text rotation |
| 5 | # text font heights | 37 | polygon fill |
| 6 | # line styles | 38 | cell array |
| 7 | line width flag | 39 | palette size |
| 8 | # marker types | 40 | # locator devices (mouse) |
| 9 | # marker sizes | 41 | # valuator devices |
| 10 | # text fonts | 42 | # choice devices |
| 11 | # area (fill) patterns | 43 | # string devices |
| 12 | # crosshatch patterns | 44 | workstation type (2 = in/out) |
| 13 | # simultaneous colours | | |
| 14 | # GDPs (10) | | |
| 15–24 | GDP ids: bar, arc, pieslice, circle, ellipse, ell-arc, ell-pie, rounded-rect, filled-rounded-rect, justified-text | | |

### `PTSOUT` = `SIZ_TAB` (12 WORDs = 6 pairs, sizes in device coords)

| Idx | Size | Idx | Size |
|-----|------|-----|------|
| 0 | min char width | 6 | max line width |
| 1 | min char height (7) | 7 | reserved |
| 2 | max char width | 8 | min marker width (15) |
| 3 | max char height (7) | 9 | min marker height (11) |
| 4 | min line width (1) | 10 | max marker width (120) |
| 5 | reserved | 11 | max marker height (88) |

## Errors

None (no VDI error return). Failure paths set `CONTRL[6] = 0`:
no free handle (all 16 in use) or `Mxalloc` out of memory.

## Dependencies

- `Mxalloc(MX_SUPER)` (GEMDOS 0x44) for the `Vwk`; `Mfree` on close.
- `validate_color_index` / `MAP_COL[]`, `st_fl_ptr`, `text_init2`.
- Globals: `vwk_ptr[]`, `CUR_WORK`, `phys_work`, `DEV_TAB`/`SIZ_TAB`
  (Line-A vars, `lineavars.S`), `xres`/`yres`, `flip_y`.

## NewTOS implementation notes (Phase 12)

- The `Vwk` is a real heap allocation (`Malloc`/`Mxalloc` already work,
  Phase 04/06). Keep the 16-handle cap and the `CUR_WORK`/chain model
  for Line-A compatibility.
- **Per-device / multi-screen (ADR-011).** The *physical* workstation
  is a **per-device struct** (own `Framebuffer` + geometry + palette)
  in a screen-device registry — not EmuTOS's global `phys_work` /
  Line-A screen vars. Each `Vwk` links to its physical device; the
  primitives draw into that device's framebuffer. `v_opnwk(device_id)`
  selects the screen. This keeps the API bit-exact while leaving the
  door open for the Pi 4's two HDMI outputs. (AES-level multi-monitor
  is out of scope — see ADR-011.)
- `DEV_TAB`/`SIZ_TAB` are filled from the **framebuffer** geometry
  (`hal::framebuffer` width/height) and the bring-up palette
  (8-bit/palette first; `CONF_WITH_VDI_16BIT` truecolor is out of
  scope initially). `[0]=width-1`, `[1]=height-1`.
- `work_in[10]` (NDC vs RC): the screen workstation is RC (raster,
  origin top-left) — matches the framebuffer directly; NDC scaling is
  a later refinement.
- The default font (`work_in[5]` ignored) is the system bitmap font
  the framebuffer text path provides.
