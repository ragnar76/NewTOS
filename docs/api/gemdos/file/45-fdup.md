# `Fdup` — GEMDOS 0x45

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fshand.c:93` (`xdup`).

## EmuTOS source

```c
long xdup(int h)
{
    if (h < 0 || h >= NUMSTD)
        return EIHNDL;                /* only dup standard handles */

    int i;
    for (i = 0; i < OPNFILES; i++)
        if (!sft[i].f_own) break;
    if (i == OPNFILES) return ENHNDL;

    sft[i].f_own = run;

    /* if the std handle currently points at a non-BIOS handle,
       inherit the OFD pointer; otherwise copy the BIOS handle */
    int mapped = run->p_uft[h];
    if (mapped > 0)
        sft[i].f_ofd = sft[mapped - NUMSTD].f_ofd;
    else
        sft[i].f_ofd = (OFD *)(long)mapped;

    sft[i].f_use = 1;
    return i + NUMSTD;
}
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0045` |
| WORD args | 1 |
| Stack frame | `+2: h (W)`, `+0: 0x0045` |

## Inputs

| Pos | Name | Type | Range / meaning                  |
|-----|------|------|----------------------------------|
| 0   | `h`  | WORD | standard handle 0..5 (NUMSTD=6)   |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: new handle ≥ 6, or negative error |

## Errors

| Code   | Value | When                                            |
|--------|------:|-------------------------------------------------|
| EIHNDL | -37   | `h < 0` or `h >= NUMSTD`                         |
| ENHNDL | -35   | SFT exhausted                                    |

## Edge cases

### 1. **Only standard handles** can be Fdup'd

Unlike POSIX `dup`, `Fdup` rejects non-standard handles. The
intent is to capture the current mapping of stdin/stdout/etc into
a stable non-std handle so it can be saved before redirection.

Idiom: `int saved = Fdup(1); Fforce(1, devhandle); ...; Fforce(1,
saved); Fclose(saved);`

### 2. New handle has refcount 1

`f_use = 1` means a single `Fclose` releases it. Subsequent
`Fdup` on the same standard handle creates *another* independent
reference, not aliased — `f_use` does not increment the original.

### 3. Inherits BIOS-handle vs OFD pointer

If `p_uft[h]` is negative (BIOS char dev), the new SFT entry
stores the negative value verbatim in `f_ofd`. Reads/writes go to
the BIOS device. If `p_uft[h]` is positive (≥ 6, mapped to file),
the OFD pointer is copied — both handles share `o_bytnum` etc.

### 4. Sharing OFD does not increment any refcount

The original SFT entry's `f_use` is not bumped. Closing the
original (via `Fforce` revert + `Fclose`) frees the OFD even if
the duplicate still references it — leading to a dangling pointer.
This is a documented TOS quirk; well-behaved programs close the
duplicate first.

### 5. NUMSTD = 6, OPNFILES depends on configuration

EmuTOS default `OPNFILES = NUMHANDLES - NUMSTD = 40 - 6 = 34`
non-standard handles globally. `ENHNDL` is plausible on
multi-program AES sessions.

### 6. `f_own = run` ties to current process

Process termination scans `sft[]` for `f_own == run` and closes
those entries. The duplicate is auto-closed.

### 7. Aliasing two standard handles

`int dup1 = Fdup(1); Fforce(2, dup1); Fclose(dup1);` redirects
stdout-handle-2 to whatever stdout-handle-1 was pointing to,
correctly preserving the underlying mapping. This is the standard
"save stderr through redirected stdout" idiom.

## Dependencies

- `run->p_uft[]`, `sft[]`.
- `OPNFILES`, `NUMSTD`, `NUMHANDLES` (`bdos/fs.h`).

Used by: AES, command shells, programs implementing redirection.

## Cross-references

- **Compendium:** "Fdup" — agrees on "standard-only" restriction.
- **Profibuch:** "Fdup (0x45)" — agrees, gives the save/redirect
  idiom.
- `docs/api/gemdos/file/46-fforce.md`, `3e-fclose.md`.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: identical SFT model. The dangling-OFD-after-original-
close quirk is preserved (intentional TOS bug-compat).
