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

- **D7** **The shell loop** — a `sh_main`-style loop dispatching a
  `MenuAction` (the kernel analogue of `sh_nextapp`: DESKTOP / CONSOLE /
  …app). The CONSOLE case is wired (see D9); the NORMAL-app case is D8.  ◑
- **D8** **Application launch** — double-click a `.PRG` / `shel_write` →
  `Pexec`, return to the desktop on exit.  ☐
- **D9** **Execute EmuCON** — File → Execute EmuCON runs the EmuCON console
  (`shell::run_console`) full-screen and returns to the desktop on `exit`.
  ✅ (console launch-target verified by screendump; full round-trip needs a
  real mouse+keyboard)
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

**Decision (2026-06-24):** keep the desktop **kernel-side** and drive the
shell loop from the kernel (`MenuAction` dispatch in `desktop::step`), rather
than making the desktop an EL0 process. Rationale: it reuses the working
desktop + the existing `shell::run_console`, with far less risk than the
EL0/`Pexec`/scheduler path; the EmuCON case (D9) is done this way. **D8**
(launching an external `.PRG`) will still need the EL0 `Pexec` path — at that
point the desktop stays kernel-side but the *launched program* is the EL0
process (the kernel waits for its `Pterm` and repaints the desktop), so the
process model is only needed for the child, not the desktop itself.

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
