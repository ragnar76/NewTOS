# PHASE-07: BIOS / XBIOS Devices

## Goal

Implement the BIOS device abstraction (character device table) and the
BIOS character I/O syscalls. Add the XBIOS dispatcher and the device
registration / status XBIOS calls. This phase **does not** include display
or sound — those are Phase 08.

## Inputs

- `CLAUDE.md`, Phases 01–06 complete
- `docs/api/bios.md` and `docs/api/xbios.md` drafted
- Atari Compendium, BIOS and XBIOS chapters
- EmuTOS: `bios/biosmain.c`, `bios/xbios.c`, `bios/chardev.c`

## Deliverables

### D1: BIOS device table
TOS has a fixed table of character devices: `BCONIN`, `BCONOUT`, `BCONSTAT`,
`BCOSTAT` indexed by device number (0=PRT, 1=AUX, 2=CON, …). Implement the
table with traits per device.

### D2: BIOS dispatch via `trap #13`
Wire trap 13 through the same exception path as GEMDOS. Different SVC
immediate; different table; same dispatcher code reused.

### D3: BIOS syscalls
Per spec:
- `Bconstat` (1), `Bconin` (2), `Bconout` (3)
- `Bcostat` (8)
- `Getmpb` (0), `Rwabs` (4) — Rwabs uses block driver from Phase 04
- `Setexc` (5) — exception vector hook (limited support; document constraints)
- `Tickcal` (6), `Getbpb` (7)
- `Mediach` (9), `Drvmap` (10), `Kbshift` (11)

### D4: XBIOS dispatch via `trap #14`
Same pattern as D2/D3. XBIOS table.

### D5: XBIOS device-related syscalls (subset)
- `Initmous` (0), `Ssbrk` (1, deprecated, error return)
- `Random` (17), `Protobt` (18)
- Keyboard/mouse table access: `Kbdvbase` (34)
- `Iorec` (14)
- `Mfpint` (13), `Xbtimer` (31) — partially: timer abstraction without MFP

### D6: Tick interrupt
A 200 Hz system timer (per TOS convention) using the AArch64 generic timer.
Increments the system tick variable visible via `_hz_200` and `Tickcal`.

## Exit criteria

- **EC1**: A test process calls `Bconout(2, 'A')` and 'A' appears on the
  console.
- **EC2**: `Tickcal` returns the configured tick rate (5 ms = 200 Hz).
- **EC3**: `Drvmap` returns a bitmask reflecting available drives (just
  C: in current state).
- **EC4**: All BIOS/XBIOS spec entries for this phase complete.

## Out of scope

- Display setup (Phase 08)
- Sound (Phase 08 or later)
- IKBD packet interface — replaced by direct USB HID handling (Phase TBD)
- ACSI/SCSI commands

## Risks

- **R1**: TOS exception vector hooks (`Setexc`) are powerful and largely
  incompatible with a memory-protected kernel. Decide what to support and
  document in ADR. Likely answer: only system vectors, not arbitrary ones.
- **R2**: `Mediach` semantics. Returns 0/1/2 for "no change / changed /
  uncertain". Get this right or apps misbehave.
- **R3**: Multiple traps sharing exception path. The dispatcher must
  switch on the SVC immediate cleanly; don't let GEMDOS args bleed into
  XBIOS.

## Claude Code instructions

- Phase 07 introduces two new TOS API surfaces. Each function gets a spec
  entry **before** implementation, same rule as before.
- Many XBIOS calls relate to hardware that doesn't exist on RPi4 (MFP
  timers, ACIA, IKBD, GLUE). They get specified, return a documented error,
  and are revisited only if a real port needs them. Document this position
  in `docs/ARCHITECTURE.md` once and refer to it from each affected call.
