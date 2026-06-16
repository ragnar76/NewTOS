# Phase 12 — VDI

> **Status:** pending. Depends on Phase 10 (framebuffer).

## Goal

Implement the Virtual Device Interface on Trap 2, function 115
(traditional `vdi(VDIPB*)`). Cover line-A primitives (line, polyline,
fill, text, raster ops) and at least one workstation type
(NEWGDOS-compatible monochrome / 16-color / 256-color).

## EmuTOS sources to read

- `vdi/vdi_main.c`, `vdi_init.c`
- `vdi/vdi_line.c`, `vdi_fill.c`, `vdi_misc.c`, `vdi_text.c`,
  `vdi_marker.c`, `vdi_input.c`
- `vdi/lineavars.S` — Line-A globals (touched by Phase 11 too).

## Phase 00 prerequisites

- `docs/api/_line-a.md` and full `docs/api/vdi/` (PHASE-00b).

## Deliverables

(To be detailed when this phase opens. Skeleton:)

D1 VDI dispatcher.
D2 `v_opnvwk`/`v_clsvwk` (workstation lifecycle).
D3 Line / polyline / rectangle.
D4 Fill (solid, hatched, user-pattern).
D5 Text rendering (8×16 system font from EmuTOS).
D6 Bit-block transfer (vrt_cpyfm / vro_cpyfm).
D7 Mouse cursor (line-A `gem_mouse`).

## Exit criteria

EC1 A VDI-test program drawing primitives matches EmuTOS' output
   pixel-for-pixel at 640×480×1.

## Out of scope

- Bezier extensions — not in classic VDI.
- 24-bit color — Falcon-only, post-MVP.
