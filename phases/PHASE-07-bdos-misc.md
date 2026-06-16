# Phase 07 — BDOS Misc (Time + RX completion)

> **Status:** pending. Small, follows Phase 06.

## Goal

Close out the remaining BDOS leaf calls that were not part of Phase 02
because they need `Pterm(-32)` (Ctrl-C) or wall-clock time:

- Blocking console reads: `Cconin`, `Cnecin`, `Crawcin`, `Cconrs`.
- Time/date: `Tgetdate`, `Tsetdate`, `Tgettime`, `Tsettime`.
- `Sversion`.

## EmuTOS sources to read

- `bdos/console.c` — the blocking branches of `xcconin` etc.
- `bdos/time.c` — date/time conversion to/from BIOS clock.
- `bios/clock.c` — RTC; for QEMU/RPi4 we use the system timer.

## Phase 00 prerequisites

- `docs/api/gemdos/console/{01,07,08,0a}-*.md` (already drafted)
- `docs/api/gemdos/time/*.md`
- `docs/api/gemdos/version/30-sversion.md`

## Deliverables

D1 BIOS clock source (CNTPCT_EL0 + offset).
D2 `Tgetdate`/`Tgettime`/`Tsetdate`/`Tsettime`.
D3 Blocking `Cconin`/`Cnecin`/`Crawcin` routed through Phase 02 RX
   ringbuffer with Phase 06 `Pterm(-32)` Ctrl-C hook.
D4 `Cconrs` line editor (the EmuTOS conrs state machine).
D5 `Sversion` returning a NewTOS-recognisable value.

## Exit criteria

EC1 `Tgettime` updates between back-to-back calls.
EC2 Pressing Ctrl-C inside a blocking `Cconin` triggers `Pterm(-32)`.
EC3 `Cconrs` accepts a line, honours the buffer length cap, returns
    on CR.

## Out of scope

- IKBD scancode-to-ASCII layout switching beyond ANSI default
  (Phase 11/13 problem).
