# `Fdelete` — GEMDOS 0x41

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsopnclo.c:623` (`xunlink`), helper `ixdel` at `:646`.

## EmuTOS source

```c
long xunlink(char *name)
{
    return ixunlink(name);            /* trivial wrapper */
}

/* ixunlink does findit + scan, then ixdel */
long ixdel(DND *dn, FCB *f, long pos)
{
    /* check if any open OFD points at this dir entry */
    for (fd = dn->d_files; fd; fd = fd->o_link)
        if (fd->o_dirbyt == pos)
            for (n = 0; n < OPNFILES; n++)
                if (sft[n].f_ofd == fd) {
                    if (sft[n].f_own == run) ixclose(fd, 0);
                    else                     return EACCDN;
                }

    /* free FAT chain */
    n = f->f_clust; swpw(n);
    while (n && !endofchain(n)) {
        n2 = getrealcl(n, dm);
        clfix(n, FREECLUSTER, dm);
        n = n2;
    }

    /* mark dir entry as erased (0xE5 in first byte) */
    fd = dn->d_ofd;
    ixlseek(fd, pos);
    c = ERASE_MARKER;
    ixwrite(fd, 1L, &c);
    ixclose(fd, CL_DIR);
    return E_OK;
}
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0041` |
| WORD args | 2 (LONG) |
| Stack frame | `+2: name (L)`, `+0: 0x0041` |

## Inputs

| Pos | Name   | Type    | Range / meaning             |
|-----|--------|---------|-----------------------------|
| 0   | `name` | char *  | path of file to delete       |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: 0 success, negative error |

## Errors

| Code   | Value | When                                                       |
|--------|------:|------------------------------------------------------------|
| EFILNF | -33   | filename not found in directory                             |
| EPTHNF | -34   | path component does not exist                               |
| EACCDN | -36   | file is read-only / is a folder / is open in another process |

## Edge cases

### 1. Read-only files

EACCDN. Programs must clear FA_RO via `Fattrib` first.

### 2. Cannot delete folders

Use `Ddelete` for folders. `Fdelete` on a folder returns EACCDN
(scan with FA_NORM finds it, the RO/folder check rejects it).

### 3. Deletes own open file silently

If the file is currently open by the **same** process, `ixdel`
calls `ixclose` first, then proceeds. The handle becomes invalid
(EIHNDL on subsequent ops). Programs commonly do
`Fopen` → `Fread` → `Fclose` → `Fdelete`; the open-and-delete
short-cut is supported but unusual.

### 4. Open in another process → EACCDN

If `sft[n].f_own != run`, deletion fails. This is the main reason
TOS tolerates `Fdelete` on a file with the same name created by
another process — it's blocked.

### 5. FAT chain freeing

Walks the entire chain via `getrealcl` and writes `FREECLUSTER`
(0x0000) into each FAT entry via `clfix`. A torn write here leaves
orphaned clusters; CHKDSK-equivalents detect them via the standard
"lost cluster" scan.

### 6. ERASE_MARKER = 0xE5

The first byte of the directory entry is set to 0xE5. A real TOS
quirk: filenames starting with `0xE5` (Sigma-S in Atari ROM) get
silently encoded as `0x05` to avoid mistaken erasure.

### 7. `dirinit` not called

`ixdel` does not compact the directory; the slot stays marked as
erased and `ixcreat` reuses it on the next file creation. `f_name[0]
== 0x00` (never used) terminates the scan; erased entries (0xE5)
are skipped by attribute matching.

### 8. Volume labels

Volume labels are deleted by `Fdelete` only if the path matches.
The standard idiom `Fdelete("\\*.*")` does NOT iterate; it just
fails with EFILNF since `*` isn't valid in a path component for
exact match. To delete a label, programs use `Fcreate(..., FA_VOL)`
which replaces the existing label.

## Dependencies

- `findit`, `scan`, `ixdel`, `ixclose`.
- FAT manipulation (`getrealcl`, `clfix`, `endofchain`).
- Global `sft[]`, OFD chain (`dn->d_files`).

Used by: file managers, installers, every program that cleans up
temp files.

## Cross-references

- **Compendium:** "Fdelete" — agrees.
- **Profibuch:** "Fdelete (0x41)" — agrees.
- `docs/api/gemdos/directory/3a-ddelete.md` — folder deletion.
- `docs/api/gemdos/file/43-fattrib.md` — clearing FA_RO first.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: full FAT delete with cluster-chain freeing. The
ERASE_MARKER = 0xE5 / encoded-0x05 quirk is preserved for
on-disk compatibility with native ST media.
