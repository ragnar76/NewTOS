# PHASE-12: GEM / AES / VDI

## Goal

Implement GEM (Graphics Environment Manager) on NewTOS: AES (Application
Environment Services), VDI (Virtual Device Interface), and the Line-A
graphics primitives. This is the largest phase by code volume.

This phase is intentionally late. It depends on a stable kernel,
filesystem, processes, display, and ideally m68k support so existing GEM
applications can run.

## Inputs

- `CLAUDE.md`, Phases 01–11 complete
- Atari Compendium, AES + VDI sections (large)
- "Atari ST GEM Programmer's Reference" (Abacus)
- EmuTOS: `aes/`, `vdi/`, `bios/lineavars.h`, `bios/lineawk.c`
- "Atari Profibuch ST/STE", AES + VDI sections (German reference)

## Deliverables

### D1: VDI low-level rendering
Software bitmap drawing primitives: line, polygon, polymark, polytext,
fill area, attribute getters/setters. Operates on a virtual screen in
TOS pixel format. Backed by Phase 08's compositor.

### D2: VDI workstation API
`v_opnwk`, `v_clswk`, `v_opnvwk`, `v_clsvwk`, …. Workstation handles,
attribute tables, escape function dispatch.

### D3: Line-A primitives
Direct register-poke equivalents that real TOS programs use to bypass VDI
for speed. Backed by D1 internally. Programs accessing Line-A variables
expect specific memory addresses — emulate via the m68k layer's
translation.

### D4: AES window management
Window list, redraw rectangles, window state machine. Top window receives
input. AES events: keyboard, mouse, message, timer, button, mesag.

### D5: AES form management
`form_do`, `form_dial`, `form_alert`, `form_keybd`, `objc_*` family. The
object tree format and walking algorithms.

### D6: AES menu management
`menu_bar`, `menu_tnormal`, `menu_text`, `menu_register`. Menu bar
rendering and event dispatch.

### D7: AES resource files (.RSC)
Loader for TOS .RSC files. Object trees, strings, free strings, tedinfo,
iconblk, bitblk. Required for almost every GEM program.

### D8: GEM desktop
Optional: a minimal desktop that can launch programs from SD card. If
deferred, document.

## Exit criteria

- **EC1**: A small GEM application (e.g., a simple text editor with
  menu bar) loads and is functional.
- **EC2**: `form_alert` displays a dialog, accepts a button click,
  returns the right value.
- **EC3**: A redraw cycle on window move is visually correct (no
  artifacts, no flicker beyond compositor frame rate).
- **EC4**: A real GEM .RSC file loads and renders correctly.

## Out of scope

- Full TOS desktop replication
- Bezel/3D widgets (TOS 4 / MiNT extensions)
- Color icon extensions (specific to later TOS versions)
- Drag and drop protocol (XAcc messages — possibly later)

## Risks

- **R1**: VDI is enormous. Estimate is 6–12 months solo. Consider scoping
  to a minimal viable subset and expanding.
- **R2**: GEM is event-driven and assumes cooperative multitasking. The
  Phase 06 cooperative scheduler is sufficient. If preemptive multitasking
  is added later, GEM event dispatch needs a serialization layer.
- **R3**: Pixel-exact behavior. Some GEM programs depend on exact pixel
  positioning of widgets. The TOS pixel grid (640×400 standard) is
  authoritative. Software rendering must match.
- **R4**: Mouse cursor. GEM has its own cursor sprite handling. Must
  composite with the framebuffer correctly without flicker.

## Claude Code instructions

- VDI/AES is bigger than all earlier phases combined. Subdivide into
  PHASE-12a (VDI primitives), PHASE-12b (VDI workstations), PHASE-12c
  (AES core), etc. Each sub-phase gets its own document when started.
- Treat EmuTOS's AES and VDI implementations as the gold reference for
  edge-case behavior. Do not blindly translate the C code, but for
  ambiguous Compendium passages, EmuTOS is the tiebreaker.
- This is the phase where compatibility with existing GEM software is
  earned. Test with real applications throughout, not just synthetic
  tests.
