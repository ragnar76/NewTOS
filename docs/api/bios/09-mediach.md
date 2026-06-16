# `Mediach` — BIOS 0x09

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/bios.c:1158` (`mediach`).

## EmuTOS source

```c
LONG mediach(WORD drv)
{
    return protect_w(hdv_mediach, drv);
}
```

`hdv_mediach` (OS variable at addr `0x47A`) points at the active
driver's media-change probe — typically `blkdev_mediach()`
(`bios/blkdev.c`), which dispatches per-drive to the floppy
controller's WP-line check, the ACSI driver's spin-up state, etc.

## Trap interface

| | |
|--|--|
| Trap | 13 (BIOS) |
| Function ID | `0x09` |
| WORD args | 1 (WORD) |
| Stack frame | `+2: drv (W)`, `+0: 0x0009` |
| Vector entry | `bios_vecs[9]` |

## Inputs

| Pos | Name  | Type | Range / meaning                              |
|-----|-------|------|----------------------------------------------|
| 0   | `drv` | WORD | physical drive index                          |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (3-state media-change indicator) |

| Value | Meaning                                                |
|------:|--------------------------------------------------------|
| 0     | media **definitely has not** changed                   |
| 1     | media **may** have changed (re-read BPB to be sure)    |
| 2     | media **definitely has** changed                       |

## Errors

`Mediach` does not return GEMDOS-style errors. Any underlying I/O
failure is collapsed into "may have changed" (1) so that the
caller's recovery path (re-mount / re-read BPB) takes over.

## Edge cases

### 1. Three-state semantics

The split between "may" and "definitely" exists because some
hardware (1.44 MB floppies on a stock ST) cannot distinguish
"new disk" from "same disk reinserted" without re-reading the
boot sector. The "may" case forces GEMDOS to re-validate the BPB
and serial number; if those match, treat as no-change.

### 2. Floppy WP-line trick

Atari's standard floppy media-change is a hardware signal latched
in the WD1772. Reading the floppy's status register clears the
latch, so the *first* `Mediach` call after a swap returns 2; the
*second* immediate call returns 0.

### 3. Hard-disk drives

ACSI/IDE/SCSI drives typically return 0 always (no removable
media) — except SCSI removable media (Syquest, ZIP) which
implement TUR (Test Unit Ready) and report 2 on cartridge swap.

### 4. Drive out of range

`drv >= BLKDEVNUM`: returns 0 (per `blkdev_mediach` defensive
default — pretends no change so callers don't panic).

### 5. The "media-change protocol" contract

GEMDOS file ops are **required** by Atari spec to call `Mediach`
before any FAT-modifying write. `Rwabs` itself does not auto-check.
Failure to honour this protocol results in disk corruption when
users hot-swap floppies. EmuTOS GEMDOS does honour it; third-party
direct-`Rwabs` callers may not.

### 6. Cached state

The driver tracks a per-drive "media changed" flag in
`blkdev[].mediachange`. `Mediach` reads-and-clears this flag for
the "definitely changed" path. Re-reading boot sector via
`Getbpb` (which calls Mediach internally) consumes the flag.

## Dependencies

- `hdv_mediach` vector.
- Per-drive `blkdev[].mediachange` state.

Used by: GEMDOS file write paths (mandatory pre-flight check),
`Getbpb` (re-validates BPB on change).

## Cross-references

- **Compendium:** "Mediach" — agrees on 3-state return.
- **Profibuch:** "Mediach (0x09)" — agrees.
- `docs/api/bios/04-rwabs.md`
- `docs/api/bios/07-getbpb.md`

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

**PHASE-04**: SD-card hot-swap detection on RPi4 is via the
SDHOST card-detect pin (CD line). USB-MSC removability comes from
udev-style hot-plug events on the USB stack. Both feed into a
per-drive `mediachange` flag the NewTOS Mediach trap reads and
clears.
