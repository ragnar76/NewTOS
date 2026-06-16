# Phase 14 — Desktop

> **Status:** pending. Depends on Phase 13.

## Goal

Port EmuTOS' `desk/` directory as a NewTOS userland program: the
desktop application with file manager, drive icons, application
launcher, control-panel hooks.

## EmuTOS sources to read

- `desk/*` — entire directory.

## Deliverables

D1 Source-port `desk/` to AArch64 + NewTOS APIs.
D2 RSC asset embedding.
D3 Boot integration: `Pexec(desktop)` after init.

## Exit criteria

EC1 Booting NewTOS lands in a working desktop with drive icons,
   menu bar, ability to open a directory window and launch a
   sample program.
