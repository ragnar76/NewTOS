# `Vwk` — VDI virtual-workstation attribute area

> Phase 00b spec. Source: EmuTOS @ `1a7484ab` — `vdi/vdi_defs.h`
> (`struct Vwk_`), initialised by `vdi/vdi_control.c:init_wk`. Used by:
> every VDI drawing/attribute call ([`../vdi/_dispatcher.md`](../vdi/_dispatcher.md)).

The `Vwk` holds the **per-handle attribute state** a virtual
workstation draws with. `v_opnvwk` allocates one (`Mxalloc`), `init_wk`
fills it, `CUR_WORK` points at the active one, and the `next_work`
chain links them all. Unlike [`BasePage`](basepage.md) it is **not** a
fixed-ABI structure that ported programs parse — programs hold only the
*handle* (`contrl[6]`); the VDI resolves it to the `Vwk`. (EmuTOS notes
some programs fake a `Vwk` by pointing `CUR_WORK` at a 16-element array
for Line-A, so the *first several* fields have de-facto Line-A meaning;
treat the layout as internal otherwise.)

## Field groups

### Line attributes
| Field | Meaning |
|-------|---------|
| `line_index` | line style (0-based; `vsl_type`) |
| `line_color` | PEL colour (`vsl_color`) |
| `line_width` | width in pixels (`vsl_width`) |
| `line_beg` / `line_end` | end styles — squared/arrow/round (`vsl_ends`) |
| `ud_ls` | user-defined line style bitmask (`vsl_udsty`) |

### Marker attributes
| Field | Meaning |
|-------|---------|
| `mark_index` | marker type (`vsm_type`) |
| `mark_color` | PEL colour (`vsm_color`) |
| `mark_height` / `mark_scale` | height + scale (`vsm_height`) |

### Fill attributes
| Field | Meaning |
|-------|---------|
| `fill_style` | interior style: hollow/solid/pattern/hatch/user (`vsf_interior`) |
| `fill_index` | pattern/hatch index (`vsf_style`) |
| `fill_color` | PEL colour (`vsf_color`) |
| `fill_per` | TRUE = outline the fill (`vsf_perimeter`) |
| `patptr` / `patmsk` | active pattern pointer + mask |
| `ud_patrn[UDPAT_PLANES*16]` | user-defined fill pattern (`vsf_udpat`) |
| `multifill` | multi-plane fill flag |

### Text attributes
| Field | Meaning |
|-------|---------|
| `cur_font` / `loaded_fonts` / `num_fonts` | active + loaded fonts (`vst_font`, `vst_load_fonts`) |
| `text_color` | PEL colour (`vst_color`) |
| `style` | effects: bold/light/italic/underline/outline (`vst_effects`) |
| `h_align` / `v_align` | alignment (`vst_alignment`) |
| `chup` | character-up (baseline rotation) vector (`vst_rotation`) |
| `pts_mode` / `t_sclsts` / `scaled` / `dda_inc` / `scratch_head` / `scrtchp` / `scrpt2` | height/scaling state (`vst_height` / `vst_point`) |

### Mode + clipping
| Field | Meaning |
|-------|---------|
| `wrt_mode` | writing/raster mode: replace/transparent/xor/reverse-transparent (`vswr_mode`) |
| `xfm_mode` | coordinate transform requested: 0=NDC, 2=RC |
| `clip` | clipping on/off (`vs_clip`) |
| `xmn_clip`/`ymn_clip`/`xmx_clip`/`ymx_clip` | clip rectangle |

### Bookkeeping
| Field | Meaning |
|-------|---------|
| `handle` | the handle this attribute area is for |
| `next_work` | next `Vwk` in the chain |
| `ext` (`CONF_WITH_VDI_16BIT`) | 16-bit truecolor palette extension — **out of bring-up scope** |

## NewTOS implementation notes

- Allocated from the kernel/BDOS heap (`Mxalloc`), 16-handle cap; the
  `Vwk`'s field layout is **internal** (not an ABI promise), so NewTOS
  may reorder/retype fields freely as long as the VDI call semantics
  match. Keep the first Line-A-visible fields only if/when a real
  Line-A binding needs them.
- **Per-device link (ADR-011).** NewTOS adds a `device` reference to
  the `Vwk` (which physical screen workstation / `Framebuffer` it
  draws on); EmuTOS implies the single global screen. Drawing
  primitives take the target framebuffer from `vwk->device`, not a
  global `v_bas_ad`.
- Colours are stored as **PEL** (hardware palette index) after mapping
  through `MAP_COL[]`; the framebuffer palette resolves PEL → RGB.
