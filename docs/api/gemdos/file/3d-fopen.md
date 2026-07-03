# `Fopen` (GEMDOS 0x3D)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fsopnclo.c:221` (`xopen`) and the static helper
> `bdos/fsopnclo.c:239` (`ixopen`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the *access-code* mode bits actually honoured
> (`0..2`), the RO-blocks-write rule, error codes, returned handle range.
> Reference: `findit`/`scan`/`opnfil`, the OFD/DFD structures.

## EmuTOS source

**Primary location:** `bdos/fsopnclo.c:221` — `long xopen(char *name, int mod)`.
It masks `mod & VALID_FOPEN_BITS` and forwards to `ixopen`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x3D` =
`xopen`, `wparms=3` (`bdos/bdosmain.c:257`). **However**, before dispatch the
GEMDOS trap layer intercepts the special device names `CON:`/`AUX:`/`PRN:`
(and lowercase) and returns a negative BIOS handle *without* calling `xopen`
(`bdos/bdosmain.c:651-663`; table at `:103-108`). See Edge case 5.

**Helpers called:**
- `bdos/fsopnclo.c:239` — `ixopen()` — resolve + scan + access check.
- `bdos/fsdir.c:1407` — `findit()` — path → parent `DND`.
- `bdos/fsdir.c:1525` — `scan()` — find the file (mask `FA_NORM`).
- `bdos/fsopnclo.c:335` — `opnfil()` → `bdos/fsopnclo.c:271` — `makopn()` —
  allocate SFT slot, build OFD, share the DFD if the file is already open.

**Headers / type definitions used:**
- `bdos/fs.h:167-173` — `INH_MODE`/`MODE_FSM`/`MODE_FAC`/`RO_MODE`/`WO_MODE`/
  `RW_MODE`/**`VALID_FOPEN_BITS = MODE_FAC = 0x07`**.
- `bdos/fs.h:195` — `FA_NORM` (`= FA_ARCHIVE|FA_SYSTEM|FA_HIDDEN|FA_RO = 0x27`).
- `include/bdosdefs.h:36` — `FA_RO`.

**System variables / globals read:** `run` (path/drive context, `p_uft`).
**System variables / globals written:** `sft[]` (new entry); OFD/DFD allocations.

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Fopen` opens an existing file and returns a handle. `xopen` first reduces the
mode to `mod & 0x07` — **only the low three bits (the access code) are kept**;
bits 4-6 (the file-sharing mode `MODE_FSM`) and bit 7 (`INH_MODE`, inheritance)
are **not** valid `Fopen` bits in this EmuTOS build and are masked away
(`VALID_FOPEN_BITS = MODE_FAC = 0x07`, `bdos/fs.h:169,173`). This is the single
biggest correction versus the old spec, which claimed `0x77` and documented
sharing bits — those are recorded only inside the OFD when set via other paths,
not accepted from `Fopen`.

`ixopen` resolves the path with `findit`; if that fails it returns `EFILNF`
(not `EPTHNF` — `Fopen` collapses both "path missing" and "file missing" to
`EFILNF`, `bdos/fsopnclo.c:248-249`). It then `scan`s the directory with the
`FA_NORM` mask, which **excludes volume labels and sub-directories**, so `Fopen`
never returns a handle for a folder or a label even when the name exists — you
still get `EFILNF`. If the found file has `FA_RO` set and any write access is
requested (`mod != 0`, i.e. `WO_MODE` or `RW_MODE`), it returns `EACCDN`;
read-only opens (`mod == 0`) of RO files succeed. Otherwise `opnfil` allocates
the handle.

The access code is recorded in `OFD.o_mod` but, as documented under `Fread`
/`Fwrite`, EmuTOS deliberately does **not** enforce read/write direction at
transfer time (Atari-TOS compatibility). So the mode chiefly matters for the
RO-blocks-write gate above.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x3D
- **Stack frame on entry (m68k):** `Fopen(fname, mode)` =
  `trap1(0x3d, fname, mode)` (`include/bdosbind.h:38`):
  ```
  +6: mode  (word)
  +2: fname (LONG, pointer)
   0: function_id (word) = 0x003D
  ```
- **Total parameter bytes:** 6 (LONG + WORD) → `wparms = 3`.
- **Native (AArch64) ABI:** ID `0x3D` in `x8`; args in `x0..x1`; LONG result
  (handle or negative error) **sign-extended** into `x0`.

  | m68k stack | AArch64 reg | Name   | Type / size |
  |------------|-------------|--------|-------------|
  | +2         | `x0`        | `name` | `char *` (ptr) |
  | +6         | `x1`        | `mod`  | `WORD` (16) — masked to `& 0x07` |

---

## Inputs

| Position | Name   | Type     | Size | Source location      | Notes |
|----------|--------|----------|------|----------------------|-------|
| arg0     | `name` | `char *` | 4    | `bdos/fsopnclo.c:239`| path to an existing file |
| arg1     | `mod`  | `int`    | 2    | `bdos/fsopnclo.c:225`| access mode; **only bits 0-2 honoured** |

### `mod` value (`bdos/fs.h:169-173`), after `& VALID_FOPEN_BITS (0x07)`

| Value | Constant  | Meaning |
|------:|-----------|---------|
| 0     | `RO_MODE` | read-only |
| 1     | `WO_MODE` | write-only |
| 2     | `RW_MODE` | read-write |
| 3-7   | —         | accepted by the mask but not a defined access code; treated as `mod != 0` (write) for the RO gate |

Bits 4-6 (`MODE_FSM`, file sharing) and bit 7 (`INH_MODE`) are **masked off**;
`Fopen` cannot set them. (Old spec claimed a `0x77` mask + sharing bits —
wrong.)

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** file handle `>= 6` on success; negative TOS error otherwise.
- **Source location:** `bdos/fsopnclo.c:263` (`opnfil` return via `makopn`).

### Side effects on global state

- Allocates one `sft[]` entry and an OFD (`bdos/fsopnclo.c:341-352`); if the
  file is already open, the new OFD shares the existing DFD and bumps
  `o_usecnt` (`bdos/fsopnclo.c:302-308`).

### Side effects on hardware

- Reads directory sectors to locate the FCB.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| Path resolution failed (`findit` NULL) | `bdos/fsopnclo.c:248-249` | `EFILNF` (-33) |
| Name not found by `scan` (or is a folder/label, excluded by `FA_NORM`) | `bdos/fsopnclo.c:256-257` | `EFILNF` (-33) |
| File is `FA_RO` and write access requested (`mod != 0`) | `bdos/fsopnclo.c:260-261` | `EACCDN` (-36) |
| SFT exhausted | `bdos/fsopnclo.c:345-346` | `ENHNDL` (-35) |

**Error paths not in Compendium:** `Fopen` returns `EFILNF` (not `EPTHNF`)
even when it is the *directory* portion of the path that is missing — the old
spec listed `EPTHNF` as a possible return, which the source never produces for
`Fopen`.

---

## Verification criteria

- **Nominal read:** `Fopen("C:\\FILE.TXT", 0)` on an existing readable file
  returns a handle `>= 6`.
- **RO read allowed:** `Fopen` with `mod=0` on a file whose entry has `FA_RO`
  set succeeds.
- **RO write blocked:** `Fopen("C:\\ROFILE", 2)` on an `FA_RO` file returns
  `EACCDN` (-36).
- **Missing file:** `Fopen("C:\\NOPE.TXT", 0)` returns `EFILNF` (-33).
- **Folder is not openable:** `Fopen("C:\\SUBDIR", 0)` where `SUBDIR` is a
  directory returns `EFILNF` (-33).
- **Special name:** `Fopen("CON:", 0)` returns the console BIOS handle
  `0xFFFF` (= -1), *without* touching the filesystem.

---

## Edge cases discovered in source

1. **Only the access code (bits 0-2) survives.** `mod & 0x07`
   (`VALID_FOPEN_BITS`); sharing/inheritance bits are dropped
   (`bdos/fsopnclo.c:225`, `bdos/fs.h:169-173`).

2. **RO files open read-only, block writes.** `mod == 0` succeeds on `FA_RO`
   files; any `mod != 0` returns `EACCDN` (`bdos/fsopnclo.c:260-261`).

3. **Access direction is recorded but not enforced.** `o_mod` stores the code,
   but `Fread`/`Fwrite` ignore it (Atari-TOS compat). So a `WO_MODE` handle can
   still be read, and a `RO_MODE` handle can still be written.

4. **Folders and volume labels are invisible.** `scan(.., FA_NORM, ..)` skips
   `FA_VOL`/`FA_SUBDIR` entries — `Fopen` on a directory name gives `EFILNF`,
   not a directory handle (`bdos/fsopnclo.c:256`).

5. **`CON:`/`AUX:`/`PRN:` are intercepted before `xopen`.** The trap dispatcher
   matches these six exact strings (upper/lower) and returns the BIOS handles
   `0xFFFF`/`0xFFFE`/`0xFFFD` directly (`bdos/bdosmain.c:103-108,651-663`).
   Mixed case (`Con:`) is **not** recognised and falls through to a filesystem
   open.

6. **Shared DFD for already-open files.** A second `Fopen` of the same file
   shares one in-memory DFD across all its OFDs, so length/time stay coherent
   (`bdos/fsopnclo.c:290-319`).

---

## Dependencies

### Requires (must be initialized / functional)

- FAT subsystem logged in (`findit`/`scan`, DMD/DND for the drive).
- A free `sft[]` slot; buffer cache + `Rwabs`.

### Used by

- Every program reading disk files; AES, shells, the loader.

---

## Related data structures

- `docs/api/structures/ofd.md` — OFD/DFD/FCB, `o_mod` bit layout.
- `docs/api/gemdos/_state.md` §2 — SFT / `p_uft[]` handle model.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents `RO/WO/RW`
  access codes and sharing modes; the sharing modes are **not** accepted by
  this EmuTOS `Fopen` — see Discrepancy log.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `VALID_FOPEN_BITS = 0x07` in this build. TOS/EmuTOS with file-sharing/
  record-locking would widen this; EmuTOS `1a7484ab` does not implement it.
- `OPNFILES = 75`, `NUMHANDLES = 81`.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: SFT + `p_uft[]` model preserved; handle numbers identical. Whether
NewTOS eventually *enforces* the access code (unlike EmuTOS) is a Phase-decision
and would be a deliberate, documented divergence.
