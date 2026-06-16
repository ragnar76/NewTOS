# `Rwabs` — BIOS 0x04

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/bios.c:1045` (`lrwabs`).

## EmuTOS source

```c
LONG lrwabs(WORD r_w, UBYTE *adr, WORD numb, WORD first, WORD drive,
            LONG lfirst)
{
    return protect_wlwwwl((PFLONG)hdv_rw, r_w, (LONG)adr, numb, first,
                          drive, lfirst);
}
```

`hdv_rw` is a function pointer in the OS variables area
(`tosvars.S:hdv_rw`), addr `0x47E`. It is set by the BIOS at boot
to the active block-device driver's read/write entry — typically
`blkdev_rwabs()` (`bios/blkdev.c`).

`protect_wlwwwl` (`util/miscasm.S`) preserves d2/a2 across the asm
driver call and marshals the C-style argument list into the
register / stack convention the driver expects.

## Trap interface

| | |
|--|--|
| Trap | 13 (BIOS) |
| Function ID | `0x04` |
| WORD args | 8 (W + L + W + W + W + L) |
| Stack frame | `+12: lfirst (LONG)`, `+10: drive (W)`, `+8: first (W)`, `+6: numb (W)`, `+2: adr (LONG)`, `+0: 0x0004 r_w-merged` — see Compendium for exact layout |
| Vector entry | `bios_vecs[4]` |

The classic TOS frame is: `r_w` (W) + `adr` (L) + `numb` (W) +
`first` (W) + `drive` (W). The `lfirst` (LONG) was added in TOS
1.04 for sector numbers > 65535. Atari's convention: pass `first =
-1` and put the real sector number in `lfirst`. EmuTOS always
forwards both.

## Inputs

| Pos | Name     | Type   | Range / meaning                              |
|-----|----------|--------|----------------------------------------------|
| 0   | `r_w`    | WORD   | bit 0: 0=read, 1=write; bit 3: don't retry; bit 4: don't translate |
| 1   | `adr`    | UBYTE* | buffer (must be word-aligned for floppy DMA) |
| 2   | `numb`   | WORD   | sector count                                 |
| 3   | `first`  | WORD   | starting sector # (use `-1` if `lfirst` valid) |
| 4   | `drive`  | WORD   | physical drive index (0=A:, 1=B:, 2=C:, …)   |
| 5   | `lfirst` | LONG   | extended starting sector (TOS ≥ 1.04)        |

### `r_w` flags

| Bit | Constant     | Meaning                                       |
|-----|--------------|-----------------------------------------------|
| 0   | `RW_WRITE`   | 0 ⇒ read, 1 ⇒ write                           |
| 1   | `RW_NOMEDIACH` | suppress media-change detection             |
| 2   | `RW_NORETRY` | abort on first error (don't retry)            |
| 3   | `RW_NOTRANS` | physical sector (no logical translation)      |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (BIOS error code) |

| Case               | Return                                         |
|--------------------|------------------------------------------------|
| Success            | `E_OK` (0)                                     |
| Error              | negative BIOS error (see `include/gemerror.h`) |

Common errors: `EUNDEV` (-15) unknown device, `ESECNF` (-8) sector
not found, `EWRPRO` (-13) write-protected, `EWRITF` (-10) write
fault, `EREADF` (-11) read fault, `ECHMED` (-14) media change,
`EBADSF` (-16) bad sectors on format.

## Errors

The driver hooked at `hdv_rw` defines the exact set. The standard
EmuTOS block-device dispatcher returns:

- `EUNDEV` (-15) — `drive` out of range or unmapped.
- `EBADRQ` (-5) — `numb == 0` or invalid command in `r_w`.
- `EWRPRO` (-13) — write attempted on RO drive.
- `EREADF` / `EWRITF` — physical I/O failure.
- `ECHMED` (-14) — `mediach()` reported "definitely changed" since
  last access.

## Edge cases

### 1. `first = -1` LONG-sector convention

When `first == -1`, drivers read the sector number from `lfirst`.
This was Atari's way of extending the API past 65535 sectors
without breaking old callers (which pass meaningful 16-bit `first`
and `lfirst = 0`, so old behaviour is preserved when sector < 65535).

### 2. Physical vs. logical drive numbers

`drive` is **physical** — the BIOS does no GEMDOS-style logical
mapping. The standard floppy-on-physical-0/1 + hard-disk-on-2..15
arrangement is hard-coded.

### 3. Word alignment for floppy DMA

The Atari floppy DMA controller requires word-aligned `adr`.
Passing an odd address to a floppy `Rwabs` historically caused
silent corruption. Hard-disk drivers (ACSI / IDE / SCSI) typically
don't care.

### 4. `numb == 0` is an error

EmuTOS's drivers return `EBADRQ` for zero-count requests; classic
TOS behaviour was undefined.

### 5. Media-change interaction

Programs writing to FAT sectors must check `mediach()` first per
the BIOS contract. The driver does NOT auto-check; a write to a
swapped floppy succeeds and corrupts the new disk's data unless
the caller honoured the media-change protocol.

### 6. `protect_wlwwwl` semantics

`a2`/`d2` saved/restored. Drivers written in 68k-asm rely on the
caller preserving these registers per the Atari `cdecl`-asm
calling convention; the C `protect_*` wrappers exist precisely to
bridge this.

### 7. No buffer-size validation

`adr + numb*sector_size` is not bounds-checked against any process
or system region. Wild buffers crash silently into device memory.

## Dependencies

- `hdv_rw` vector at `0x47E`.
- The active block-device driver (typically `blkdev_rwabs`).

Used by: GEMDOS file I/O (Fread/Fwrite via FAT layer), GEMDOS
directory walks, `Getbpb` indirectly (via media-change), every disk
operation.

## Cross-references

- **Compendium:** "Rwabs" — agrees on signature and `lfirst`.
- **Profibuch:** "Rwabs (0x04)" — agrees.
- `docs/api/bios/07-getbpb.md`
- `docs/api/bios/09-mediach.md`
- `docs/api/bios/_state.md`
- **PHASE-04**: block-storage subsystem, `hdv_rw` design.

## Configuration

- `BLKDEVNUM = 16` (drives A:..P:).

## NewTOS implementation notes

(Empty during Phase 00.)

**PHASE-04 design driver**: `hdv_rw` will be a Rust trait dispatch
into a per-drive `BlockDevice` impl. The MMC/SD-card driver
(`bcm2711::sdhost`) services drives 0..1 (boot SD); USB-MSC drives
2..15. The `r_w` flag bits map cleanly onto a `BlockOp` enum.
Sector size will be 512 throughout (no Atari 512-vs-1024 switch).
