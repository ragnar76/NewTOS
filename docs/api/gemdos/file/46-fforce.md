# `Fforce` — GEMDOS 0x46

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fshand.c:32` (`xforce`), helper `ixforce` at `:42`.

## EmuTOS source

```c
long xforce(int std, int h)
{
    return ixforce(std, h, run);
}

long ixforce(int std, int h, PD *p)
{
    if (std < 0 || std >= NUMSTD) return EIHNDL;

    if (h < 0)                      /* BIOS char dev */
        p->p_uft[std] = h;
    else {
        if (h < NUMSTD) return EIHNDL;
        if (!getofd(h)) return EIHNDL;

        long fh = (long) sft[h - NUMSTD].f_ofd;
        if (fh < 0L)                /* mapped to BIOS dev */
            p->p_uft[std] = fh;
        else {
            p->p_uft[std] = h;
            sft[h - NUMSTD].f_use++;
        }
    }
    return E_OK;
}
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0046` |
| WORD args | 2 (WORD + WORD) |
| Stack frame | `+4: h (W)`, `+2: std (W)`, `+0: 0x0046` |

## Inputs

| Pos | Name  | Type | Range / meaning                                  |
|-----|-------|------|--------------------------------------------------|
| 0   | `std` | WORD | standard handle 0..5 (target slot)                |
| 1   | `h`   | WORD | source handle: `<0` BIOS dev, or `≥6` non-std     |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: 0 success, negative error |

## Errors

| Code   | Value | When                                                  |
|--------|------:|-------------------------------------------------------|
| EIHNDL | -37   | `std < 0` or `std >= NUMSTD`; `h` in `[0, NUMSTD)`; `h ≥ NUMSTD` but no OFD |

## Edge cases

### 1. Cannot Fforce to another standard handle

`h < NUMSTD` is rejected. To redirect stdout to stdin, programs
must `Fdup(0)` first to get a non-std handle.

### 2. BIOS-handle pass-through

When `h < 0`, the negative BIOS handle is stored verbatim. Reads/
writes on the std handle become BIOS char-device ops.

### 3. Refcount bump

For non-std file handles, `sft[h-NUMSTD].f_use++` ensures the OFD
survives even if the user closes the source handle later. Closing
the std handle (which reverts to default and then closes the
underlying) decrements again.

### 4. BIOS-mapped non-std handle

If `h ≥ 6` but the SFT entry's `f_ofd` is negative (the entry was
itself created via `Fforce` from a BIOS dev), the negative value
is propagated and **`f_use` is NOT incremented**. Result: the std
handle now refers to the same BIOS dev, no refcount inflation.

### 5. No-op on already-mapped

Passing the current mapping silently bumps refcount. Programs
calling `Fforce(0, my_handle)` twice end up with `f_use = 3`,
needing two `Fclose` to balance.

### 6. Per-process

`p_uft[]` is per-PD. `Fforce` only affects the calling process.
`Pexec` children inherit the parent's mappings at creation; later
changes diverge.

### 7. `ixforce` exposed for AES

The 3-arg `ixforce(std, h, pd)` is called by AES to set up a
child process's I/O before launching it. Not exposed to user
programs directly.

## Dependencies

- `run->p_uft[]`, `sft[]`, `getofd`.
- `NUMSTD`, `NUMHANDLES`.

Used by: shells, AES (process launcher), redirection.

## Cross-references

- **Compendium:** "Fforce" — agrees.
- **Profibuch:** "Fforce (0x46)" — agrees.
- `docs/api/gemdos/file/45-fdup.md`, `3e-fclose.md`.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-06: process-launch path uses `ixforce` to wire stdin/stdout/
stderr of the child. The refcount semantics (especially the
non-bump on BIOS-mapped non-std) are preserved.
