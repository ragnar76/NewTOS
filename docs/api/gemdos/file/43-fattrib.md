# `Fattrib` — GEMDOS 0x43

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsdir.c:402` (`xchmod`).

## EmuTOS source

```c
long xchmod(char *p, int wrt, UBYTE mod)
{
    DND *dn = findit(p, &s, 0);
    if (!dn) return EPTHNF;

    long pos = 0;
    if (!scan(dn, s, FA_NORM, &pos))
        return EFILNF;

    if (wrt && (mod & ~FA_NORM))    /* invalid bits for ordinary file */
        return EACCDN;

    pos -= 21;                       /* offset of attribute byte in FCB */
    OFD *fd = dn->d_ofd;
    ixlseek(fd, pos);

    if (!wrt)
        ixread(fd, 1L, &mod);        /* get */
    else {
        ixwrite(fd, 1L, &mod);
        ixclose(fd, CL_DIR);          /* flush dir */
    }
    return mod;
}
```

`FA_NORM = 0x27` (FA_RO | FA_HIDDEN | FA_SYSTEM | FA_ARCHIVE).
Setting FA_VOL or FA_SUBDIR via `Fattrib` is rejected.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0043` |
| WORD args | 4 (LONG + WORD + WORD) |
| Stack frame | `+8: mod (W)`, `+6: wrt (W)`, `+2: p (L)`, `+0: 0x0043` |

## Inputs

| Pos | Name | Type    | Range / meaning                                 |
|-----|------|---------|-------------------------------------------------|
| 0   | `p`  | char *  | path                                            |
| 1   | `wrt`| WORD    | 0 = get, 1 = set                                 |
| 2   | `mod`| WORD    | new attribute byte (low 8 bits used; only on set) |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: attribute byte (sign-extended UBYTE) on success, negative error |

For `wrt = 0`: returns the file's current attributes.
For `wrt = 1`: returns the new attributes (i.e. `mod`).

## Errors

| Code   | Value | When                                              |
|--------|------:|---------------------------------------------------|
| EPTHNF | -34   | path component does not exist                      |
| EFILNF | -33   | filename not found                                  |
| EACCDN | -36   | `wrt = 1` and `mod` contains FA_VOL or FA_SUBDIR    |

## Edge cases

### 1. EmuTOS = TOS 1-3 behaviour: files only

`scan(.., FA_NORM, ..)` only matches FA_NORM entries — folders and
volume labels return EFILNF. TOS 4 added folder support; EmuTOS
deliberately matches TOS 1-3.

### 2. FA_RO / FA_HIDDEN / FA_SYSTEM / FA_ARCHIVE

These four bits are settable. The mask `~FA_NORM = 0xD8` rejects
any combination including FA_VOL (0x08), FA_SUBDIR (0x10), or
upper bits.

### 3. Read returns sign-extended

The byte read into `mod` is returned as a LONG. Bit 7 of the
attribute byte (rare — usually 0) sign-extends. Programs should
mask `& 0xFF`.

### 4. Directly writes to dir entry

The on-disk attribute byte at offset `+11` of the FCB is rewritten
in place (`pos - 21` because `scan` advances `pos` by `sizeof(FCB)
= 32` and `32 - 11 = 21`). The change is flushed via `ixclose(fd,
CL_DIR)`.

### 5. Open files

`Fattrib` operates on the directory entry, not on any open OFD.
Changing FA_RO on a file currently open RW does not retroactively
revoke the open mode. The next `Fopen` will see the new flag.

### 6. Write of identical value

Setting the same `mod` still triggers a dir flush. Programs
hitting this in tight loops will thrash the buffer cache.

### 7. Wildcards

`Fattrib` does NOT iterate; passing `*.txt` returns EFILNF (scan
with FA_NORM does match wildcards in `scan`, but the resulting
behaviour is "first match wins" — which is undefined for a single
attribute change). Programs should resolve names via `Fsfirst`/
`Fsnext` and apply `Fattrib` per file.

## Dependencies

- `findit`, `scan`, `ixread`, `ixwrite`, `ixclose`, `ixlseek`.
- FCB layout (`f_attrib` at offset +11).
- `FA_NORM` (`bdos/fs.h`).

Used by: file managers, archivers, installers.

## Cross-references

- **Compendium:** "Fattrib" — agrees with TOS 1-3 behaviour.
- **Profibuch:** "Fattrib (0x43)" — notes TOS 4 folder support
  (which EmuTOS does not implement).
- `docs/api/gemdos/file/3c-fcreate.md` — initial attributes.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: keeps TOS 1-3 file-only behaviour to avoid surprising
programs that rely on EFILNF for folders.
