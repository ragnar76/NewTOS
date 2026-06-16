# `Getbpb` — BIOS 0x07

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/bios.c:1117` (`getbpb`).

## EmuTOS source

```c
LONG getbpb(WORD drive)
{
    return protect_w(hdv_bpb, drive);
}
```

`hdv_bpb` is the OS-variable function pointer at addr `0x472`. It
is set at boot to `blkdev_getbpb()` (`bios/blkdev.c`), which:

1. Checks media change via `hdv_mediach`.
2. Reads boot sector via `hdv_rw`.
3. Parses the boot record into a static `BPB` struct
   (one per drive, in `blkdev[].bpb`).
4. Returns a pointer to that struct (or `NULL` on error).

## Trap interface

| | |
|--|--|
| Trap | 13 (BIOS) |
| Function ID | `0x07` |
| WORD args | 1 (WORD) |
| Stack frame | `+2: drive (W)`, `+0: 0x0007` |
| Vector entry | `bios_vecs[7]` |

## Inputs

| Pos | Name    | Type | Range / meaning                              |
|-----|---------|------|----------------------------------------------|
| 0   | `drive` | WORD | physical drive index (0=A:, …, 15=P:)         |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (`BPB *` or `NULL`) |

Returns a pointer to the drive's 18-byte BPB
(see `docs/api/structures/bpb.md`). The pointer is into a
**BIOS-owned static buffer** — caller must not free or modify it
beyond the documented mutable fields.

| Case                                       | Return       |
|--------------------------------------------|--------------|
| Boot sector readable, BPB parsed           | `BPB *` (non-NULL) |
| Drive offline / media not present          | `0L` (NULL)  |
| Boot sector unreadable                     | `0L` (NULL)  |
| Unsupported FAT (e.g. exFAT)               | `0L` (NULL)  |

## Errors

`Getbpb` returns `NULL` (`0L`) on any failure — there is **no
error code**. To distinguish "drive offline" from "unreadable
boot sector" the caller must consult `mediach()` separately.

## Edge cases

### 1. Pointer aliasing

The BPB buffer is shared across calls — calling `Getbpb(0)` then
`Getbpb(1)` returns two distinct pointers (one per drive),
**both** of which remain valid concurrently. EmuTOS keeps one BPB
struct per drive in `blkdev[]`.

### 2. Re-read on media change

`Getbpb` always re-runs the boot-sector read; it does not cache.
This is by design — media change must invalidate the cached BPB.

### 3. Side effect: GEMDOS dirty cache flush

`blkdev_getbpb` also flushes any GEMDOS in-memory FAT cache for
that drive when it detects media change. Programs that intend to
"check if media changed" should NOT call `Getbpb` lightly — they
should use `Mediach` instead.

### 4. Atari boot-sector fields

The on-disk boot sector has many fields; `Getbpb` returns a
**reduced** 18-byte BPB. The full boot sector (with serial
number, reserved sectors, etc.) is not exposed via this call.

### 5. `drive` out of range

Drives ≥ `BLKDEVNUM` (16): `blkdev_getbpb` returns NULL.

### 6. FAT12 vs FAT16

The BPB's `b_flags` field bit 0 (`B_16`) distinguishes 12-bit and
16-bit FATs. The threshold is 4085 clusters per Microsoft's
algorithm. EmuTOS correctly classifies this. Larger FATs (FAT32,
exFAT) are not supported.

### 7. UWORD `recsiz`/`numcl`

The BPB uses 16-bit fields throughout. Maximum representable
partition size is 2 GiB (UWORD numcl × cluster size). Larger
partitions will either return NULL or report truncated values —
both are observable, neither is handled gracefully.

## Dependencies

- `hdv_bpb` vector (addr `0x472`).
- `hdv_mediach`, `hdv_rw` (transitively).
- `blkdev[].bpb` static array.

Used by: GEMDOS `flopopn`/disk-mount logic, AES drive enumerator,
BPB-aware utilities (`SHOWBPB.PRG`).

## Cross-references

- **Compendium:** "Getbpb" — agrees on NULL-on-failure.
- **Profibuch:** "Getbpb (0x07)" — agrees, notes that return
  pointer survives across calls.
- `docs/api/structures/bpb.md`
- `docs/api/bios/04-rwabs.md`
- `docs/api/bios/09-mediach.md`

## Configuration

- `BLKDEVNUM = 16`.
- FAT support implicit; no toggles.

## NewTOS implementation notes

(Empty during Phase 00.)

**PHASE-04**: NewTOS implements FAT12/FAT16 only, matching EmuTOS.
A separate FAT32 path (for SD-card boot) lives parallel to the
classic-BPB path. `Getbpb` will return a pointer to a
NewTOS-managed `BPB` only for FAT12/FAT16 partitions; FAT32
partitions return NULL (caller must use a NewTOS-extension call).
