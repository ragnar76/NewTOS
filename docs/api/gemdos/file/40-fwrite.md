# `Fwrite` — GEMDOS 0x40

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsio.c:528` (`xwrite`), helper `ixwrite` at `:558`.

## EmuTOS source

```c
long xwrite(int h, long len, void *ubufr)
{
    OFD *p = getofd(h);
    if (!p) return EIHNDL;
    return ixwrite(p, len, ubufr);
}

long ixwrite(OFD *p, long len, void *ubufr)
{
    return xrw(1, p, len, ubufr);
}
```

`xrw(1, ...)` walks clusters, allocating new ones via `nextcl`
when writing past EOF, and writes through the buffer cache.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0040` |
| WORD args | 5 (WORD + LONG + LONG) |
| Stack frame | `+8: ubufr (L)`, `+4: len (L)`, `+2: h (W)`, `+0: 0x0040` |
| Dispatcher flag | `0x82` |

## Inputs

| Pos | Name    | Type    | Range / meaning                          |
|-----|---------|---------|------------------------------------------|
| 0   | `h`     | WORD    | file handle (≥ 0)                        |
| 1   | `len`   | LONG    | bytes to write                           |
| 2   | `ubufr` | void *  | source buffer                             |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: bytes actually written (0..`len`), or negative error |

`ret < len` indicates a short write — typically disk-full.
Programs should compare `ret == len` and treat anything else as a
soft error.

## Errors

| Code        | Value | When                                                |
|-------------|------:|-----------------------------------------------------|
| EIHNDL      | -37   | invalid handle                                       |
| BIOS errors | var   | disk write failures via `longjmp`                    |

`xrw` does not return ENSMEM for "disk full"; instead it returns a
short count.

## Edge cases

### 1. `len <= 0`

Returns 0 with no side effects.

### 2. Write extends file

If `o_bytnum + len > o_fileln`, `xrw` calls `nextcl(p, 1)` to grow
the file by allocating new clusters. The OFD `o_fileln` is updated
in memory; the directory entry (`f_fileln` in FCB) is updated only
on `Fclose` or buffer flush.

### 3. Write to read-only handle

Atari TOS allows it (per source comment "we used to disallow
writes to an existing file opened as read-only, but this is not
compatible with Atari TOS"). Mode bits are not enforced.

### 4. Disk full (short write)

When `nextcl` cannot allocate a cluster, `xrw` stops and returns
the partial count. The caller must check `ret == len` for full
success. Subsequent writes return 0.

### 5. Read-only file system / write-protected disk

Returns BIOS-level error (typically EWRPRO -13 or EWRITF -10) via
the longjmp path before any data is written.

### 6. Buffer-cache eviction

Long writes flush dirty cache entries asynchronously. A power loss
mid-write produces a torn FAT — see `docs/api/_dependencies.md`
for the recovery model. NewTOS plans optional sync-after-write.

### 7. Atari "0-byte truncate" idiom

Some programs `Fopen` then `Fwrite` 0 bytes after `Fseek` to
truncate. EmuTOS does **not** truncate on zero-byte write; the
proper truncate is `Fseek(0, h, 0)` + `Fwrite(0)` followed by
modifying `o_fileln` directly — actually, EmuTOS has no truncate
syscall. Programs achieve truncation by `Fcreate` (which deletes
+ recreates) or by writing zeros and updating `Fdatime` manually.

### 8. Concurrent writes

Same caveat as `Fread` — no locking. Programs sharing a handle via
`Fdup` must serialise externally.

## Dependencies

- `getofd`, `xrw`, `nextcl`.
- Buffer cache (`getrec`, `getbcb`, `flush`).
- FAT writer.

Used by: every program that writes files.

## Cross-references

- **Compendium:** "Fwrite" — agrees.
- **Profibuch:** "Fwrite (0x40)" — agrees, notes short-write
  signals disk-full.
- `docs/api/gemdos/file/3f-fread.md`, `42-fseek.md`,
  `3e-fclose.md`.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: write path goes through the FAT driver with explicit
cluster allocation. NewTOS may add an `Fsync`-equivalent XBIOS
extension; existing programs continue to rely on `Fclose` for
flush.
