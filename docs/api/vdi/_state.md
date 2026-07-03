# VDI shared state — workstation level (Phase 00b)

> Source: EmuTOS @ `1a7484ab`. Verified against
> `vdi/vdi_control.c:41-52` (`mcs_ptr`, `vwk_ptr[]`, `phys_work`),
> `:25-27` (handle range), `bios/lineavars.S:114-136`
> (`_INQ_TAB`/`_DEV_TAB`/`_SIZ_TAB`/`_CUR_WORK`), `vdi/vdi_defs.h:266-281`
> (extern declarations). Calling convention:
> [`_dispatcher.md`](_dispatcher.md).
>
> **Scope.** This file captures only **workstation-level** shared state
> (the handle table, capability tables, and the current-workstation
> pointer). The per-attribute state (line/marker/fill/text defaults, the
> `MAP_COL`/palette machinery) is owned by the attributes/inquiry specs
> and by [`../structures/vwk.md`](../structures/vwk.md); a later
> attributes agent extends this file if needed.
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> The **values** `DEV_TAB`/`INQ_TAB`/`SIZ_TAB` report are contract
> (programs read them via `v_opnwk`/`v_opnvwk`/`vq_extnd`). The **storage**
> (Line-A globals, the `vwk_ptr[]` array, the `next_work` chain) is
> **reference** — NewTOS keeps an equivalent registry per ADR-011.

## The handle table (`vwk_ptr[]`)

- `static Vwk *vwk_ptr[NUM_VDI_HANDLES+1]` (`vdi_control.c:48`). Entry `n`
  points at the `Vwk` for VDI handle `n`; entry 0 is unused.
- Handles run `FIRST_VDI_HANDLE = 1` (`= VDI_PHYS_HANDLE`, the physical
  workstation) through `LAST_VDI_HANDLE = NUM_VDI_HANDLES`
  (`vdi_control.c:25-27`).
- `NUM_VDI_HANDLES` is **build-config**: 64 on the small/STe target, 128
  by default (`include/config.h:391-393,1660-1662`). **Not 16.**
- `get_vwk_by_handle(handle)` (`vdi_control.c:178`) range-checks and
  returns `vwk_ptr[handle]` (NULL if closed/out-of-range). The dispatcher
  calls it for every opcode except `v_opnwk`/`v_opnvwk`; a NULL result is
  a silent no-op (VDI's only "error").

### Lifecycle transitions

| Event | Effect on `vwk_ptr[]` / `CUR_WORK` |
|-------|-------------------------------------|
| `v_opnwk` (1) | `vwk_ptr[1] = &phys_work`; `vwk_ptr[2..] = NULL`; `CUR_WORK = &phys_work` (`vdi_control.c:542-567`) |
| `v_opnvwk` (100) | first free slot in `[2..LAST]` → new `Mxalloc`'d `Vwk`; `CUR_WORK = new` (`:420-459`) |
| `v_clsvwk` (101) | `vwk_ptr[handle] = NULL`, `Mfree`; `CUR_WORK = &phys_work` (`:462-491`) |
| `v_clswk` (2) | `Mfree` every `vwk_ptr[2..]`, set NULL; `CUR_WORK = vwk_ptr[1]` (`:573-590`) |

`build_vwk_chain()` (`:390-403`) relinks all live `Vwk`s through
`next_work` after every open/close, because some programs (Warp9) walk
the chain. **Reference** detail.

## `CUR_WORK`

- `Vwk *CUR_WORK` (Line-A global, `lineavars.S:136`; extern
  `vdi_defs.h:281`) — the currently-active virtual workstation.
- Always kept pointing at a **valid open** workstation (the physical one
  as the fallback), even on an early-exit open failure
  (`vdi_control.c:417`) and after any close.
- Used by the Line-A flood fill (which reads the fill colour from
  `CUR_WORK`) and mirrored from the resolved `vwk` at the end of every
  non-close dispatch (`vdi_main.c:183-190`).

## The capability tables — `DEV_TAB`, `INQ_TAB`, `SIZ_TAB`

Three WORD arrays living as Line-A globals, populated by `v_opnwk` from
ROM templates and patched with the live geometry. Their **contents** are
the contract; where they physically live is reference.

| Table | Size | ROM template | Returned by | Live patch |
|-------|------|--------------|-------------|------------|
| `DEV_TAB` | 45 WORDs (`lineavars.S:118`) | `DEV_TAB_rom` (`vdi_control.c:128`) | `INTOUT` of `v_opnwk`/`v_opnvwk`; `vq_extnd` mode 0 | `[0,1]` res−1, `[3,4]` pixel size, `[13]` #colours, `[35]` colour-cap, `[39]` palette (`update_rez_dependent`, `:203-208`) |
| `INQ_TAB` | 45 WORDs (`lineavars.S:114`) | `INQ_TAB_rom` (`vdi_control.c:76`) | `INTOUT` of `vq_extnd` mode 1 | `[4]` #planes, `[5]` video-LUT (`:210-213`); `[19]` clip flag patched per query in `vq_extnd` |
| `SIZ_TAB` | 12 WORDs used / 15 allocated (`lineavars.S:132`) | `SIZ_TAB_rom` (`vdi_control.c:58`) | `PTSOUT` of `v_opnwk`/`v_opnvwk`; `vq_extnd` mode 0 | `[0..3]` char metrics via `text_init` |

Full byte-index tables:
- `DEV_TAB` / `SIZ_TAB` → [`workstation/100-v_opnvwk.md`](workstation/100-v_opnvwk.md).
- `INQ_TAB` → [`workstation/102-vq_extnd.md`](workstation/102-vq_extnd.md).

The convenient aliases `xres = DEV_TAB[0]`, `yres = DEV_TAB[1]`,
`numcolors = DEV_TAB[13]` (`vdi_defs.h:94-98`) are used throughout the VDI.

## `phys_work` and `mcs_ptr`

- `Vwk phys_work` — the static attribute area for the physical
  workstation (extern `vdi_control.c:49`); never heap-allocated, so
  `v_opnwk` cannot fail for memory.
- `MCS *mcs_ptr` (`vdi_control.c:41`) — mouse-cursor save area, sized by
  `v_planes` in `update_rez_dependent`. Mouse-subsystem state; documented
  fully under [`input/_mouse.md`](input/_mouse.md).

## NewTOS implementation notes (Phase 12)

- The handle registry is a NewTOS structure (not a raw `Vwk *[]` at a
  Line-A address); `CUR_WORK`, the chain, and the tables are reimplemented
  per **ADR-011** as per-physical-device state (each screen device owns
  its `DEV_TAB`/`INQ_TAB`/`SIZ_TAB` + palette). The **values** returned to
  programs must match EmuTOS's for the chosen framebuffer profile.
- **Open questions** (see [`_discrepancies.md`](_discrepancies.md)):
  the `NUM_VDI_HANDLES`/`MAX_VERTICES` profile (D-VDI-3/D-VDI-7) and the
  exact `DEV_TAB[13/35/39]` / `INQ_TAB[4/5]` values NewTOS reports for a
  640×480×16-colour framebuffer (D-VDI-7).
