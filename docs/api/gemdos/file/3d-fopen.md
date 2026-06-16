# `Fopen` — GEMDOS 0x3D

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsopnclo.c:221` (`xopen`), with internal helper
> `ixopen` at `:243`.

## EmuTOS source

```c
long xopen(char *name, int mod)
{
    return ixopen(name, mod & VALID_FOPEN_BITS);
}

static long ixopen(char *name, int mod)
{
    DND *dn = findit(name, &s, 0);
    if (!dn) return EFILNF;

    long pos = 0;
    FCB *fcb = scan(dn, s, FA_NORM, &pos);
    if (!fcb) return EFILNF;

    /* read-only attribute blocks write opens */
    if ((fcb->f_attrib & FA_RO) && (mod != 0))
        return EACCDN;

    return opnfil(fcb, dn, mod);
}
```

`VALID_FOPEN_BITS = 0x77` (low 3 bits = mode, bits 4..6 = sharing,
bit 3 reserved). `opnfil()` allocates an SFT slot and returns the
handle (≥ 6).

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x003D` |
| WORD args | 3 (LONG + WORD) |
| Stack frame | `+6: mod (W)`, `+2: name (L)`, `+0: 0x003D` |

## Inputs

| Pos | Name   | Type    | Range / meaning                                 |
|-----|--------|---------|-------------------------------------------------|
| 0   | `name` | char *  | path to existing file                            |
| 1   | `mod`  | WORD    | open mode (see below)                            |

### `mod` bit layout

| Bits | Field        | Values                                                        |
|------|--------------|---------------------------------------------------------------|
| 0..2 | access mode  | 0 = read-only, 1 = write-only, 2 = read/write                  |
| 3    | (reserved)   | masked off                                                     |
| 4..6 | sharing mode | 0 compat, 1 deny-rwx, 2 deny-write, 3 deny-read, 4 deny-none   |
| 7+   | (ignored)    |                                                                |

EmuTOS masks `mod & 0x77`; bits 7..15 are silently dropped.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (file handle ≥ 6, or negative error)                       |

Handles 0..5 are reserved (standard handles + Fforce-able BIOS
slots). `Fopen` always returns ≥ 6 for new handles.

## Errors

| Code   | Value | When                                                    |
|--------|------:|---------------------------------------------------------|
| EFILNF | -33   | path component not found, or filename not in directory   |
| EACCDN | -36   | file is read-only and `mod != 0` (write requested)       |
| ENHNDL | -35   | system file table full (`opnfil` ran out of SFT entries)  |
| EPTHNF | -34   | path resolution failed before reaching the filename      |

## Edge cases

### 1. Read-only access to RO files

`mod = 0` succeeds on FA_RO files; only write modes are blocked.

### 2. Sharing mode is recorded, not enforced (Atari TOS)

Atari TOS records the sharing-mode bits in the OFD but does not
actually enforce them across processes. EmuTOS follows this: the
bits travel through `opnfil` into the OFD but no concurrent-open
check runs. NewTOS will preserve the recording, may or may not
enforce — TBD per phase.

### 3. Volume labels and folders

`scan(.., FA_NORM, ..)` skips entries with FA_VOL or FA_SUBDIR.
`Fopen` therefore never returns a handle for a directory or label
— `EFILNF` is returned even if the name exists as a folder.

### 4. Per-process handle table

`p_uft[0..5]` is the per-process standard-handle table; `sft[]` is
global. New handles ≥ 6 index `sft[handle - NUMSTD]`. NUMSTD = 6.

### 5. Findit walks `..`

`findit()` resolves `..` by walking parent DNDs. `..` past root
clamps to root (no error).

### 6. Reset-time inheritance

Open files survive `Pterm`/`Pterm0` only via `Fforce` redirection
of standard handles. Non-standard handles owned by an exiting
process are auto-closed by `proc_term`.

## Dependencies

- `findit`, `scan`, `opnfil`, `ckdrv` (FAT layer).
- `run->p_uft[]` (PD handle table).
- Global `sft[]` (System File Table, `OPNFILES = NUMHANDLES - NUMSTD`).
- `OFD`, `DFD`, `DND`, `FCB`, `DMD` structures.

Used by: every program that reads disk files. AES, BIOS bootstrap,
shell.

## Cross-references

- **Compendium:** "Fopen" — agrees on mode bits and error codes.
- **Profibuch:** "Fopen (0x3D)" — agrees.
- `docs/api/gemdos/file/3c-fcreate.md`, `3e-fclose.md`.
- `docs/api/_dependencies.md` — FAT subsystem.

## Configuration

None — always present.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: backed by the FAT16/32 driver on Pi SD/USB block storage.
The SFT and PD `p_uft[]` model is preserved verbatim — programs see
identical handle numbers and sharing-mode bits.
