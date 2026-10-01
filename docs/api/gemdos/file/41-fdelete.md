# `Fdelete` (GEMDOS 0x41)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fsopnclo.c:623` (`xunlink`), the static helper
> `bdos/fsopnclo.c:588` (`ixunlink`), and the work routine
> `bdos/fsopnclo.c:652` (`ixdel`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, RO/folder/other-process refusals, error codes,
> the on-disk `0xE5` erase marker. Reference: FAT-chain free, `ixdel`
> internals, buffer flush.

## EmuTOS source

**Primary location:** `bdos/fsopnclo.c:623` — `long xunlink(char *name)`
(thin wrapper over `ixunlink`).

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x41` =
`xunlink`, `wparms=2` (`bdos/bdosmain.c:261`).

**Helpers called:**
- `bdos/fsopnclo.c:588` — `ixunlink()` — resolve + scan + RO check + `ixdel`.
- `bdos/fsdir.c:1407` — `findit()`, `bdos/fsdir.c:1525` — `scan()`.
- `bdos/fsopnclo.c:652` — `ixdel()` — close-if-mine-else-refuse, free FAT
  chain, mark the directory entry erased.
- `bdos/fsopnclo.c:511` — `ixclose()`; `bdos/fsfat.c` — `getrealcl()`,
  `clfix()`, `endofchain()`.

**Headers / type definitions used:**
- `bdos/fs.h:193` — `ERASE_MARKER = 0xE5`.
- `bdos/fs.h:195` — `FA_NORM = 0x27`.
- `bdos/fs.h:483-485` — `FREECLUSTER`, `ENDOFCHAIN`, `endofchain()`.

**System variables / globals read:** `run` (for the owner check in `ixdel`);
`dn->d_files` (open OFDs on the directory), `sft[]`.
**System variables / globals written:** the FAT (clusters freed), the directory
sector (first name byte → `0xE5`), buffer cache.

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Fdelete` removes a file by name. `ixunlink` resolves the path (`findit`;
failure → `EFILNF`) and scans the directory with the `FA_NORM` mask
(`bdos/fsopnclo.c:604`). Because `FA_NORM` excludes `FA_VOL`/`FA_SUBDIR`,
`Fdelete` matches only normal files — a folder or a volume label is simply "not
found" and returns `EFILNF`. If the found entry is read-only (`FA_RO`), it
returns `EACCDN`; the caller must clear the flag with `Fattrib` first.

`ixdel` then does the real work. It walks the directory's list of open files
(`dn->d_files`) looking for an OFD at the same directory offset; for each, it
checks `sft[]`: if the file is open by the **current** process it is closed
first (`ixclose`), but if it is open by **another** process, `ixdel` returns
`EACCDN`. Next it frees the file's FAT cluster chain (`getrealcl`/`clfix` write
`FREECLUSTER` into each entry). Finally it overwrites the first byte of the
directory entry with the erase marker `0xE5` and flushes (`ixclose(fd,
CL_DIR)`). On success `E_OK`. Physical failures during these steps take the
BIOS `longjmp` path, so `ixdel` can simply return `E_OK` afterward.

Note the interface: `Fdelete` has no wildcard iteration and no error for
"already gone"; it targets one named file.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x41
- **Stack frame on entry (m68k):** `Fdelete(fname)` = `trap1(0x41, fname)`:
  ```
  +2: fname (LONG, pointer)
   0: function_id (word) = 0x0041
  ```
- **Total parameter bytes:** 4 (LONG) → `wparms = 2`.
- **Native (AArch64) ABI:** ID `0x41` in `x8`; `name` in `x0`; LONG result
  sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name   | Type / size |
  |------------|-------------|--------|-------------|
  | +2         | `x0`        | `name` | `char *` (ptr) |

---

## Inputs

| Position | Name   | Type     | Size | Source location       | Notes |
|----------|--------|----------|------|-----------------------|-------|
| arg0     | `name` | `char *` | 4    | `bdos/fsopnclo.c:588` | path of file to delete; exact name, no wildcards |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** `E_OK` (0) on success; negative error otherwise.
- **Source location:** `bdos/fsopnclo.c:699` (`ixdel` success).

### Side effects on global state

- Frees the file's FAT cluster chain (`bdos/fsopnclo.c:674-683`).
- Marks the directory entry erased (`0xE5`) and flushes
  (`bdos/fsopnclo.c:688-692`).
- May close an OFD owned by the current process before deleting.

### Side effects on hardware

- FAT and directory-sector writes via the block device.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| Path/file not found (`findit` NULL, or `scan` no match) | `bdos/fsopnclo.c:598-599`, `:604-605` | `EFILNF` (-33) |
| Entry is read-only (`FA_RO`) | `bdos/fsopnclo.c:607-608` | `EACCDN` (-36) |
| File open by **another** process | `bdos/fsopnclo.c:668` (`ixdel`) | `EACCDN` (-36) |

**Note:** because the scan uses `FA_NORM`, a **folder** or **volume label** of
that name is not matched and the result is `EFILNF`, not `EACCDN` — the old
spec's "delete on a folder returns EACCDN" is incorrect; delete folders with
`Ddelete` (0x3A).

**Error paths not in Compendium:** the "open by another process → `EACCDN`"
distinction and the same-process auto-close before delete.

---

## Verification criteria

- **Nominal:** `Fdelete("C:\\TMP.DAT")` on an existing, non-RO, unopened file
  returns `E_OK` (0); a following `Fopen` of that name returns `EFILNF`.
- **RO refusal:** `Fdelete` on an `FA_RO` file returns `EACCDN` (-36); after
  `Fattrib(name, 1, 0)` the delete succeeds.
- **Folder not matched:** `Fdelete("C:\\SUBDIR")` returns `EFILNF` (-33).
- **Missing file:** `Fdelete("C:\\NONE")` returns `EFILNF` (-33).

---

## Edge cases discovered in source

1. **RO files are refused.** `FA_RO` → `EACCDN` (`bdos/fsopnclo.c:607-608`).

2. **Folders/labels are "not found", not "denied".** The `FA_NORM` scan mask
   excludes them, so `Fdelete` returns `EFILNF` (`bdos/fsopnclo.c:604`).

3. **Same-process open file is auto-closed, then deleted.** If the file is open
   by `run`, `ixdel` calls `ixclose` first and proceeds; the handle becomes
   stale (`bdos/fsopnclo.c:665-666`).

4. **Open by another process blocks.** `sft[n].f_own != run` → `EACCDN`
   (`bdos/fsopnclo.c:667-668`).

5. **Erase marker `0xE5`.** The first name byte is set to `0xE5`
   (`ERASE_MARKER`); the slot is not compacted and is reused by the next
   `Fcreate` (`bdos/fsopnclo.c:688-692`). **Correction:** an earlier draft of
   this spec claimed a name legitimately beginning with byte `0xE5` is stored
   on disk as `0x05` by the create path (the classic MS-DOS convention to avoid
   false-erasure). **This EmuTOS build implements no such substitution** —
   `builds()`/`ixcreat()` (`bdos/fsopnclo.c:99-208`, `bdos/fsdir.c:737-796`)
   copy/uppercase bytes verbatim (`toupper()` is a plain `'a'..'z'` fold,
   `util/string.c:171-177`) with no `0xE5`→`0x05` special case anywhere in
   `bdos/fsdir.c`/`bdos/fsopnclo.c`. A file whose 8.3-encoded name legitimately
   starts with the byte `0xE5` is therefore indistinguishable from an erased
   entry on the next directory scan (`match()`/`ixgetfcb()` both treat
   `f_name[0] == 0xE5` as "deleted", `bdos/fsdir.c:1818`, `:614`) — a latent,
   real EmuTOS limitation, not a NewTOS bug. See `_discrepancies.md` D-063.

6. **FAT chain freed cluster-by-cluster.** `getrealcl`/`clfix` write
   `FREECLUSTER`; a crash mid-free leaves lost clusters that a CHKDSK-style scan
   would reclaim (`bdos/fsopnclo.c:674-683`).

---

## Dependencies

### Requires (must be initialized / functional)

- FAT subsystem for the drive; buffer cache + `Rwabs`; `sft[]`/OFD chain.

### Used by

- File managers, installers, temp-file cleanup; `Frename`'s overwrite path via
  `ixcreat`→`ixdel` for a pre-existing target.

---

## Related data structures

- `docs/api/structures/ofd.md` — FCB, `ERASE_MARKER`, FAT chain fields.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents "returns 0 or
  error"; the "another process → EACCDN" and folder-is-EFILNF details are
  EmuTOS's.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- None.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: FAT delete with cluster-chain free; the `0xE5`/`0x05` on-disk erase
convention is preserved for native-ST media compatibility.

**FS01 (2026-09-24): a deleted entry takes its long-name slots with it.**
EmuTOS frees the 8.3 entry alone, because classic TOS knows no long names —
its directory scan only *skips* `FA_LFN` records (`bdos/fsdir.c:326,1810`) and
nothing ever erases them. NewTOS does not write long-name slots either, but
every card written by a modern host carries them, and leaving them behind on
delete accumulates orphan chains: after emptying a card completely, `fsck_msdos
-n` no longer completes (measured: it dies in phase 2 on signal 5,
`measurements/measurement-2026-09-24-fs01/serial/fsck.txt`).

So `Fdelete` (and `Ddelete`, and the `Fcreate`-over-an-existing-file path,
which shares `erase_entry`) now marks the 8.3 entry `0xE5` **and then** the
`attrib == 0x0F` slots in front of it — `dir::erase_slot_with_lfn`
(`tos-api/src/bdos/fs/dir.rs`). Which slots belong to the entry is decided the
way FreeMiNT's FAT layer decides it (`references/freemint/sys/fatfs.c:779-790`
for the `LDIR` record, `:3690-3748` for the ordinal/checksum validation), and
the erase order — short entry first, then the chain — is FreeMiNT's
`unlink_cookie_i` (`:4246-4272`). A chain that does not validate (wrong
checksum, broken ordinal sequence, already erased) is left untouched: it was
not this entry's name to begin with, and FreeMiNT's `__readvfat` rejects it the
same way. This is a **NewTOS decision** taken from the FAT layout, not
behaviour extracted from EmuTOS — EmuTOS has no answer here.
