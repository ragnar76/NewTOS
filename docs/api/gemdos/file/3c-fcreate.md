# `Fcreate` — GEMDOS 0x3C

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsopnclo.c:81` (`xcreat`), helper `ixcreat` at `:97`.

## EmuTOS source (essence)

```c
long xcreat(char *name, UBYTE attr)
{
    return ixcreat(name, attr & ~FA_SUBDIR);    /* mask off SUBDIR bit */
}

long ixcreat(char *name, UBYTE attr)
{
    DND *dn = findit(name, &s, 0);
    if (!dn) return EPTHNF;
    if (!*s || *s == '.') return EPTHNF;
    if (contains_illegal_characters(s)) return EACCDN;

    if ((attr & FA_VOL) && attr != FA_VOL) return EACCDN;
    if (attr == FA_VOL && dn->d_parent)   return EACCDN;

    /* if file exists: delete unless it's RO/folder */
    if ((fcb = scan(dn, s, FA_NORM|FA_SUBDIR, &pos))) {
        if (attr != FA_VOL) {
            if ((fcb->f_attrib & (FA_SUBDIR|FA_RO)) || attr == FA_SUBDIR)
                return EACCDN;
        }
        if (ixdel(dn, fcb, pos - sizeof(FCB)) < 0)
            return EACCDN;        /* open in another process */
    }

    /* find empty slot, growing dir as needed */
    while (!(fcb = scan(dn, ERASED, 0xff, &pos))) {
        if (!fd->o_dnode) return EACCDN;     /* root cannot grow */
        if (nextcl(fd, 1)) return EACCDN;
        dirinit(dn);
        pos = 0;
    }

    /* write FCB with current_time / current_date, attr, zero size */
    return opnfil(fcb, dn, RW_MODE | (attr & FA_RO ? RO_MODE : 0));
}
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x003C` |
| WORD args | 3 (LONG + WORD) |
| Stack frame | `+6: attr (W)`, `+2: name (L)`, `+0: 0x003C` |

## Inputs

| Pos | Name   | Type    | Range / meaning                                  |
|-----|--------|---------|--------------------------------------------------|
| 0   | `name` | char *  | path of file to create                            |
| 1   | `attr` | WORD    | attribute byte (low 8 bits used)                  |

### `attr` bits

| Bit | Mask  | Meaning             |
|----:|------:|---------------------|
| 0   | 0x01  | FA_RO read-only      |
| 1   | 0x02  | FA_HIDDEN            |
| 2   | 0x04  | FA_SYSTEM            |
| 3   | 0x08  | FA_VOL volume label   |
| 4   | 0x10  | FA_SUBDIR (masked off — `Fcreate` always strips this) |
| 5   | 0x20  | FA_ARCHIVE           |

`Fcreate` cannot create directories; use `Dcreate (0x39)` instead.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG handle (≥ 6) or negative error |

The returned handle is opened **read-write**, even if FA_RO is set
in `attr`. The FA_RO flag applies after `Fclose`.

## Errors

| Code   | Value | When                                                    |
|--------|------:|---------------------------------------------------------|
| EPTHNF | -34   | path not found or filename empty / `.` / `..`            |
| EACCDN | -36   | illegal chars; FA_VOL with other bits; FA_VOL outside root; existing entry is RO/folder; existing file open elsewhere; root directory full and cannot grow |
| ENHNDL | -35   | SFT exhausted (in `opnfil`)                               |

## Edge cases

### 1. Existing file is overwritten

If a non-RO, non-folder file with the same name exists, it is
silently deleted (`ixdel`) and recreated empty. Programs assuming
"create or fail" must check with `Fsfirst` first.

### 2. FA_SUBDIR is silently stripped

`xcreat` masks off the SUBDIR bit. Programs that pass FA_SUBDIR
(`0x10`) get a regular file. Use `Dcreate` for folders.

### 3. Volume label exclusivity

`attr == FA_VOL` (exactly 0x08): creates label in root only;
deletes any existing label first. Any combination FA_VOL +
something-else returns EACCDN.

### 4. Initial timestamp

`f_td.time = current_time` and `f_td.date = current_date` —
populated from the BIOS time on disk write, not on `Fcreate`. The
in-memory OFD timestamp is set; final on-disk value is written by
`Fclose`.

### 5. Initial cluster = 0

`f_clust = 0` and `f_fileln = 0` — no cluster is allocated yet. The
first `Fwrite` triggers cluster allocation via `nextcl()`.

### 6. Illegal filename characters

`contains_illegal_characters` rejects `< > : " / \ | ? * ` and the
control characters. Path separators within `name` are *path*
separators, not filename chars.

### 7. Root directory full

FAT root has a fixed entry count (224 on a 1.44 MB floppy, 512 on
TOS-formatted ST hard disks). When full, `Fcreate` returns EACCDN
because the root cannot grow.

## Dependencies

- `findit`, `scan`, `opnfil`, `ixdel`, `nextcl`, `dirinit`.
- `current_time`, `current_date` (BIOS clock variables).
- `FA_*` flags (`bdos/fs.h`).

Used by: every program that writes to disk.

## Cross-references

- **Compendium:** "Fcreate" — agrees.
- **Profibuch:** "Fcreate (0x3C)" — agrees.
- `docs/api/gemdos/file/3d-fopen.md`, `3e-fclose.md`,
  `docs/api/gemdos/directory/39-dcreate.md`.

## Configuration

None — always present.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: implements full FAT16/32 directory grow + `current_time`
hook into the RPi system clock (PHASE-03 already established).
