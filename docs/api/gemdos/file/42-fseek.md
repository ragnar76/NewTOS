# `Fseek` — GEMDOS 0x42

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsio.c:321` (`xlseek`), helper `ixlseek` at `:355`.

## EmuTOS source

```c
long xlseek(long n, int h, int flg)
{
    OFD *f = getofd(h);
    if (!f) return EIHNDL;

    if (flg == 2)      n += f->o_dfd->o_fileln;   /* SEEK_END */
    else if (flg == 1) n += f->o_bytnum;          /* SEEK_CUR */
    else if (flg)      return EINVFN;             /* unknown */
    /* flg == 0: SEEK_SET, n is absolute */

    return ixlseek(f, n);
}
```

`ixlseek` walks the FAT cluster chain to position `n`. Returns
ERANGE for out-of-bounds and EINTRN if the chain is shorter than
`o_fileln` claims.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0042` |
| WORD args | 4 (LONG + WORD + WORD) |
| Stack frame | `+6: flg (W)`, `+4: h (W)`, `+0: offset (L)` then `0x0042` at `+0` |
| Dispatcher flag | `0x81` (special arg pattern) |

The "special" pattern means the LONG argument is **first** (right
after the function code), not last. Compendium argument order is
`(offset, h, flg)`.

## Inputs

| Pos | Name  | Type | Range / meaning                                   |
|-----|-------|------|---------------------------------------------------|
| 0   | `n`   | LONG | offset in bytes                                    |
| 1   | `h`   | WORD | file handle                                        |
| 2   | `flg` | WORD | 0 SEEK_SET, 1 SEEK_CUR, 2 SEEK_END                  |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: new absolute position, or negative error |

## Errors

| Code   | Value | When                                                  |
|--------|------:|-------------------------------------------------------|
| EIHNDL | -37   | invalid handle                                         |
| EINVFN | -32   | `flg` not in {0, 1, 2}                                 |
| ERANGE | -64   | resulting position is `< 0` or `> o_fileln`             |
| EINTRN | -65   | FAT chain shorter than `o_fileln` (filesystem corruption) |

## Edge cases

### 1. `flg = 2` past end is invalid

`SEEK_END` with positive `n` would seek past EOF; `ixlseek`
rejects this with ERANGE. To extend a file, use `Fwrite` not
`Fseek`. (Some POSIX-style code expects holes — TOS has no
sparse-file support.)

### 2. `flg = 2` with negative `n`

Standard idiom `Fseek(0, h, 2)` returns the file size in D0.
`Fseek(-100, h, 2)` jumps to 100 bytes before EOF.

### 3. `flg = 1` relative

`o_bytnum + n` must be in `[0, o_fileln]`. Negative results return
ERANGE.

### 4. Invalid `flg`

Returns EINVFN, not EINVFL or EARG. Compendium calls this out as
a TOS quirk (POSIX uses EINVAL).

### 5. Seek to cluster boundary

`ixlseek` deliberately positions one cluster *before* a boundary
(the `clnum--` adjustment) so that the next `xrw` call can chain
forward correctly. This is an internal optimisation invisible to
callers.

### 6. Forward-chain optimisation

If the new position is ≥ current and `o_curcl != 0`, `ixlseek`
chains forward from the current cluster instead of starting at
`o_strtcl`. Saves one FAT walk per seek for streaming reads.

### 7. Seek on read-only handle

Allowed. Mode bits are not consulted.

### 8. Seek past EOF on extending file

If a process is mid-write and the OFD's `o_fileln` was just
extended, `Fseek` to the new size succeeds. If the directory
entry (`f_fileln`) hasn't been flushed yet, that's irrelevant —
`Fseek` reads the in-memory `o_fileln`.

## Dependencies

- `getofd`, `getclnum`, `endofchain`, `cl2rec`.
- OFD `o_bytnum`, `o_curcl`, `o_currec`, `o_curbyt`.
- DFD `o_fileln`, `o_strtcl`.
- DMD `m_clblog`, `m_clbm`, `m_clsizb`.

Used by: every program that does random-access I/O.

## Cross-references

- **Compendium:** "Fseek" — agrees on `flg` values.
- **Profibuch:** "Fseek (0x42)" — agrees.
- `docs/api/gemdos/file/3f-fread.md`, `40-fwrite.md`.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: same OFD model. The `clnum--` cluster-boundary trick is
preserved verbatim because it ties into the `xrw` chaining
behaviour — diverging would break read/write cluster transitions.
