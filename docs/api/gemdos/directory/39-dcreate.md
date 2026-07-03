# `Dcreate` (GEMDOS 0x39)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fsdir.c:201` (`xmkdir`) and the shared create helper
> `bdos/fsopnclo.c:99` (`ixcreat`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the error codes, the illegal-character set, the
> on-disk `.`/`..` initialisation, the total-path-length cap. Reference: the
> DND/OFD/FCB structures, `ixcreat`, `makdnd`/`makofd`/`nextcl`/`dirinit`.

## EmuTOS source

**Primary location:** `bdos/fsdir.c:201` — `long xmkdir(char *s)`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x39` =
`xmkdir`, `wparms = 2` (`bdos/bdosmain.c:253`).

**Helpers called:**
- `bdos/fsopnclo.c:99` — `ixcreat(s, FA_SUBDIR)` — create the directory entry;
  does path resolution (`findit`), illegal-char / empty-name checks, the
  "existing entry blocks creation" check, and allocates the SFT handle.
- `bdos/fsdir.c:178` — `namlen()` — packed 8.3 name length, used for the
  total-path-length cap.
- `bdos/fsdir.c` — `makdnd()` / `makofd()` — build the in-memory DND/OFD (only
  return on success).
- `bdos/fsfat.c` — `nextcl(f0, 1)` — allocate the folder's first cluster.
- `bdos/fsdir.c:1257` — `dirinit()` — zero the new directory cluster and return
  a pointer to write `.`/`..` into.
- `bdos/fsopnclo.c` — `ixclose(f, CL_DIR | CL_FULL)` — flush the new folder to
  disk; `bdos/fsdir.c` — `ixdel()` — roll back the entry on failure.

**Headers / type definitions used:**
- `include/bdosdefs.h:40` — `FA_SUBDIR = 0x10`.
- `include/sysconf.h:51` — `LEN_ZPATH = 114` (max path incl. drive).
- `bdos/fsopnclo.c:64` — `ILLEGAL_FNAME_CHARACTERS = " *,:;<=>?[]|"`.
- `bdos/fs.h:490-491` — `CL_DIR`, `CL_FULL`.
- `bdos/fs.h:32` — `FNAMELEN = 11`.
- `bdos/fsdir.c:167` — `dots[22]` template for the `.` / `..` entries.

**System variables / globals read:** `run` (path/drive context via `ixcreat`→
`findit`); `current_time`/`current_date` (stamped into the new entry inside
`ixcreat`).
**System variables / globals written:** on-disk directory (new entry + new
folder cluster with `.`/`..`); one `sft[]` slot is transiently allocated by
`ixcreat` and then **released** by `xmkdir` (`f_own = 0`, `f_ofd = 0`).

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Dcreate` creates a new sub-directory at path `s`. `xmkdir` delegates the
directory-entry creation to `ixcreat(s, FA_SUBDIR)` — the **same helper**
`Fcreate` uses, except `Fcreate` strips `FA_SUBDIR` (`xcreat` masks
`attr & ~FA_SUBDIR`, `bdos/fsopnclo.c:85`) whereas `xmkdir` passes it through, so
the entry is marked as a directory. `ixcreat` resolves the parent path with
`findit` (failure → `EPTHNF`), rejects an empty final component or a `.`/`..`
name (→ `EPTHNF`), rejects any of the illegal filename characters
`" *,:;<=>?[]|"` (→ `EACCDN`), and — crucially — if an entry of that name
already exists, returns `EACCDN` because for a directory create the guard
`(fcb->f_attrib & (FA_SUBDIR|FA_RO)) || (attr == FA_SUBDIR)` is **always true**
(`attr == FA_SUBDIR`). So `Dcreate` fails with `EACCDN` if **any** entry — file
*or* folder — of that name already exists; it never overwrites.

On a successful entry create, `xmkdir` checks the **total path length**: it sums
the packed 8.3 lengths (`namlen`) of the new name plus every ancestor DND name;
if that reaches `LEN_ZPATH-3` (= 111) it rolls the entry back with `ixdel` and
returns `EACCDN`. It then builds the in-memory DND (`makdnd`) and OFD (`makofd`),
allocates the folder's first cluster with `nextcl(f0, 1)` (failure — e.g. disk
full — rolls back and returns `EACCDN`), and initialises the cluster with
`dirinit`.

Into that fresh cluster it writes the two mandatory entries: `.` (attribute
`FA_SUBDIR`, time/date and start cluster = the new folder's own) and `..`
(attribute `FA_SUBDIR`). For a folder created in the **root**, the `..` entry's
time, date, and cluster are all set to **zero** (Atari/DOS convention); otherwise
`..` gets the parent's time/date and start cluster. Finally it forces the folder
to disk (`ixclose(f, CL_DIR | CL_FULL)`), frees the temporary OFD (`xmfreblk`),
releases the transient SFT slot, and returns `E_OK`. The caller never sees a
handle for the new folder.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x39
- **Stack frame on entry (m68k):** `Dcreate(path)` = `trap1(0x39, path)`:
  ```
  +2: path (LONG, pointer)
   0: function_id (word) = 0x0039
  ```
- **Total parameter bytes:** 4 (LONG) → `wparms = 2`.
- **Native (AArch64) ABI:** ID `0x39` in `x8`; arg in `x0`; LONG result
  (`E_OK` or negative error) sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | +2         | `x0`        | `s`  | `char *` (ptr) — path of folder to create |

---

## Inputs

| Position | Name | Type    | Size | Source location    | Notes |
|----------|------|---------|------|--------------------|-------|
| arg0     | `s`  | `char *`| 4    | `bdos/fsdir.c:201` | path of the folder to create (drive-prefixed or relative to the current dir) |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** `E_OK` (0) on success; negative TOS error otherwise.
- **Source location:** `bdos/fsdir.c:283` (`return E_OK`); error returns at
  `:212`, `:229`, `:242`.

### Side effects on global state

- Writes a new directory entry into the parent directory and a new one-cluster
  directory (with `.`/`..`) to disk (`bdos/fsdir.c:245-279`).
- Allocates the folder's first cluster in the FAT (`nextcl`, `:237`).
- Transiently allocates then **releases** one `sft[]` slot (`:281-282`); frees
  the temporary OFD (`xmfreblk`, `:280`). No handle is returned to the caller.

### Side effects on hardware

- Reads directory sectors (path resolution) and writes the new entry + folder
  cluster through the buffer cache to the block device.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| Parent path not found (`findit` fails in `ixcreat`) | `bdos/fsopnclo.c:114-115` | `EPTHNF` (-34) |
| Empty final component, or name is `.` / `..` | `bdos/fsopnclo.c:117-118` | `EPTHNF` (-34) |
| Name contains an illegal character (`" *,:;<=>?[]|"`) | `bdos/fsopnclo.c:121-122` | `EACCDN` (-36) |
| An entry of that name already exists (file **or** folder) | `bdos/fsopnclo.c:159-160` | `EACCDN` (-36) |
| Cannot grow the root directory (root full) | `bdos/fsopnclo.c:175-176` | `EACCDN` (-36) |
| Cluster allocation failed inside `ixcreat` (disk full) | `bdos/fsopnclo.c:178-179` | `EACCDN` (-36) |
| SFT exhausted (`opnfil` in `ixcreat`) | `bdos/fsopnclo.c:344-345` | `ENHNDL` (-35) |
| Total path length ≥ `LEN_ZPATH-3` (= 111) | `bdos/fsdir.c:226-229` | `EACCDN` (-36) |
| Cannot allocate the new folder's first cluster (disk full) | `bdos/fsdir.c:237-242` | `EACCDN` (-36) |

**Error paths not in Compendium:** the "existing file *or* folder of that name
→ `EACCDN`" collapse (Compendium tends to describe `Dcreate` as failing only on
an existing *directory*), and the `LEN_ZPATH-3 = 111` total-path cap (Compendium
does not state the exact bound).

---

## Verification criteria

- **Nominal:** `Dcreate("C:\\NEWDIR")` on a writable C: with `NEWDIR` absent
  returns `E_OK`; a following `Fsfirst("C:\\NEWDIR", FA_SUBDIR)` finds it, and its
  cluster contains a `.` and a `..` entry.
- **Root `..` is zeroed:** a folder created directly in the root has a `..` entry
  whose time, date, and start cluster are all 0.
- **Error — bad parent:** `Dcreate("C:\\NODIR\\X")` returns `EPTHNF` (-34).
- **Error — name exists (file):** `Dcreate("C:\\EXISTS")` where `EXISTS` is a
  regular file returns `EACCDN` (-36) (does not overwrite).
- **Error — name exists (folder):** `Dcreate("C:\\SUBDIR")` where `SUBDIR` is a
  folder returns `EACCDN` (-36).
- **Error — illegal char:** `Dcreate("C:\\A*B")` returns `EACCDN` (-36).
- **Error — path too long:** creating a folder whose summed packed path length
  reaches 111 returns `EACCDN` (-36).

---

## Edge cases discovered in source

1. **`FA_SUBDIR` is preserved.** `xmkdir` calls `ixcreat(s, FA_SUBDIR)`
   directly; the entry is a directory. (`Fcreate` strips this bit;
   `bdos/fsopnclo.c:85` vs `bdos/fsdir.c:211`.)

2. **Any pre-existing entry blocks creation.** Because `attr == FA_SUBDIR`, the
   `ixcreat` overwrite guard is always taken → `EACCDN`, whether the existing
   entry is a file or a folder. `Dcreate` never replaces. (`bdos/fsopnclo.c:159`)

3. **Total-path cap is `LEN_ZPATH-3 = 111`**, not 124/125. `LEN_ZPATH = 114`
   in this build (`include/sysconf.h:51`). The 3 bytes reserve the `X:\` prefix.
   (`bdos/fsdir.c:226`) (Earlier drafts used 128/124 — wrong; see D-025.)

4. **`.`/`..` written explicitly.** `dots[22]` (`. ` + blanks) is copied twice;
   `..` is distinguished by `f_name[1] = '.'`. Root-folder `..` has zero
   time/date/cluster; otherwise it points at the parent. (`bdos/fsdir.c:245-276`)

5. **Rollback on failure.** A failed length check or cluster allocation calls
   `ixdel` on the just-created entry (and `freednd` for the DND) so a partial
   folder is not left on disk. (`bdos/fsdir.c:228,239-241`)

6. **Transient handle is invisible.** `ixcreat` returns a real SFT handle;
   `xmkdir` uses it to initialise the folder then releases it (`f_own = 0`).
   Programs cannot obtain a handle to a freshly-created directory.
   (`bdos/fsdir.c:281-282`)

7. **Illegal-character set matches `Fcreate`.** `" *,:;<=>?[]|"` — includes
   space, `*`, `,`, `:`, `;`, `<`, `=`, `>`, `?`, `[`, `]`, `|`; does **not**
   include `/`, `\`, or `"`. (See D-021, `bdos/fsopnclo.c:64`.)

8. **Write-protected / hard I/O errors** surface via the BDOS `longjmp` /
   critical-error path as the underlying BIOS error, not via a `Dcreate`-specific
   code.

---

## Dependencies

### Requires (must be initialized / functional)

- FAT subsystem for the drive logged in (`findit`/`ckdrv`).
- A free `sft[]` slot (transiently) and free clusters on the volume.
- Buffer cache + `Rwabs`.

### Used by

- `mkdir` shell command, AES installers, archivers that recreate trees.

---

## Related data structures

- `docs/api/structures/dnd.md` / `ofd.md` — the in-memory directory node and
  open-file descriptor built during creation.
- `docs/api/gemdos/_state.md` §2 — the SFT slot transiently used and released.
- `docs/api/gemdos/directory/3a-ddelete.md` — the inverse operation.
- `docs/api/gemdos/file/3c-fcreate.md` — shares `ixcreat` and the illegal-char
  set.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents `Dcreate` and
  `EPTHNF`/`EACCDN`; the exact "existing file also blocks" and the `111`-byte cap
  are EmuTOS specifics.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `LEN_ZPATH = 114` in this build; the cap is `111`. If NewTOS widens the path
  limit it must do so as a documented, deliberate divergence.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: same algorithm — create entry, cap total path length, allocate one
cluster, write `.`/`..` (root `..` zeroed), flush, return `E_OK`. The transient
handle and the DND/OFD internals are free NewTOS choices; the on-disk result and
the error codes are contract.
