# `Fclose` — GEMDOS 0x3E

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsopnclo.c:426` (`xclose`), helper `ixclose` at `:512`.

## EmuTOS source (essence)

```c
long xclose(int h)
{
    if (h < 0) return E_OK;            /* BIOS character device */
    if (h >= NUMHANDLES) return EIHNDL;

    if (h < NUMSTD) {
        int h0 = run->p_uft[h];
        run->p_uft[h] = get_default_handle(h);
        h = h0;
        if (h < 0) return E_OK;
        if (h < NUMSTD) return EIHNDL;  /* "can't happen" */
    } else {
        FTAB *ftab = &sft[h - NUMSTD];
        if ((long)ftab->f_ofd < 0L) {   /* mapped to char dev */
            if (--ftab->f_use == 0) { ftab->f_ofd = 0; ftab->f_own = 0; }
            return E_OK;
        }
    }

    OFD *fd = getofd(h);
    if (!fd) return EIHNDL;

    long rc = ixclose(fd, 0);
    FTAB *ftab = &sft[h - NUMSTD];
    if (--ftab->f_use == 0) sftdel(ftab);
    return rc;
}
```

`ixclose` flushes the FCB (date/time, start cluster, length) and
calls the buffer cache to write back dirty blocks.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x003E` |
| WORD args | 1 |
| Stack frame | `+2: h (W)`, `+0: 0x003E` |

## Inputs

| Pos | Name | Type | Range / meaning                                       |
|-----|------|------|-------------------------------------------------------|
| 0   | `h`  | WORD | file handle: `<0` BIOS char dev, `0..5` std, `6..` SFT |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: 0 success, negative error |

## Errors

| Code   | Value | When                                                |
|--------|------:|-----------------------------------------------------|
| EIHNDL | -37   | `h >= NUMHANDLES`, or std-handle remapped to another std (corrupted Fforce), or non-std handle has no OFD |
| EINTRN | -65   | `ixclose` flush failed (buffer cache write error)    |

Bios-level disk errors propagate via `longjmp` and are surfaced as
their original BIOS error code by the trap handler.

## Edge cases

### 1. `h < 0` is a no-op

Negative handles refer to raw BIOS character devices (e.g. `-1`
console aux); they have no OFD and `Fclose` succeeds without doing
anything. AES uses this for `Fclose(-1)` clean-up.

### 2. Standard handle (0..5) revert

Closing a standard handle reverts `p_uft[h]` to the boot-default
mapping and **does not free** the underlying file. Any subsequent
read/write on the standard handle hits the default device. The
underlying handle (returned via `h0`) is then closed if non-std.

### 3. Reference counting

`ftab->f_use` starts at 1 on `opnfil` and is incremented by `Fdup`
and the Fforce-via-non-std-mapped path. Only when it reaches 0 is
the OFD freed (`sftdel`). Closing a duplicate decrements without
flushing.

### 4. Dirty-flush ordering

`ixclose` writes:
1. FCB time/date/cluster/length back to the directory entry.
2. All dirty buffer cache entries for this OFD.
3. Frees OFD memory.

Step 1 must precede step 2 because the directory entry sits in the
cache too; flush order matters for crash consistency.

### 5. CL_DIR / CL_FULL flags (internal)

`ixclose(fd, CL_DIR)` flushes the dir entry only (used by
`Fcreate`'s partial close). `CL_FULL` forces all buffers. The
public `Fclose` always uses flag 0 = full normal close.

### 6. Process termination cascade

`Pterm`/`Pterm0` calls `Fclose` on every handle owned by the
process via `sft[].f_own == run`. Standard handles inherited via
Fforce are restored to defaults.

### 7. EIHNDL distinguishes errors

A "can't happen" path returns EIHNDL when a standard handle is
mapped to another standard handle (would only occur if Fforce was
called incorrectly). Programs should treat any EIHNDL as fatal.

## Dependencies

- `getofd`, `ixclose`, `sftdel`, `get_default_handle`.
- `run->p_uft[]`, global `sft[]`.
- Buffer cache, FAT writer.

Used by: every program that called `Fopen`/`Fcreate`/`Fdup`.

## Cross-references

- **Compendium:** "Fclose" — agrees.
- **Profibuch:** "Fclose (0x3E)" — agrees.
- `docs/api/gemdos/file/3d-fopen.md`, `45-fdup.md`, `46-fforce.md`.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: identical SFT and PD `p_uft[]` semantics; flush calls
into the FAT writer with the same ordering invariants.
