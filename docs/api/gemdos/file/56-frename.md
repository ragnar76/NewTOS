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
same directory (same start cluster) it simply overwrites the eleven name bytes
of the existing FCB in place (`fsdir.c:1076-1083`): slot, attribute, time/date,
cluster and length are untouched, so the directory order is kept and an OFD
open on the file still names the live entry. If they differ, it prevents cycles
(`is_subdir`, e.g. `a\b -> a\b\c` → `EACCDN`), `ixcreat(p2,att)`s a new entry in
the target directory — **any** failure of that call is answered `EPTHNF`
(`fsdir.c:1014-1018`) — erases the old entry (`0xE5`), copies the old
time/date/cluster/length into the new handle's DFD with `O_DIRTY`, and closes
it: for a file through `xclose` → `ixclose(fd,0)`, which writes them to the new
entry **and ORs `FA_ARCHIVE` into its attribute** (`fsopnclo.c:530-549`); for a
folder through `ixclose(fd2,CL_DIR|CL_FULL)` (length written as 0, no archive
bit) after updating the moved folder's `..` entry to the new parent and setting
the folder's attribute byte in the new parent to `att` (failures here →
`EINTRN`). The FAT cluster chain is **not** copied; only the directory entry
moves.

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

- Same-directory: overwrites the eleven FCB name bytes in place
  (`bdos/fsdir.c:1077-1082`); nothing else in the entry changes.
- Cross-directory: creates a new directory entry (`ixcreat`: name, `att`,
  `f_fill` zeroed, `fsopnclo.c:187-196`) carrying the old time/date, start
  cluster and length; a moved **file** gains `FA_ARCHIVE` (`ixclose`,
  `fsopnclo.c:541-545`), a moved folder keeps `att`; erases the old entry,
  updates a moved folder's `..` and parent attribute byte
  (`bdos/fsdir.c:1005-1073`).
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
| Cyclical move, **one level only** (`is_subdir`, e.g. `a\b → a\b\c`) | `bdos/fsdir.c:1002-1003` | `EACCDN` (-36) |
| `ixcreat` of the new entry failed (cross-dir only): no name (`"C:\"`), a name starting with `.` (`fsopnclo.c:117-118`), no room — a full root cannot grow (`:172-176`) or a sub-directory cannot grow — or no free handle | `bdos/fsdir.c:1017-1018` | `EPTHNF` (-34) |
| Can't erase old entry (cross-dir) | `bdos/fsdir.c:1023-1027` | `EACCDN` (-36) |
| Can't update folder `..` / parent attribute (cross-dir folder) | `bdos/fsdir.c:1053-1064` | `EINTRN` (-65) |

**Check order** (`fsdir.c:912-990`) — the first failing check answers:
target exists → source directory → source entry → read-only source → target
directory → illegal characters in the new name → other drive → (cross-dir)
cycle → `ixcreat`. So `Frename("C:\NODIR\X","C:\EXISTS")` is `EACCDN`, and
`Frename("C:\NOFILE","C:\NODIR\Y")` is `EFILNF`.

**Error paths not in Compendium:** the `EINTRN` cases for a cross-directory
folder move, and the "target already exists → `EACCDN`" precheck.

**Not caught at all (EmuTOS source gap, see edge case 6a / D-070):** a
multi-level self-nesting move (moving a folder into a subdirectory of
itself, e.g. `a → a\sub\a2`) is **not** rejected — `is_subdir` only detects
the immediate one-level case. NewTOS must decide whether to widen this check
(walking the full ancestor chain) as a deliberate safety improvement, or
reproduce the gap for bug-compat (not recommended, given this is exactly the
historical `Frename` corruption class this batch was asked to scrutinise).

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
- **In place:** files `A`, `B` in one folder, `Frename(A, C)` → the folder lists
  `C`, `B` (same slot); a file renamed while open, then written and closed,
  has the written length and data.
- **Cross-dir empty name:** `Frename(0, "C:\\F\\X.TXT", "C:\\")` returns
  `EPTHNF` (-34) and creates nothing (TASK/FS14).
- **Archive bit:** a file with attribute `0x02` moved to another folder has
  `0x22`; renamed within its folder it keeps `0x02`.

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

6a. **`is_subdir` only catches a one-level self-nesting cycle — deeper cycles
   are not detected.** Its own doc comment says exactly this: "check if
   directory 2 is an **immediate** subdirectory of directory 1"
   (`bdos/fsdir.c:844-863`). The check is
   `dn2->d_parent != dn1 → not a cycle; else compare dn2's name to s1`. This
   catches `Frename(0,"C:\A","C:\A\NEW")` (moving `A` to be nested directly
   inside itself) but **not** a two-or-more-level self-nesting move, e.g.
   `Frename(0,"C:\A","C:\A\SUB\NEW")` where `SUB` is a subdirectory of `A`:
   there, `dn2` (the DND for `C:\A\SUB`) has `d_parent == A`'s DND, not the
   root (`dn1`), so `is_subdir` returns `FALSE` and the rename **proceeds**,
   moving `A`'s directory entry to live inside its own descendant `SUB` — a
   genuine directory-tree cycle (traversal code such as `Dgetpath`'s
   `dopath()`, which recurses via `d_parent`, or any future CHKDSK-style
   tree walker, would recurse without terminating). This is a **source-level
   gap in EmuTOS's own cycle prevention**, not a misreading of it — see
   `docs/api/_discrepancies.md` D-070. NewTOS's mutation path must decide
   whether to walk the full ancestor chain (not just one level) before
   allowing a cross-directory folder move.

7. **FAT chain is not copied.** Only the directory entry moves; the file's data
   clusters are untouched (`bdos/fsdir.c:1029-1034`).

8. **DND locking during resolution.** `dn1->d_flag |= DND_LOCKED` around the
   `findit(p2)` / `ixcreat` calls prevents the source DND being scavenged
   mid-operation (`bdos/fsdir.c:971-973,1012-1016`).

9. **Open source files are not checked.** `xrename` does not verify that the
   source is closed. In the **same** directory this is harmless: the entry
   keeps its slot, and the OFD's `ixclose` writes the length/cluster/time into
   the renamed entry (`fsopnclo.c:530-549`). **Across** directories the OFD
   keeps pointing at the erased old entry (`o_dirfil`/`o_dirbyt`); a later
   dirty `Fclose` writes the start cluster and length into the erased slot,
   while the moved entry keeps those of the move — data written after the move
   is lost and its clusters leak. (Correction 2026-10-19, TASK/FS18: an
   earlier draft called the same-directory case corrupting too.) NewTOS
   deviation: see NewTOS notes and `docs/COMPATIBILITY.md`.

10. **Check order.** The target-exists test comes before everything else, the
    source before the target directory (see "Check order" above).

11. **"Target exists" sees what `ixsfirst(p2, FA_SUBDIR)` sees.** `ixsfirst`
    ORs `FA_ARCHIVE|FA_RO` into the mask (`fsdir.c:459-460`), and `match`
    accepts an entry with attribute 0 or one sharing a bit with the mask
    (`fsdir.c:1841-1847`): a target whose attribute is only
    `FA_HIDDEN`/`FA_SYSTEM` (e.g. `0x02`) is **not** found, and the rename
    proceeds — in the same directory it writes a second entry of that name,
    across directories `ixcreat` deletes the hidden file first
    (`fsopnclo.c:148-165`). Wildcards in `p2` match like `Fsfirst`. NewTOS:
    not reproduced (any entry of that exact name is "existing" → `EACCDN`);
    open, not part of FS18.

12. **Over-long names.** `builds` truncates each part of the new name to 8.3
    (`fsdir.c:737-795`). NewTOS' `fcb_name` refuses them with `EPTHNF`, as in
    every other path call; open, not part of FS18.

13. **A same-directory rename to an empty name** writes eleven blanks as the
    name (`builds("")`); only the cross-directory case fails (`ixcreat`).
    NewTOS matches both (TASK/FS06, FS14).

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

`tos-api/src/bdos/fs/file.rs` `frename` (TASK/FS18 + FS14, 2026-10-19): the
EmuTOS check order; same-directory = the eleven name bytes overwritten in
place; cross-directory = `ixcreat`-style entry (name, `att` | `FA_ARCHIVE` for
files, zeroed `0x0C..0x15`, old time/date/cluster, length 0 for a folder), every
`ixcreat` failure `EPTHNF`, old entry erased, `..` fixed. Tests
`tos-api/tests/fs18_frename.rs`.

Additions over EmuTOS:
- The cycle guard walks the whole ancestor chain (D-070), not one level.
- The old name's VFAT long-name slots are erased with it — on the in-place
  rename too, where they no longer match the new name (TASK/FS01 rule;
  invisible to TOS, keeps `fsck_msdos` clean). Test
  `tos-api/tests/fs18_frename_lfn.rs`.
- **Open file moved across directories:** its OFDs are pointed at the new
  entry (`move_ofds`), so the close of a handle written after the move lands
  in the moved entry. EmuTOS loses that data (edge case 9). Sanctioned
  deviation, `docs/COMPATIBILITY.md` (FS18). This replaces the Phase-00 plan
  to refuse such a rename with `EACCDN` (D-023), which would have changed
  `Frename`'s result for every program renaming an open file.
- No handle is consumed by the cross-directory move (EmuTOS' `ixcreat` opens
  one, so a full handle table makes it fail `EPTHNF`); not reproduced.
