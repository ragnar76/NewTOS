# `Fread` — GEMDOS 0x3F

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsio.c:440` (`xread`), helper `ixread` at `:512`.

## EmuTOS source

```c
long xread(int h, long len, void *ubufr)
{
    OFD *p = getofd(h);
    if (!p) return EIHNDL;
    return ixread(p, len, ubufr);
}

long ixread(OFD *p, long len, void *ubufr)
{
    long maxlen = p->o_dfd->o_fileln - p->o_bytnum;
    if (len > maxlen) len = maxlen;       /* truncate at EOF */
    if (len > 0)  return xrw(0, p, len, ubufr);
    return 0L;
}
```

`xrw(0, ...)` is the unified read/write routine that walks
clusters and copies through the buffer cache.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x003F` |
| WORD args | 5 (WORD + LONG + LONG) |
| Stack frame | `+8: ubufr (L)`, `+4: len (L)`, `+2: h (W)`, `+0: 0x003F` |
| Dispatcher flag | `0x82` (special arg pattern in `bdosmain.c`) |

## Inputs

| Pos | Name    | Type    | Range / meaning                                |
|-----|---------|---------|------------------------------------------------|
| 0   | `h`     | WORD    | file handle (≥ 0)                              |
| 1   | `len`   | LONG    | bytes to read                                  |
| 2   | `ubufr` | void *  | destination buffer in user space                |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: bytes actually read (0..`len`), or negative error |

The "actually read" value can be less than `len` (EOF) without
being an error — programs detect EOF by `ret < len` or `ret == 0`.

## Errors

| Code   | Value | When                                              |
|--------|------:|---------------------------------------------------|
| EIHNDL | -37   | `getofd(h)` returned NULL (invalid handle)         |
| BIOS errors | various | propagated from buffer-cache disk reads      |

`ixread` does NOT distinguish EOF from "read 0 bytes" — caller
sees `0` either way.

## Edge cases

### 1. `len <= 0`

`len = 0` returns 0 immediately. Negative `len` is silently treated
as 0 (the `if (len > 0)` branch).

### 2. Read past EOF

Truncates to `o_fileln - o_bytnum`. Reading at EOF returns 0.
File pointer (`o_bytnum`) is left at file end after partial read.

### 3. Read from write-only handle

Atari TOS allows it (originally EmuTOS rejected, then matched TOS
per the source comment "we used to disallow reads from a file
opened as write-only, but this is not compatible with Atari TOS").
The mode bits in the OFD are recorded but not enforced for reads.

### 4. Cross-cluster read

`xrw` chains through the FAT, reading whole clusters into the
buffer cache and copying to user space. Crossing a cluster
boundary is transparent.

### 5. Buffer alignment

No alignment requirement on `ubufr`. The buffer cache uses its own
aligned record buffers; user-space copy is byte-wise via memcpy.

### 6. Concurrent reads

No locking. Two processes reading the same handle through
`Fdup`/`Fforce` share the OFD's `o_bytnum` — interleaved reads
will corrupt each other's positions. This is by design (TOS is
single-tasking; concurrent access is the program's problem).

### 7. BIOS character devices

If `h < 0` (BIOS char dev), `getofd` returns NULL and EIHNDL is
raised. Character device input goes through `Cconin` / `Cnecin`
/ `Bconin`, not `Fread`.

### 8. EmuTOS Mint extension

When `h == -1`, EmuTOS-MiNT redirects to `Bconin` loop. EmuTOS
without MiNT does NOT — `Fread(-1, ...)` returns EIHNDL.

## Dependencies

- `getofd`, `xrw`, buffer cache (`getrec`, `getbcb`).
- OFD `o_bytnum`, `o_curcl`, `o_curbyt`.

Used by: every program that reads files.

## Cross-references

- **Compendium:** "Fread" — agrees on semantics.
- **Profibuch:** "Fread (0x3F)" — agrees, notes EOF == 0 return.
- `docs/api/gemdos/file/40-fwrite.md`, `42-fseek.md`.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: backed by the FAT driver. The user/kernel boundary on
AArch64 is checked once at the trap entry (page-table valid +
writable for the whole `[ubufr, ubufr+len)` range).
