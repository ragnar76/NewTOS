# `Dcreate` — GEMDOS 0x39

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsdir.c:201` (`xmkdir`).

## EmuTOS source (essence)

```c
long xmkdir(char *s)
{
    long h = ixcreat(s, FA_SUBDIR);          /* create dir entry */
    if (h < 0) return h;
    OFD *f = getofd(h);

    /* validate total path length ≤ LEN_ZPATH-3 */
    int plen = namlen(fcb1->f_name);
    for (DND *dn = f->o_dnode; dn; dn = dn->d_parent)
        plen += namlen(dn->d_name);
    if (plen >= LEN_ZPATH-3) {
        ixdel(...);
        return EACCDN;
    }

    /* build DND, allocate first cluster, init "." and ".." entries */
    DND *dn = makdnd(f->o_dnode, fcb1);
    OFD *f0 = makofd(dn);
    if (nextcl(f0, 1)) {            /* allocate cluster */
        ixdel(...);  freednd(dn);
        return EACCDN;
    }

    FCB *fcb2 = dirinit(dn);
    memcpy(fcb2, dots, 22);              /* "."  entry */
    fcb2->f_attrib = FA_SUBDIR;
    fcb2->f_td = dfd->o_td;
    fcb2->f_clust = swap(dfd->o_strtcl);
    fcb2++;
    memcpy(fcb2, dots, 22);              /* ".." entry */
    fcb2->f_name[1] = '.';
    fcb2->f_attrib = FA_SUBDIR;
    /* ".." in root has time/date/cluster zero; otherwise parent's */

    ixclose(f, CL_DIR | CL_FULL);        /* flush */
    sft[h-NUMSTD].f_own = 0;             /* release temp handle */
    sft[h-NUMSTD].f_ofd = 0;
    return E_OK;
}
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0039` |
| WORD args | 2 (LONG) |
| Stack frame | `+2: s (L)`, `+0: 0x0039` |

## Inputs

| Pos | Name | Type    | Range / meaning           |
|-----|------|---------|---------------------------|
| 0   | `s`  | char *  | path of folder to create   |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: 0 success, negative error |

## Errors

| Code   | Value | When                                                  |
|--------|------:|-------------------------------------------------------|
| EPTHNF | -34   | parent path missing; trailing component empty / `.`/`..` |
| EACCDN | -36   | illegal chars; existing entry blocks creation; total path length ≥ `LEN_ZPATH-3` (= 124); cannot grow root; cannot allocate cluster |
| ENHNDL | -35   | SFT exhausted during `ixcreat`                          |

## Edge cases

### 1. Reuses `ixcreat`

The directory entry is created via `ixcreat(s, FA_SUBDIR)` — same
helper as `Fcreate`. The FA_SUBDIR bit propagates (not stripped,
unlike from `xcreat` which masks it off).

### 2. Allocates one cluster immediately

`nextcl(f0, 1)` allocates the first cluster of the new folder.
Failure (disk full) rolls back via `ixdel` on the entry just
created.

### 3. Initialises `.` and `..` entries

Standard DOS convention. `.` points to the new folder's own
cluster; `..` points to the parent's cluster. For folders created
in the root, `..` has time/date/cluster all zero (Atari TOS
convention).

### 4. Path-length cap

`LEN_ZPATH = 128` (full TOS path with drive). The check
`plen >= LEN_ZPATH-3` (=125) leaves room for "X:\" prefix. Trying
to create a folder whose absolute path exceeds 124 chars returns
EACCDN.

### 5. `dots` constant

`dots[]` is a 22-byte template: 11 bytes filename (`. ` then
spaces) + 11 bytes blanks. Used for both `.` and `..` entries
with `..` distinguished by setting `f_name[1] = '.'`.

### 6. Temporary SFT handle freed

`ixcreat` returns a real handle that gets released after init —
the caller never sees it. Programs cannot "open" the new folder
to inspect its handle.

### 7. Cannot create over existing folder

If a folder of the same name exists, `ixcreat` finds it and tries
to delete (per its file-replace logic). The check `(fcb->f_attrib
& FA_SUBDIR) || attr == FA_SUBDIR` blocks this: returns EACCDN.

### 8. Drive not writable

`Fcreate`-level checks (write-protected disk) propagate via
`longjmp` and surface as the underlying BIOS error.

### 9. Empty / root rejection

`!*s || *s == '.'` catches "create root" and "create `.`" / `..`.
Returns EPTHNF.

## Dependencies

- `ixcreat`, `makdnd`, `makofd`, `nextcl`, `dirinit`, `ixdel`,
  `freednd`.
- `dots[]`, `LEN_ZPATH`, `FA_SUBDIR`, `CL_DIR | CL_FULL`.

Used by: `mkdir` shell command, AES installer, archivers.

## Cross-references

- **Compendium:** "Dcreate" — agrees.
- **Profibuch:** "Dcreate (0x39)" — agrees.
- `docs/api/gemdos/directory/3a-ddelete.md`,
  `docs/api/gemdos/file/3c-fcreate.md`.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: same algorithm. The `LEN_ZPATH = 128` cap is a TOS
constant; NewTOS preserves it for ABI compat even though FAT32
allows longer paths.
