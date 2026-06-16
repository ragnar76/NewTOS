# Phase 11 — XBIOS (Trap 14)

> **Status:** pending. Depends on Phase 10 for screen calls and
> Phase 08 for IKBD.

## Goal

Implement Trap 14 — the eXtended BIOS — for everything that does
not belong in BIOS Trap 13: timing (`Vsync`, `Tickcal`, `Random`),
keyboard tables (`Keytbl`, `Kbdvbase`, `Kbrate`), mouse (`Initmous`),
sound stubs (Giaccess / Dosound), MIDI/serial stubs, screen-mode
helpers (`Eset*`/`Eget*`/`Vsetmode`/`Vsetscreen`).

## EmuTOS sources to read

- `xbios/*` — already triaged in Phase 00 D5; spec set complete.
- `bios/ikbd.c` — IKBD vectors backing `Kbdvbase`.

## Phase 00 prerequisites

- All of `docs/api/xbios/` (Phase 00 D5 — 100% triaged).

## Deliverables

D1 Trap 14 dispatcher.
D2 IKBD-vector emulation (`Kbdvbase` returning live function ptrs).
D3 `Vsync` (frame interrupt count from Phase 10).
D4 `Random` / `Tickcal`.
D5 PSG/Dosound — record state, no audio yet (or Phase 14 audio).
D6 MIDI/Serial — `EUNDEV` stubs from triage doc.
D7 Floppy/DMA — `EUNDEV` stubs (we have no floppy).
D8 Screen helpers from `_tt-shifter.md` group.

## Exit criteria

EC1 An EmuTOS-style program calling `Kbdvbase()` then poking its
   `vkbd` vector receives keyboard scancodes from our PL011/IKBD.
EC2 `Vsync()` blocks until next frame, accuracy ±1 frame.
