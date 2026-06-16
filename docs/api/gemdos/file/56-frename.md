# `Frename` — GEMDOS 0x56

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsdir.c:895` (`xrename`).

## EmuTOS source (essence)

```c
long xrename(int n, char *p1, char *p2)
{
    /* n is unused (Atari historical) */

    if (!ixsfirst(p2, FA_SUBDIR, NULL))   /* new path already exists? */
        return EACCDN;

    DND *dn1 = findit(p1, &s1, 0);
    if (!dn1) return EPTHNF;

    /* remember source drive and start cluster */
    DMD  *dmd1   = dn1->d_drv;
    CLNO  strtcl1 = dn1->d_parent ? dn1->d_strtcl : ROOT_PSEUDO_CLUSTER;

    FCB *fcb = scan(dn1, s1, FA_NORM|FA_SUBDIR, &posp);
    if (!fcb) return EFILNF;
    if (fcb->f_attrib & FA_RO) return EACCDN;     /* RO files refused */

    /* save old attribute / time / cluster / length */
    UBYTE att = fcb->f_attrib;
    /* ... swpw / swpl into local copies ... */

    dn1->d_flag |= DND_LOCKED;        /* prevent scavenge */
    DND *dn2 = findit(p2, &s2, 0);
    dn1->d_flag &= ~DND_LOCKED;
    if (!dn2) return EPTHNF;

    if (contains_illegal_characters(s2))
        return EACCDN;
    if (dmd1 != dn2->d_drv)            /* cross-device */
        return ENSAME;

    /* cross-directory: ensure no a\b -> a\b\c (would create cycle) */
    if (strtcl1 != strtcl2) { /* extra checks ... */ }

    /* create new entry, then erase old */
}
```

`ROOT_PSEUDO_CLUSTER` is a sentinel value distinguishing "root
dir" from a real start-cluster of 2 (the actual root start).

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0056` |
| WORD args | 5 (WORD + LONG + LONG) |
| Stack frame | `+10: p2 (L)`, `+6: p1 (L)`, `+2: n (W)`, `+0: 0x0056` |

## Inputs

| Pos | Name | Type    | Range / meaning                          |
|-----|------|---------|------------------------------------------|
| 0   | `n`  | WORD    | unused (historical reserved word)         |
| 1   | `p1` | char *  | source path                                |
| 2   | `p2` | char *  | destination path                           |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: 0 success, negative error |

## Errors

| Code   | Value | When                                                       |
|--------|------:|------------------------------------------------------------|
| EACCDN | -36   | `p2` already exists; `p1` is read-only; `p2` has illegal chars; cyclical move (`a\b` → `a\b\c`) |
| EPTHNF | -34   | source or destination path component missing                |
| EFILNF | -33   | source filename not present                                  |
| ENSAME | -48   | cross-device rename (different drive letters)               |

## Edge cases

### 1. Volume labels not renameable

Per Rainbow TOS Release Notes, the scan uses `FA_NORM | FA_SUBDIR`
(without FA_VOL); volume labels return EFILNF.

### 2. Cross-device → ENSAME

Renaming `A:foo` → `B:bar` returns ENSAME (-48). Programs must
copy + delete instead.

### 3. Cross-directory rename supported

Same drive, different directory — handled. The FCB is removed
from the source directory and a new FCB is created in the target
directory pointing at the same cluster chain. Cluster data is
**not** copied.

### 4. Cycle prevention

`a\b -> a\b\c` would create a cycle (folder containing itself).
EmuTOS detects via `strtcl1 != strtcl2` plus a parent-walk check
and returns EACCDN. `b -> a\b` (rename across folders, no cycle)
is allowed.

### 5. Renaming open files

`xrename` does not check for open OFDs. The OFD's `o_dirfil` /
`o_dirbyt` continue to point at the **old** directory entry,
which has been erased. Subsequent `Fclose` writes the new metadata
to the freed slot — corrupts the directory. **Programs must close
files before renaming.** TOS bug-compatibility: NewTOS preserves
this trap.

### 6. RO files refused

`FA_RO` source returns EACCDN. Programs must clear FA_RO via
`Fattrib` first. Comment in source: "renames are forbidden for
Read-Only files".

### 7. Folder rename allowed (TOS-3+ behaviour)

Source attribute mask `FA_NORM | FA_SUBDIR` permits folder rename.
The folder's own `.` and `..` entries are NOT updated — they
still point at the parent's old/new cluster (which actually
remains the same since the cluster chain is unchanged).

### 8. Argument `n` unused

`n` is historically a "drive number" parameter that was never
implemented. Programs pass 0 or any value; ignored by EmuTOS.

### 9. DND lock

`dn1->d_flag |= DND_LOCKED` prevents the `findit(p2)` call from
scavenging `dn1`. Without this, a memory-pressure path in
`makdnd()` might free `dn1` mid-operation.

## Dependencies

- `findit`, `scan`, `ixsfirst`, `ixdel`.
- DND chain, FCB layout, FAT directory writes.
- `ROOT_PSEUDO_CLUSTER`, `DND_LOCKED`.

Used by: file managers, archivers, command shells.

## Cross-references

- **Compendium:** "Frename" — agrees, notes `n` unused.
- **Profibuch:** "Frename (0x56)" — agrees.
- `docs/api/gemdos/file/41-fdelete.md`, `3c-fcreate.md`.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: same algorithm. The "open file rename corrupts dir"
quirk (Edge case 5) is **NOT** preserved — NewTOS will check the
OFD chain and return EACCDN if any open OFD references the source.
This is a deliberate divergence from TOS bug-compat; TBD whether
it warrants a discrepancy entry. Logged informally in
`docs/api/_discrepancies.md` once a real program is found that
*depends* on the original buggy behaviour.
