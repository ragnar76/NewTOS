# `Fsfirst` — GEMDOS 0x4E

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsdir.c:523` (`xsfirst`), helper `ixsfirst` at `:467`.

## EmuTOS source

```c
long xsfirst(char *name, int att)
{
    DTAINFO *dt = (DTAINFO *)(run->p_xdta);
    dt->dt_offset_drive = -1L;       /* mark uninitialised */

    long result = ixsfirst(name, att, dt);

    if (result < 0 || !contains_wildcard_characters(name))
        return result;
    return E_OK;
}

long ixsfirst(char *name, WORD att, DTAINFO *addr)
{
    if (att != FA_VOL)
        att |= (FA_ARCHIVE | FA_RO);  /* always include archive+RO */

    DND *dn = findit(name, &s, 0);
    if (!dn) return EPTHNF;

    long pos = 0;
    FCB *fcb = scan(dn, s, att, &pos);
    if (!fcb) return EFILNF;

    if (addr) makbuf(fcb, addr);     /* fill DTA from FCB */
    return E_OK;
}
```

`makbuf` populates the DTA's user-visible portion (attribute,
time, date, size, name) from the directory entry FCB.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x004E` |
| WORD args | 3 (LONG + WORD) |
| Stack frame | `+6: att (W)`, `+2: name (L)`, `+0: 0x004E` |

## Inputs

| Pos | Name   | Type    | Range / meaning                                     |
|-----|--------|---------|-----------------------------------------------------|
| 0   | `name` | char *  | path with optional wildcards in last component       |
| 1   | `att`  | WORD    | match-attribute mask (see below)                     |

### `att` mask

| Bit  | Mask  | Meaning                          |
|------|-------|----------------------------------|
| 0    | 0x01  | FA_RO (always added if not VOL)  |
| 1    | 0x02  | FA_HIDDEN — match if set         |
| 2    | 0x04  | FA_SYSTEM — match if set         |
| 3    | 0x08  | FA_VOL — exclusive                |
| 4    | 0x10  | FA_SUBDIR — match if set         |
| 5    | 0x20  | FA_ARCHIVE (always added)         |

The "always-added" semantics mean: search always returns FA_RO and
FA_ARCHIVE files. Hidden/system/folder files only appear if their
bit is set in `att`. Volume labels are exclusive: `att == FA_VOL`
matches only labels.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: 0 success, negative error |

DTA (`run->p_xdta`) is populated. The DTA layout:

| Offset | Field         | Type   |
|-------:|---------------|--------|
| 0..20  | `dt_reserved` | private (offset/drive/cluster for Fsnext) |
| 21     | `dt_attr`     | UBYTE attribute byte                     |
| 22..23 | `dt_time`     | UWORD DOS-time                            |
| 24..25 | `dt_date`     | UWORD DOS-date                            |
| 26..29 | `dt_size`     | LONG file size                            |
| 30..43 | `dt_name`     | 14-byte ASCIIZ filename ("8.3" + zero)    |

## Errors

| Code   | Value | When                                                |
|--------|------:|-----------------------------------------------------|
| EPTHNF | -34   | path component missing                               |
| EFILNF | -33   | no entry matches                                     |

## Edge cases

### 1. Wildcard contract

If `name` contains `*` or `?`, `xsfirst` always returns 0 on
success (the per-FCB result is in DTA). The `if (result < 0 ||
!contains_wildcard_characters(name)) return result;` line means:
no wildcard → return as-is (0 or error); wildcard → always return
0 if a match was found (so subsequent `Fsnext` works).

### 2. DTA per-process

`run->p_xdta` is the per-process DTA pointer set via `Fsetdta`.
Default is in PD; programs override. Concurrent searches in the
same process require swapping DTAs.

### 3. `dt_offset_drive = -1` invalidation

`xsfirst` marks the DTA invalid before searching. If `ixsfirst`
fails or finds no match, `Fsnext` correctly returns ENMFIL because
the sentinel is preserved.

### 4. Wildcard in non-final component

`Fsfirst("dir/*.txt/foo", 0)` is rejected at `findit` time —
`findit` does not match wildcards in path components, only in the
final filename. Returns EPTHNF.

### 5. Always-included FA_ARCHIVE / FA_RO

EmuTOS always sets these in `att` (unless searching for VOL). This
matches DOS/TOS convention but surprises programs expecting
"only return FA_RO if I asked".

### 6. Volume label exclusivity

`att == FA_VOL` (exactly 0x08): returns labels only. Any other
combination ignores volume labels.

### 7. DTA size = 44 bytes

The full DTA is 44 bytes; reserved area is 21 bytes for cluster
position state used by `Fsnext`. Programs that use a custom DTA
must allocate 44 bytes.

### 8. Filename in DTA: 14 bytes "8.3.\0"

`dt_name` is `name.ext\0` style (8.3 dot-separated, null-
terminated, max 12 chars + null + pad = 14).

### 9. AES path-with-name conventions

AES code passes `name` like `"C:\AUTO\*.PRG"`. Path resolution
via `findit` handles drive and directory walk; only the final
component is wildcarded.

## Dependencies

- `findit`, `scan`, `match`, `makbuf`.
- `run->p_xdta`, DTA layout (`bdos/fs.h`).
- `contains_wildcard_characters`.

Used by: every file manager, AES desktop, batch script processors.

## Cross-references

- **Compendium:** "Fsfirst" — agrees.
- **Profibuch:** "Fsfirst (0x4E)" — agrees, lists DTA layout.
- `docs/api/gemdos/file/4f-fsnext.md`, `dta/1a-fsetdta.md`.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: identical DTA layout (44 bytes, big-endian time/date as
on m68k). The "archive + RO always added" idiom is preserved.
