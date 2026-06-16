# `Dfree` — GEMDOS 0x36

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsfat.c:337` (`xgetfree`).

## EmuTOS source

```c
long xgetfree(long *buf, int drv)
{
    drv = drv ? drv-1 : run->p_curdrv;       /* 0 = current */

    WORD n = ckdrv(drv, TRUE);
    if (n < 0) return ERR;

    DMD *dm = drvtbl[n];
    CLNO free;

    if (dm->m_16) {
        free = countfree16(dm);              /* fast 16-bit FAT scan */
    } else {
        free = 0;
        for (i = 0; i < dm->m_numcl; i++)
            if (!getrealcl(i+2, dm))         /* cluster numbers from 2 */
                free++;
    }

    *buf++ = (long) free;
    *buf++ = (long) dm->m_numcl;
    *buf++ = (long) dm->m_recsiz;
    *buf   = (long) dm->m_clsiz;
    return E_OK;
}
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0036` |
| WORD args | 3 (LONG + WORD) |
| Stack frame | `+6: drv (W)`, `+2: buf (L)`, `+0: 0x0036` |

## Inputs

| Pos | Name | Type     | Range / meaning                                |
|-----|------|----------|------------------------------------------------|
| 0   | `buf`| LONG *   | pointer to 16-byte buffer (4 LONGs)             |
| 1   | `drv`| WORD     | 0 = current drive, 1 = A:, 2 = B:, …             |

Note the `+1` offset: 0 means "current", not "A:".

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: 0 (E_OK) success, `ERR` (-1) on `ckdrv` failure |

`buf[0..3]` filled in as 4 LONGs (16 bytes total):

| Index | Value           | Meaning                              |
|------:|-----------------|--------------------------------------|
| 0     | `free`           | free clusters                         |
| 1     | `m_numcl`        | total clusters on volume              |
| 2     | `m_recsiz`       | bytes per logical record (sector)      |
| 3     | `m_clsiz`        | records per cluster                    |

Bytes-per-cluster = `buf[2] * buf[3]`.
Free bytes        = `buf[0] * buf[2] * buf[3]`.
Total bytes       = `buf[1] * buf[2] * buf[3]`.

## Errors

| Code | Value | When                                                  |
|------|------:|-------------------------------------------------------|
| ERR  | -1    | `ckdrv` failed (drive offline / not in `Drvmap`)       |

EmuTOS returns the generic `ERR = -1`, not the specific EDRIVE
(-46). Compendium documents EDRIVE here — see Discrepancy candidate.

## Edge cases

### 1. Drive number `+1` convention

`drv = 0` is "current drive"; `drv = 1` is A:, `drv = 2` is B:.
Compare to `Dgetpath` which uses the same convention. `Dsetdrv`
uses 0-based (0 = A:).

### 2. FAT12 vs FAT16 path

`m_16` (16-bit FAT) flag selects the fast `countfree16` path
(scans 16-bit entries directly). FAT12 uses the slow per-cluster
`getrealcl` loop (~3× slower because each entry crosses bytes).
NewTOS supports FAT16/32; FAT12 (floppy) covered by the same code.

### 3. Cluster numbers start at 2

`for (i=0; i<m_numcl; i++) getrealcl(i+2, dm)` — FAT clusters
start at 2 (clusters 0 and 1 are reserved for FAT signature and
end-of-chain marker).

### 4. `ERR` vs `EDRIVE` discrepancy candidate

EmuTOS returns `ERR = -1` on bad drive. Compendium and Profibuch
both document `EDRIVE = -46`. Real Atari TOS also returns `-1`.
**This matches the convention `EmuTOS = TOS`, so NewTOS follows.**
Logged in `docs/api/_discrepancies.md` as candidate D-010 if
worth tracking.

### 5. No buffer-validation

`xgetfree` does not check that `buf` is a valid pointer. Garbage
or NULL crashes via the page-fault path on the host. NewTOS
should validate at trap entry (basic user-pointer check).

### 6. `ckdrv(drv, TRUE)` triggers media change check

The TRUE flag means "do mediach check". If the disk has been
swapped, this remounts the drive (or returns E_CHNG depending on
state). Programs use `Dfree(0, ?)` as a cheap "is the disk still
there?" probe.

### 7. Performance on large volumes

Counting free clusters on a multi-GB FAT32 volume is O(N) — can
take seconds. AES caches the result; programs hammering `Dfree`
in tight loops noticeably slow down on large drives.

## Dependencies

- `ckdrv`, `drvtbl[]`, `DMD` (drive media descriptor).
- `countfree16`, `getrealcl`, `endofchain`.
- `run->p_curdrv`.

Used by: `df`-style tools, file managers showing "X bytes free",
GEM disk-info dialog.

## Cross-references

- **Compendium:** "Dfree" — agrees on layout, claims EDRIVE on
  error (incorrect — see Edge case 4).
- **Profibuch:** "Dfree (0x36)" — same.
- `docs/api/gemdos/drive/0e-dsetdrv.md`.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: backed by FAT driver. NewTOS may add a "fast Dfree"
extension that returns a cached value from the mount layer; the
classic syscall keeps the precise (slow) scan for ABI fidelity.
