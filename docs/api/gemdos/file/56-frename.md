# `Frename` (GEMDOS 0x56)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fsdir.c:895` (`xrename`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI (**reserved word first, then two path pointers**), the
> "target must not exist" / RO / cross-device rules, folder rename support, the
> full error set. Reference: the create-then-erase cross-directory algorithm,
> `is_subdir`, DND locking.

## EmuTOS source

**Primary location:** `bdos/fsdir.c:895` — `long xrename(int n, char *p1, char *p2)`
(`n` is a historical, unused reserved word).

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x56` =
`xrename`, `wparms=5` (`bdos/bdosmain.c:286`).

**Helpers called:**
- `bdos/fsdir.c:452` — `ixsfirst()` — check whether the target `p2` already
  exists (called with a NULL DTA, search-only).
- `bdos/fsdir.c:1407` — `findit()`, `bdos/fsdir.c:1525` — `scan()`.
- `bdos/fsopnclo.c:708` — `contains_illegal_characters()`.
- `bdos/fsdir.c:853` — `is_subdir()` — cycle-prevention predicate.
- `bdos/fsdir.c:871` — `update_fcb()` — seek + write a directory-entry field.
- `bdos/fsopnclo.c:99` — `ixcreat()` (cross-directory case creates the new
  entry), `bdos/fsopnclo.c:426` — `xclose()`, `bdos/fsopnclo.c:511` —
  `ixclose()`.
- `bdos/fsdir.c` — `builds()`, `getdnd()`, `unpackit()`, `freednd()`.

**Headers / type definitions used:**
- `bdos/fs.h:195` — `FA_NORM`, `include/bdosdefs.h:40` — `FA_SUBDIR`.
- `bdos/fsdir.c:137` — `ROOT_PSEUDO_CLUSTER = 1` (distinguishes the root from a
  real start cluster).
- `bdos/fs.h:230` — `DND_LOCKED`.
- `bdos/fsdir.c:139` — `DIR_FILE_LENGTH` (fake size for directory entries).

**System variables / globals read/written:** the source and destination
directory sectors, FAT (unchanged — cluster chain is *not* copied), buffer cache;
`sft[]`/OFD for the transient new entry in the cross-directory case.

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Frename` renames or moves a file (or folder) on the **same** drive. The `n`
argument is a vestigial "drive number" that was never implemented and is ignored
(the binding passes 0: `Frename(oldname, newname)` = `trap1(0x56, 0, oldname,
newname)`, `include/bdosbind.h:53`).

It first checks that the target path `p2` does **not** already exist by calling
`ixsfirst(p2, FA_SUBDIR, NULL)`; a success (return 0) means the target exists →
`EACCDN`. It resolves the source `p1` (`findit`; failure → `EPTHNF`) and scans
it with `FA_NORM | FA_SUBDIR` — note this mask includes sub-directories (so
folders can be renamed) but **excludes volume labels** (per the Rainbow TOS
Release Notes, labels cannot be renamed → they are "not found", `EFILNF`).
Read-only source files are refused (`EACCDN`). It saves the source's attribute,
time/date, start cluster, and length.

It then resolves the target directory `p2` (locking the source DND during the
call so it is not scavenged) — failure → `EPTHNF`. Illegal characters in the new
name → `EACCDN`. A different drive → `ENSAME`. If source and target are in the
same directory (same start cluster) it simply overwrites the FCB name in place.
If they differ, it prevents cycles (`is_subdir`, e.g. `a\b -> a\b\c` → `EACCDN`),
`ixcreat`s a new entry in the target directory carrying the old
time/date/cluster/length, erases the old entry (`0xE5`), and — for a folder move
— updates the moved folder's `..` entry to point at the new parent and fixes the
folder's attribute byte in the new parent (failures here → `EINTRN`). The FAT
cluster chain is **not** copied; only the directory entry moves.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x56
- **Stack frame on entry (m68k):** `Frename(oldname, newname)` =
  `trap1(0x56, 0, oldname, newname)` (`include/bdosbind.h:53`) — a reserved
  WORD is pushed first, then the two path pointers:
  ```
  +10: newname (LONG, pointer) → p2
   +6: oldname (LONG, pointer) → p1
   +2: n = 0   (word, reserved)
    0: function_id (word) = 0x0056
  ```
- **Total parameter bytes:** 10 (WORD + LONG + LONG) → `wparms = 5`.
- **Native (AArch64) ABI:** ID `0x56` in `x8`; args in `x0..x2` in declaration
  order; LONG result (0 or negative error) sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | +2         | `x0`        | `n`  | `WORD` (16) — reserved, ignored |
  | +6         | `x1`        | `p1` | `char *` (ptr) — old path |
  | +10        | `x2`        | `p2` | `char *` (ptr) — new path |

---

## Inputs

| Position | Name | Type     | Size | Source location    | Notes |
|----------|------|----------|------|--------------------|-------|
| arg0     | `n`  | `int`    | 2    | `bdos/fsdir.c:895` | historical reserved word; unused |
| arg1     | `p1` | `char *` | 4    | `bdos/fsdir.c:915` | source path (file or folder) |
| arg2     | `p2` | `char *` | 4    | `bdos/fsdir.c:912` | destination path; must not already exist |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** `E_OK` (0) on success; negative error otherwise.
- **Source location:** `bdos/fsdir.c:1100` (final `ixclose(fd, CL_DIR)`).

### Side effects on global state

- Same-directory: overwrites the FCB name in place (`bdos/fsdir.c:1077-1082`).
- Cross-directory: creates a new directory entry, erases the old, updates a
  moved folder's `..` and parent attribute byte (`bdos/fsdir.c:1005-1073`).
- For a folder rename, frees any stale DND for the old name
  (`bdos/fsdir.c:1089-1098`).

### Side effects on hardware

- Directory-sector reads/writes; no FAT-data movement.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| Target `p2` already exists | `bdos/fsdir.c:912-913` | `EACCDN` (-36) |
| Source path (directory portion) not found | `bdos/fsdir.c:915-917` | `EPTHNF` (-34) |
| Source name not found (or is a volume label) | `bdos/fsdir.c:937-938` | `EFILNF` (-33) |
| Source is read-only (`FA_RO`) | `bdos/fsdir.c:943-944` | `EACCDN` (-36) |
| Target directory not found | `bdos/fsdir.c:975-976` | `EPTHNF` (-34) |
| Illegal characters in the new name | `bdos/fsdir.c:985-986` | `EACCDN` (-36) |
| Cross-device (different drive) | `bdos/fsdir.c:989-990` | `ENSAME` (-48) |
| Cyclical move (`is_subdir`, e.g. `a\b → a\b\c`) | `bdos/fsdir.c:1002-1003` | `EACCDN` (-36) |
| `ixcreat` of the new entry failed | `bdos/fsdir.c:1017-1018` | `EPTHNF` (-34) |
| Can't erase old entry (cross-dir) | `bdos/fsdir.c:1023-1027` | `EACCDN` (-36) |
| Can't update folder `..` / parent attribute (cross-dir folder) | `bdos/fsdir.c:1053-1064` | `EINTRN` (-65) |

**Error paths not in Compendium:** the `EINTRN` cases for a cross-directory
folder move, and the "target already exists → `EACCDN`" precheck.

---

## Verification criteria

- **Same-directory rename:** `Frename(0, "C:\\OLD.TXT", "C:\\NEW.TXT")` returns
  `E_OK`; `OLD.TXT` becomes `NEW.TXT`, cluster chain unchanged.
- **Cross-directory move:** `Frename(0, "C:\\A\\F.TXT", "C:\\B\\F.TXT")` (same
  drive) returns `E_OK`.
- **Target exists:** renaming onto an existing name returns `EACCDN` (-36).
- **RO source:** renaming an `FA_RO` file returns `EACCDN` (-36).
- **Cross-device:** `Frename(0, "A:\\F", "B:\\F")` returns `ENSAME` (-48).
- **Missing source:** `Frename(0, "C:\\NONE", "C:\\X")` returns `EFILNF` (-33).

---

## Edge cases discovered in source

1. **`n` is ignored.** Historical reserved word; the binding hard-codes 0
   (`bdos/fsdir.c:894-895`, `include/bdosbind.h:53`).

2. **Target must not exist.** Checked via `ixsfirst(p2, FA_SUBDIR, NULL)` before
   anything else → `EACCDN` if it does (`bdos/fsdir.c:912-913`).

3. **Folders can be renamed/moved; labels cannot.** Scan mask
   `FA_NORM | FA_SUBDIR` includes directories but not `FA_VOL`
   (`bdos/fsdir.c:936`).

4. **Read-only files are refused.** `FA_RO` source → `EACCDN`
   (`bdos/fsdir.c:943-944`).

5. **Cross-device → `ENSAME`.** Only same-drive renames; copy+delete otherwise
   (`bdos/fsdir.c:989-990`).

6. **Cross-directory move updates the folder `..`.** For a moved folder, the
   `..` entry is rewritten to the new parent's start cluster and the folder's
   attribute in the new parent is fixed (`bdos/fsdir.c:1043-1064`). (Correction:
   the `..` entry **is** updated — an earlier draft claimed it was not.)

7. **FAT chain is not copied.** Only the directory entry moves; the file's data
   clusters are untouched (`bdos/fsdir.c:1029-1034`).

8. **DND locking during resolution.** `dn1->d_flag |= DND_LOCKED` around the
   `findit(p2)` / `ixcreat` calls prevents the source DND being scavenged
   mid-operation (`bdos/fsdir.c:971-973,1012-1016`).

9. **Open source files are not checked.** `xrename` does not verify that the
   source is closed; renaming an open file leaves the OFD pointing at the erased
   old entry, which can corrupt the directory on a later `Fclose`. Programs must
   close before renaming. (NewTOS intends to *fix* this — see NewTOS notes and
   the discrepancy log.)

---

## Dependencies

### Requires (must be initialized / functional)

- FAT subsystem for the drive; DND tree; buffer cache; `ixcreat` path.

### Used by

- File managers, archivers, shells.

---

## Related data structures

- `docs/api/structures/ofd.md` — FCB fields moved; DND relationships.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the reserved
  `n`, `ENSAME` for cross-device, and rename semantics.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- Folder rename/move is supported (unlike the strict TOS-1 files-only view);
  labels are not renameable. `ROOT_PSEUDO_CLUSTER = 1`.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: same algorithm. The **open-file rename corruption** (edge case 9) is
**not** to be preserved — NewTOS will check the OFD chain and refuse (`EACCDN`)
a rename of a file with an open OFD. That is a deliberate divergence from TOS
bug-compat; it is logged in `docs/api/_discrepancies.md`.
