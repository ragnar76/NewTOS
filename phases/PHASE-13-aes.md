# Phase 13 — AES

> **Status:** pending. Depends on Phase 12 (VDI) and Phase 06.

## Goal

Implement the Application Environment Services on Trap 2,
function 200: `appl_*`, `evnt_*`, `wind_*`, `menu_*`, `form_*`,
`objc_*`, `rsrc_*`, `fsel_*`, `graf_*`, `scrp_*`, `shel_*`.

## EmuTOS sources to read

- `aes/*` (full directory — large, will be its own subsystem).

## Phase 00 prerequisites

- `docs/api/aes/*` (PHASE-00b deliverable, not yet started).

## Deliverables

(Detailed when this phase opens. AES is the largest single
sub-system; expect 3–4 sub-phases or a separate work package.)

D1 AES dispatcher.
D2 `appl_init` / `appl_exit` / message pipe.
D3 Event multiplexer (`evnt_multi`).
D4 Window manager (`wind_*`).
D5 Menu / dropdown.
D6 Object library + form rendering.
D7 RSC file loader.
D8 File selector (`fsel_*`).
D9 Clipboard (`scrp_*`) and shell (`shel_*`) integration.

## Exit criteria

EC1 A two-window AES demo runs (open, drag, close).
EC2 RSC-driven dialog renders correctly.
