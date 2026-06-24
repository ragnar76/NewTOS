# Phase 14 — Desktop

> **Status:** in progress (since 2026-06-22). Depends on Phase 13 (AES).

## Goal

**Reimplement the behaviour** of EmuTOS' `desk/` — the GEM desktop /
file-manager shell: drive icons, menu bar with the real menus, directory
windows, file operations, the application launcher, and desktop
configuration. Per [`docs/COMPATIBILITY.md`](../docs/COMPATIBILITY.md) this
is a **clean reimplementation against the extracted behaviour**, *not* a
source-port of the EmuTOS C. (Source-porting is for external TOS
applications — Phase 15 — not for EmuTOS' own shell.)

The user-observable surface to match is the desktop's *behaviour* (menu
items + what they do, window/file semantics, the `NEWDESK.INF` format); the
implementation (Rust, kernel-side rendering over the AES) is NewTOS's own.

## EmuTOS sources (extracted, not ported)

- `desk/*` — behaviour extracted to [`docs/api/desktop.md`](../docs/api/desktop.md).
- The boot/shell flow extracted to
  [`docs/api/emutos-boot-flow.md`](../docs/api/emutos-boot-flow.md)
  (`biosmain` → AUTO folder → `gem_main` → `sh_main` shell loop).

## Deliverables

Done (verified by QMP screendump / serial on raspi4b):

- **D1** Desktop screen — `g_patcol` background, menu bar, drive icons.  ✅
- **D2** Real EmuTOS icons from `icon.rsc` (drives/trash/printer/folder/doc)
  + the modern 3D look.  ✅
- **D3** Full drop-down menus (the real `ADMENU` item set), authentic to an
  Atari target (Eject/Blitter/Cache dropped — non-Atari / no-HW).  ✅
- **D4** Window manager integration + directory windows (text & icon view,
  sort, size-to-fit) with the desktop-background paint hook.  ✅
- **D5** Menu actions — File/View groups A+B (Close/Cycle, Sort, view,
  New folder, Delete + confirm, Drag-to-Trash, Info/Rename).  ✅
- **D6** Data-driven desktop config — `g_patcol` model, `NEWDESK.INF` `#Q`
  load + Save desktop.  ◑ (only `#Q` so far)

Open (from the boot-flow cross-check — these were not in the original
3-deliverable plan):

- **D7** **The shell loop** — a `sh_main`-style loop driven by
  `shel_write`/`sh_nextapp` (DESKTOP / NORMAL-app / CONSOLE / autorun), so
  the desktop can launch a program and have it return to the desktop. This
  is the structural backbone for D8–D10.  ☐
- **D8** **Application launch** — double-click a `.PRG` / `shel_write` →
  `Pexec`, return to the desktop on exit.  ☐
- **D9** **Execute EmuCON** — launch the EmuCON console (CONSOLE_APP) and
  return to the desktop.  ☐ (a serial EmuCON-style shell already exists in
  `kernel/src/shell.rs`; wire it as a launchable console app)
- **D10** **AUTO folder** — run `\AUTO\*.PRG` (directory order) at boot
  before the desktop.  ☐
- **D11** Fuller `NEWDESK.INF` — `#E` (settings), `#W` (window positions),
  `#Z` (autorun), `#M`/`#T`/`#O` (icon placement).  ☐
- **D12** Desk accessories — `\*.ACC` loading.  ☐ (later; needs the
  multi-process model)
- **D13** Remaining File/Options items — Search, Select all, Set file mask,
  Set-background/Desktop-config picker dialogs, Install…, Preferences,
  Change resolution.  ☐

## Architecture decision (open) — desktop: kernel module or process?

EmuTOS runs the desktop as a **process** inside the `sh_main` shell loop, so
it can be swapped with EmuCON / applications and resumed. NewTOS currently
runs the desktop as a **kernel module** (`kernel/src/desktop.rs`,
`desktop::run()` hardwired at boot). That is fine for the standalone desktop
but blocks the shell loop (D7) and thus app launch / EmuCON (D8/D9).

**Decision needed before D7:** keep the desktop kernel-side but add a shell
loop the kernel drives, **or** move the desktop to an EL0 process started
via `Pexec` (closer to EmuTOS, ties into the Phase-06 process / Pwait
work). To be recorded as an ADR in `docs/MEMORY.md` when chosen. (Note: the
original plan's "D3 Boot integration: `Pexec(desktop)`" assumed the process
model; we diverged to a kernel module — this ADR resolves that.)

## Exit criteria

- **EC1** Booting NewTOS lands in a working desktop: drive icons, menu bar,
  open a directory window, file operations.  ✅
- **EC2** Launch a program from the desktop and return to it on exit
  (needs D7/D8).  ☐
- **EC3** "Execute EmuCON" runs the console and returns to the desktop
  (D9).  ☐
- **EC4** `\AUTO\*.PRG` run at boot (D10).  ☐
- **EC5** Desktop config (pattern/colour, window positions) persists across
  a reboot via `NEWDESK.INF`.  ◑ (pattern/colour done; positions = D11)
