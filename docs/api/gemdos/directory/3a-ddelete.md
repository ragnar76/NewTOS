# `Ddelete` — GEMDOS 0x3A

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/fsdir.c:297` (`xrmdir`).

## EmuTOS source (essence)

```c
long xrmdir(char *p)
{
    DND *d = findit(p, &s, 1);          /* mode 1: must be folder */
    if (!d)            return EPTHNF;
    if (!d->d_parent)  return EACCDN;    /* can't delete root */

    OFD *fd = d->d_ofd ?: makofd(d);

    /* scan dir to ensure it's empty (skip ".", "..", erased, LFN) */
    ixlseek(fd, 2*sizeof(FCB));
    do {
        fcb = ixgetfcb(fd);
        if (!fcb) break;
    } while (fcb->f_name[0] == ERASE_MARKER || fcb->f_attrib == FA_LFN);

    if (fcb && fcb->f_name[0] != 0x00)  /* found a real entry */
        return EACCDN;

    /* unlink from sibling DND chain */
    for (d1 = *(q = &d->d_parent->d_left); d1 != d; d1 = *(q = &d1->d_right)) ;
    if (d->d_files) return EINTRN;        /* sanity */
    if (d->d_left)  return EINTRN;
    *q = d->d_right;

    /* free OFD/DND, then erase parent dir entry */
    if (d->d_ofd) xmfreblk(d->d_ofd);
    DND *parent = d->d_parent;
    xmfreblk(d);

    ixlseek(fd->o_dirfil, fd->o_dirbyt);
    fcb = ixgetfcb(fd->o_dirfil);
    return ixdel(parent, fcb, fd->o_dirbyt);
}
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x003A` |
| WORD args | 2 (LONG) |
| Stack frame | `+2: p (L)`, `+0: 0x003A` |

## Inputs

| Pos | Name | Type    | Range / meaning                  |
|-----|------|---------|----------------------------------|
| 0   | `p`  | char *  | path of folder to delete (must be empty) |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG: 0 success, negative error |

## Errors

| Code   | Value | When                                                  |
|--------|------:|-------------------------------------------------------|
| EPTHNF | -34   | path component does not exist                          |
| EACCDN | -36   | trying to delete root; folder not empty                 |
| EINTRN | -65   | internal: open OFDs in folder, or child DNDs still attached |

## Edge cases

### 1. Folder must be empty

After `.` and `..`, scan must find either an end-of-dir marker
(`f_name[0] == 0`) or only erased / LFN entries. Any real entry
returns EACCDN.

### 2. Root cannot be deleted

`!d->d_parent` ⇒ EACCDN. Even on a clean disk with everything
removed, root is undeletable.

### 3. LFN entries skipped

EmuTOS recognises FAT-LFN entries (`f_attrib == FA_LFN = 0x0F`)
and skips them when checking emptiness. A folder containing only
LFN orphans (left over from a damaged dir) is considered empty
and can be deleted.

### 4. Erased entries skipped

`ERASE_MARKER = 0xE5`-prefixed entries are skipped. A folder
that previously had files (now deleted) is empty if no live
entries remain.

### 5. DND not in cache → `findit` walks disk

`findit(p, &s, 1)` either uses an existing DND or walks the
parent dir to build one. Mode 1 ensures the result is a folder
(not a file).

### 6. Sibling-chain unlink

The DND tree is a parent → leftmost-child → right-siblings linked
list. `xrmdir` walks until it finds itself, then patches the
predecessor's pointer. EINTRN if the chain is corrupted (DND not
found in sibling list — can't happen unless memory was scribbled).

### 7. `d_files` or `d_left` non-empty → EINTRN

These indicate "open files in this folder" or "child folders
still cached". Should be impossible if the disk-emptiness check
passed; defensive EINTRN. NewTOS will likely never trigger.

### 8. Deletes folder cluster chain

The actual cluster containing the folder's directory data is
freed via `ixdel`'s standard cluster-chain free loop. The first
cluster (with `.` and `..`) is freed along with any growth
clusters.

### 9. ERR vs EACCDN for "not empty"

Compendium documents "directory not empty" as EACCDN. EmuTOS
matches. POSIX uses ENOTEMPTY; TOS does not have this code.

## Dependencies

- `findit` (mode 1), `ixgetfcb`, `ixdel`, `makofd`.
- DND tree (`d_parent`, `d_left`, `d_right`, `d_files`).
- `ERASE_MARKER`, `FA_LFN`.

Used by: `rmdir` shell command, file managers, installers.

## Cross-references

- **Compendium:** "Ddelete" — agrees.
- **Profibuch:** "Ddelete (0x3A)" — agrees.
- `docs/api/gemdos/directory/39-dcreate.md`,
  `docs/api/gemdos/file/41-fdelete.md`.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: same algorithm. LFN skip is preserved (NewTOS supports
LFN read-through for compatibility with FAT32 media written by
modern hosts).
