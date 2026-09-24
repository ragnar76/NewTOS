# `Ddelete` (GEMDOS 0x3A)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fsdir.c:297` (`xrmdir`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the "must be empty" and "cannot delete root" rules, the
> error codes. Reference: the DND sibling-chain unlink, `findit`/`ixgetfcb`/
> `ixdel`.

## EmuTOS source

**Primary location:** `bdos/fsdir.c:297` — `long xrmdir(char *p)`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x3A` =
`xrmdir`, `wparms = 2` (`bdos/bdosmain.c:254`).

**Helpers called:**
- `bdos/fsdir.c:1407` — `findit(p, &s, 1)` — resolve the path; **mode 1** means
  the target must be a directory (returns its DND, else NULL).
- `bdos/fsdir.c` — `makofd()` — get/make the OFD for the folder's own contents.
- `bdos/fsdir.c:196` — `ixlseek()` / `ixgetfcb()` — walk the folder's entries to
  test emptiness.
- `bdos/fsdir.c` — `ixdel()` — erase the parent's directory entry and free the
  folder's cluster chain.
- `bdos/mem.h` — `xmfreblk()` — free the in-memory OFD/DND.

**Headers / type definitions used:**
- `bdos/fs.h:193` — `ERASE_MARKER = 0xE5` (erased-entry marker in `f_name[0]`).
- `bdos/fs.h:196` — `FA_LFN = 0x0F` (VFAT long-filename slot attribute).
- `bdos/fs.h:206-225` — `DND` fields (`d_parent`, `d_left`, `d_right`,
  `d_files`, `d_ofd`).
- `include/gemerror.h` — `EPTHNF`, `EACCDN`, `EINTRN`.

**System variables / globals read:** the DND tree for the drive (via `findit`).
**System variables / globals written:** the parent on-disk directory (entry
erased); the FAT (folder cluster chain freed); the in-memory DND tree (target
DND unlinked and freed).

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Ddelete` removes an **empty** directory. `xrmdir` resolves the path with
`findit(p, &s, 1)` (mode 1 = must be a folder); a failure returns `EPTHNF`. It
then refuses to delete the **root** of a drive (`!d->d_parent` → `EACCDN`) — the
root is undeletable even on an otherwise-empty disk.

To confirm the folder is empty it opens the folder's own OFD, seeks past the two
mandatory entries (`.` and `..`, i.e. `2 * sizeof(FCB)`), and reads entries: it
skips erased entries (`f_name[0] == ERASE_MARKER`) and VFAT long-filename slots
(`f_attrib == FA_LFN`). If it reaches the end-of-directory marker
(`f_name[0] == 0x00`) or the OFD runs out, the folder is empty. Any surviving
"real" entry means the folder is not empty → `EACCDN`.

If empty, it unlinks the folder's DND from its parent's sibling chain (walking
from the parent's first child along `d_right` until it finds itself, patching the
predecessor's link). Two defensive checks guard against a corrupted tree: if the
DND still has open files (`d_files`) or child directories (`d_left`) it returns
`EINTRN` — these should be impossible once the emptiness scan passed. Finally it
frees the OFD (if any) and the DND (`xmfreblk`), and calls `ixdel` on the parent
to erase the on-disk entry and free the folder's cluster chain; `ixdel`'s result
is returned (`E_OK` on success).

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x3A
- **Stack frame on entry (m68k):** `Ddelete(path)` = `trap1(0x3a, path)`:
  ```
  +2: path (LONG, pointer)
   0: function_id (word) = 0x003A
  ```
- **Total parameter bytes:** 4 (LONG) → `wparms = 2`.
- **Native (AArch64) ABI:** ID `0x3A` in `x8`; arg in `x0`; LONG result
  (`E_OK` or negative error) sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | +2         | `x0`        | `p`  | `char *` (ptr) — path of the folder to delete |

---

## Inputs

| Position | Name | Type    | Size | Source location    | Notes |
|----------|------|---------|------|--------------------|-------|
| arg0     | `p`  | `char *`| 4    | `bdos/fsdir.c:297` | path of the folder to delete; must resolve to an **empty** directory |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** `E_OK` (0) on success; negative TOS error otherwise.
- **Source location:** `bdos/fsdir.c:377` (`return ixdel(...)`); early error
  returns at `:308`, `:312`, `:329`, `:347`, `:353`.

### Side effects on global state

- Erases the folder's entry in the parent on-disk directory and frees its
  cluster chain (`ixdel`, `bdos/fsdir.c:377`).
- Unlinks and frees the folder's in-memory DND (and its OFD, if any) from the
  DND tree (`bdos/fsdir.c:360-369`).

### Side effects on hardware

- Reads the folder's directory sectors (emptiness scan) and writes the parent
  directory + FAT updates through the buffer cache to the block device.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| Path does not resolve to a folder (`findit` mode 1 fails) | `bdos/fsdir.c:307-308` | `EPTHNF` (-34) |
| Target is the root directory (`!d_parent`) | `bdos/fsdir.c:311-312` | `EACCDN` (-36) |
| Folder is not empty (a live, non-erased, non-LFN entry remains) | `bdos/fsdir.c:328-329` | `EACCDN` (-36) |
| Internal: DND still has open files (`d_files`) | `bdos/fsdir.c:346-347` | `EINTRN` (-65) |
| Internal: DND still has child directories (`d_left`) | `bdos/fsdir.c:352-353` | `EINTRN` (-65) |
| Whatever `ixdel` returns on the final erase | `bdos/fsdir.c:377` | `E_OK` or an `ixdel` error |

**Error paths not in Compendium:** the two defensive `EINTRN` returns (open
files / child DNDs still attached) — should be unreachable once the emptiness
scan passes, but they are real return paths. Compendium documents only
`EPTHNF`/`EACCDN`.

---

## Verification criteria

- **Nominal:** `Dcreate("C:\\TMP")` then `Ddelete("C:\\TMP")` returns `E_OK`, and
  a subsequent `Fsfirst("C:\\TMP", FA_SUBDIR)` returns `EFILNF`.
- **Error — not empty:** `Ddelete("C:\\HAS_FILE")` where the folder contains a
  live file returns `EACCDN` (-36).
- **Error — root:** `Ddelete("C:\\")` returns `EACCDN` (-36).
- **Error — missing:** `Ddelete("C:\\NOPE")` returns `EPTHNF` (-34).
- **Empty-after-erase:** a folder whose only remaining slots are erased
  (`0xE5`) or LFN entries is treated as empty and deletes successfully.

---

## Edge cases discovered in source

1. **Only `.`/`..`, erased, and LFN entries count as "empty".** The scan skips
   `ERASE_MARKER`-prefixed entries and `FA_LFN` slots; a live entry (any other
   attribute, non-erased) makes the folder non-empty → `EACCDN`.
   (`bdos/fsdir.c:321-329`)

2. **Root cannot be deleted.** `!d->d_parent` → `EACCDN`, even on a clean disk.
   (`bdos/fsdir.c:311-312`)

3. **Path must be a folder.** `findit(p, &s, 1)` (mode 1); a path resolving to a
   file yields NULL → `EPTHNF` (not `EFILNF`). (`bdos/fsdir.c:306`)

4. **The two `EINTRN` guards are defensive, not the sibling-unlink loop.** The
   sibling-chain walk (`bdos/fsdir.c:340`) has no error exit — it assumes the DND
   is in the chain (guaranteed on a consistent tree). The `EINTRN` returns come
   only from `d_files` / `d_left` being non-NULL after the emptiness check
   passed. (Earlier drafts implied the unlink loop itself returns `EINTRN` —
   it does not.)

5. **Cluster chain freed.** `ixdel` frees the folder's first cluster (holding
   `.`/`..`) and any growth clusters via the standard FAT-chain free loop.
   (`bdos/fsdir.c:377`)

6. **`ENOTEMPTY` does not exist in TOS.** "Not empty" is `EACCDN`, matching DOS
   convention; there is no POSIX-style not-empty code.

---

## Dependencies

### Requires (must be initialized / functional)

- FAT subsystem logged in for the drive (`findit`/`ckdrv`).
- The DND tree for the parent path; buffer cache + `Rwabs`.

### Used by

- `rmdir` shell command, file managers, installers/uninstallers.

---

## Related data structures

- `docs/api/structures/dnd.md` — the DND sibling-chain (`d_parent`/`d_left`/
  `d_right`/`d_files`) walked and patched here.
- `docs/api/gemdos/directory/39-dcreate.md` — the inverse operation.
- `docs/api/gemdos/file/41-fdelete.md` — file delete (uses `ixdel` similarly).

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents `Ddelete`,
  `EPTHNF`, and `EACCDN` for "not empty"/"root"; the `EINTRN` guards are EmuTOS
  internals.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- None. The LFN-skip behaviour is build-independent in this EmuTOS build.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: same algorithm — verify empty (skipping erased/LFN slots), refuse
root, erase entry, free cluster chain. The DND-tree unlink is a free NewTOS
implementation choice; the "empty + non-root only" rule and the error codes are
contract. NewTOS keeps the LFN skip for FAT32 media written by modern hosts.

**FS01 (2026-09-24): the folder's long-name slots are freed with its entry.**
Like `Fdelete`, `Ddelete` now erases the `attrib == 0x0F` slots that precede
the folder's 8.3 entry (`dir::erase_slot_with_lfn`,
`tos-api/src/bdos/fs/dir.rs`); a folder created by a modern host carries such a
chain just as a file does. Rationale, sources and the "a chain that does not
validate is not ours to erase" rule: `docs/api/gemdos/file/41-fdelete.md`,
section *FS01*.
