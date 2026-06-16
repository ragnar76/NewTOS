# `Fdatime` — GEMDOS 0x57

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsdir.c:684` (`xgsdtof`).

## EmuTOS source

```c
long xgsdtof(DOSTIME *buf, int h, int wrt)
{
    OFD *f = getofd(h);
    if (!f) return EIHNDL;

    DFD *dfd = f->o_dfd;

    if (wrt) {
        swpcopyw(&buf->time, &dfd->o_td.time);
        swpcopyw(&buf->date, &dfd->o_td.date);
        dfd->o_flag |= O_DIRTY;
    } else {
        swpcopyw(&dfd->o_td.time, &buf->time);
        swpcopyw(&dfd->o_td.date, &buf->date);
    }
    return E_OK;
}
```

`swpcopyw` byte-swaps a WORD on copy — the on-disk DFD format is
**little-endian** (FAT convention) but the OS-side `DOSTIME`
buffer is **big-endian** (m68k native). The copy direction
determines who's the source.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0057` |
| WORD args | 4 (LONG + WORD + WORD) |
| Stack frame | `+8: wrt (W)`, `+6: h (W)`, `+2: buf (L)`, `+0: 0x0057` |

## Inputs

| Pos | Name  | Type      | Range / meaning                              |
|-----|-------|-----------|----------------------------------------------|
| 0   | `buf` | DOSTIME * | source (set) or dest (get) for time/date      |
| 1   | `h`   | WORD      | open file handle                              |
| 2   | `wrt` | WORD      | 0 = get, 1 = set                              |

### `DOSTIME` structure (4 bytes, big-endian on m68k)

| Offset | Field  | Encoding (FAT/DOS-time)                              |
|-------:|--------|------------------------------------------------------|
| 0      | `time` | bits 15..11 hour (0..23), 10..5 min (0..59), 4..0 sec/2 (0..29) |
| 2      | `date` | bits 15..9 year-1980 (0..119), 8..5 month (1..12), 4..0 day (1..31) |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: 0 success, negative error |

`buf` is populated on get; left unchanged on set.

## Errors

| Code   | Value | When                |
|--------|------:|---------------------|
| EIHNDL | -37   | invalid handle       |

## Edge cases

### 1. Endianness asymmetry

The DFD stores `o_td.time` / `o_td.date` in **little-endian** (FAT
on-disk format). The userland `DOSTIME` buffer is **big-endian**
(host m68k order). `swpcopyw` swaps every direction. On NewTOS-
AArch64 this becomes "FAT little-endian ↔ TOS-ABI big-endian" —
the swap is preserved for ABI compat regardless of host
endianness.

### 2. O_DIRTY flag triggers flush on close

Setting time/date marks the DFD dirty so `Fclose` writes the new
values back to the directory entry. Without `O_DIRTY` the change
exists only in memory.

### 3. Read does NOT mark dirty

Reading does not touch `O_DIRTY`. A subsequent close after a
read-only sequence does not rewrite the directory.

### 4. Time = 2-second resolution

The DOS-time format encodes seconds in 2-second steps (5 bits =
0..29). Programs setting odd seconds see the value silently
rounded down.

### 5. Year cap at 2107

7-bit year-1980 → max 1980+127 = 2107. After that, dates wrap.
Real-world programs often saturate at 2079 (older TOS conventions).

### 6. No path version

Unlike POSIX `utime`, `Fdatime` operates on an **open handle**
only. To touch a closed file, programs must `Fopen` it first
(read-mode is enough; the dirty-flush only happens on close).

### 7. Folder timestamps

Folders are usually opened via internal mechanisms (no public
syscall to `Fopen` a folder). Setting folder timestamps via
`Fdatime` is therefore not directly possible.

### 8. EmuTOS bug fix M01.01.0918.01

The original Atari TOS forgot the `O_DIRTY` set on write — changes
were lost on close. EmuTOS adds it. NewTOS keeps the EmuTOS fix.

## Dependencies

- `getofd`, OFD `o_dfd`, DFD `o_td`/`o_flag`.
- `swpcopyw` (byte-swap macro).

Used by: file managers, build tools, archivers preserving
timestamps.

## Cross-references

- **Compendium:** "Fdatime" — agrees on encoding.
- **Profibuch:** "Fdatime (0x57)" — agrees.
- `docs/api/gemdos/time/2c-tgetdate.md` (analogue for system clock).

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: byte-swap is preserved as ABI requirement (TOS programs
expect big-endian `DOSTIME`). On AArch64 host order, NewTOS
implements `swpcopyw` as explicit byte-swap.
