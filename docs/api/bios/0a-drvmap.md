# `Drvmap` — BIOS 0x0A

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/bios.c:1206` (`drvmap`).

## EmuTOS source

```c
LONG drvmap(void) { return blkdev_drvmap(); }
```

`blkdev_drvmap()` (`bios/blkdev.c`) returns a 32-bit bitmap built
from the per-drive `blkdev[i].valid` flags during the boot
device-discovery scan.

## Trap interface

| | |
|--|--|
| Trap | 13 (BIOS) |
| Function ID | `0x0A` |
| WORD args | 0 |
| Stack frame | `+0: 0x000A` |
| Vector entry | `bios_vecs[10]` |

## Inputs

None.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (drive bitmap) |

Bit `n` (n ∈ 0..15) set ⇒ drive at index `n` is online.
Bits 16..31 are zero in EmuTOS (only 16 drives supported).

| Bit | Drive |
|----:|-------|
| 0   | A:    |
| 1   | B:    |
| 2   | C:    |
| …   | …     |
| 15  | P:    |

## Errors

None — function cannot fail.

## Edge cases

### 1. Floppy A: + B: convention

Per the Compendium: "if the BIOS supports logical drives A and B
on a single physical drive, it should return both bits set if a
floppy drive is present." EmuTOS follows this — single-floppy
systems still report bits 0 AND 1 set, so old programs that
expect "drive B: exists" can do their swap-prompt routine.

### 2. Static after boot

The bitmap is computed once during boot-device discovery and not
updated for hot-plug. SD-card insertion *after* boot does not flip
a bit in the EmuTOS bitmap. Atari hardware had no hot-plug
expectation. NewTOS will likely make this dynamic — see notes.

### 3. P: BetaDOS quirk

The fsmain.c comment "BetaDOS expects to be able to select drive
P: even if it does not exist" implies that `Dsetdrv(15)` succeeds
even with bit 15 clear. Drvmap remains the truthful map; clients
must consult it before assuming a drive is usable.

### 4. Boot drive

The boot drive (whatever device the kernel was loaded from) is
always present in the bitmap. On a typical Mega ST/STe with hard
disk, that's bit 2 (C:). On a floppy boot, bits 0 and 1.

### 5. No drive ordering guarantee

The mapping of bits to physical hardware is a BIOS implementation
detail. Floppy 0 → A: is universal; partition layout for hard
disks is per-driver. NewTOS will document its own mapping in
`docs/api/bios/_state.md`.

## Dependencies

- `blkdev[].valid` flags set by `blkdev_init()`.

Used by: `Dsetdrv`/`Dgetdrv` (returned drive map), AES drive
enumeration, shell drive list.

## Cross-references

- **Compendium:** "Drvmap" — agrees.
- **Profibuch:** "Drvmap (0x0A)" — agrees, notes A:+B: convention.
- `docs/api/gemdos/drive/0e-dsetdrv.md`
- `docs/api/bios/_state.md`

## Configuration

- `BLKDEVNUM = 16`.

## NewTOS implementation notes

(Empty during Phase 00.)

**PHASE-04 plan**: SD-card hot-plug will dynamically update the
bitmap. NewTOS programs querying Drvmap repeatedly will see bits
appear/disappear as USB drives are attached. Programs that cache
the map must treat it as a snapshot. A separate Drvmap-changed
event mechanism may be added (NewTOS extension, not exposed
through TOS API).
