# `Fsnext` — GEMDOS 0x4F

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsdir.c:654` (`xsnext`), helper `ixsnext` at `:545`.

## EmuTOS source

```c
long xsnext(void)
{
    DTAINFO *dt = (DTAINFO *)run->p_xdta;

    if (dt->dt_offset_drive < 0L)         /* DTA not initialised */
        return ENMFIL;

    FCB *fcb = ixsnext(dt);
    if (fcb == NULL) {
        dt->dt_offset_drive = -1L;        /* mark exhausted */
        return ENMFIL;
    }

    makbuf(fcb, (DTAINFO *)run->p_xdta);  /* refill DTA */
    return E_OK;
}
```

`ixsnext` resumes the directory walk from the saved cluster /
offset state stored in the DTA's reserved area.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x004F` |
| WORD args | 0 |
| Stack frame | `+0: 0x004F` |

## Inputs

None — state is entirely in the per-process DTA.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: 0 success, ENMFIL on no-more |

DTA reused — refilled with the next match.

## Errors

| Code   | Value | When                                                  |
|--------|------:|-------------------------------------------------------|
| ENMFIL | -49   | no more matches; DTA never initialised; DTA invalidated by previous failure |

## Edge cases

### 1. State in DTA reserved area

The 21-byte reserved area holds:
- `dt_offset_drive` (LONG) — combined drive number (low 6 bits)
  + offset within root directory (when in BT_ROOT)
- `dt_cloffset` (WORD) — offset within current cluster
- `dt_clnum` (CLNO/WORD) — current cluster number
- `dt_attr`, `dt_name` — copy of search attribute and pattern
  for `match()` re-evaluation

Programs that overwrite the reserved area corrupt the search
state — well-behaved programs treat the DTA as opaque between
`Fsfirst` and `Fsnext` calls.

### 2. Sentinel `dt_offset_drive = -1`

`Fsfirst` initialises this to `-1`. `Fsnext` checks for `<0` and
returns ENMFIL — handles both "DTA never initialised" and
"previous search exhausted" with one check.

### 3. Walks both root and subdirs

`ixsnext` distinguishes BT_ROOT (fixed-size root area on FAT12/16)
from BT_DATA (cluster-chained subdirs). When `dt_clnum == 0`, root
is being scanned; otherwise a subdir.

### 4. Cross-cluster transitions

When `recnum` overflows the current cluster, `ixsnext` calls
`getrealcl` to advance. End-of-chain produces NULL → ENMFIL.

### 5. End-of-directory marker

A directory entry with `f_name[0] == 0x00` marks "never used"
(end of valid entries). Earlier scanned + erased (0xE5) entries
are skipped via `match()`'s pattern check.

### 6. Concurrent searches

The search state is in `run->p_xdta`. A program doing two
concurrent searches must save/restore the DTA via `Fgetdta`/
`Fsetdta` between calls. The standard idiom uses a stack-
allocated DTA per search context.

### 7. AES "find next executable"

AES desktop iterates `Fsfirst("*.PRG", FA_NORM)` then `Fsnext`
until ENMFIL to populate the launcher menu. Performance scales
linearly with directory size.

### 8. No `Fsclose` exists

There is no explicit close for a directory iteration. The DTA
sentinel after exhaustion (`-1L`) lets the OS know the iteration
has ended; no resources to release.

### 9. Mediach during iteration

If the user swaps the disk between `Fsfirst` and `Fsnext` on the
*same* drive, EmuTOS detects via `mediach` and returns E_CHNG
(-14) on the next disk read — the iteration aborts.

## Dependencies

- `run->p_xdta`, DTA reserved area.
- `ixsnext`, `getrealcl`, `match`, `makbuf`.
- `BT_ROOT`/`BT_DATA` distinction, FAT chain walk.

Used by: paired with every `Fsfirst` call.

## Cross-references

- **Compendium:** "Fsnext" — agrees.
- **Profibuch:** "Fsnext (0x4F)" — agrees, documents ENMFIL=-49.
- `docs/api/gemdos/file/4e-fsfirst.md`, `dta/1a-fsetdta.md`.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: identical state model. The 21-byte reserved area in
DTA is preserved as ABI requirement (programs serialise/restore).
