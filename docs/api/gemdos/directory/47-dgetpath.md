# `Dgetpath` (GEMDOS 0x47)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fsdir.c:1229` (`xgetdir`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the `+1` drive convention, the output format (leading
> `\`, no drive letter, `""` at root), `EDRIVE` + empty-buffer on error, no
> media-change check. Reference: the DND-chain walk `dopath`, `dirtbl[]`.

## EmuTOS source

**Primary location:** `bdos/fsdir.c:1229` — `long xgetdir(char *buf, int drv)`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x47` =
`xgetdir`, `wparms = 3` (`bdos/bdosmain.c:267`).

**Helpers called:**
- `include/biosbind.h:29` — `Drvmap()` — BIOS 0x0A drive bitmap.
- `bdos/fsdrive.c:77` — `ckdrv(drv, FALSE)` — validate/log in the drive;
  **`FALSE` = do *not* run a media-change check**.
- `bdos/fsdir.c:1365` — `dopath(p, buf, &len)` — recursively walk the DND chain
  from root down, writing each component followed by `\`, honouring the length
  budget.
- `bdos/fsdir.c:1297` — `packit()` — format a packed 8.3 name as `NAME.EXT`
  (called by `dopath`).

**Headers / type definitions used:**
- `include/sysconf.h:51` — `LEN_ZPATH = 114` (max path incl. drive).
- `bdos/fs.h:342-345` — `DIRTBL_ENTRY`; `bdos/fs.h:206-225` — `DND` (`d_parent`,
  `d_name`).
- `include/bdosdefs.h:89` — `PD.p_curdir[26]`.

**System variables / globals read:** `run->p_curdrv` (when `drv == 0`);
`run->p_curdir[drv]`; `dirtbl[]`; `drvbits` (via `Drvmap()`).
**System variables / globals written:** the caller's `buf`.

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Dgetpath` writes the current directory of a drive, as a path string, into the
caller's buffer. `xgetdir` maps the drive argument with the **`+1` convention**:
`drv == 0` means the current drive (`run->p_curdrv`), otherwise `drv-1` is the
0-based drive (`1 = A:`). It then validates the drive: it must be present in
`Drvmap()` **and** pass `ckdrv(drv, FALSE)`. If either fails it writes an empty
string (`*buf = 0`) and returns `EDRIVE`.

The `FALSE` argument to `ckdrv` is deliberate and documented in the source
comment (`bdos/fsdir.c:1223-1224`): unlike `Dfree`, `Dgetpath` does **not**
trigger a media-change check on removable media — matching Atari TOS, so that a
program polling `Dgetpath` for existence does not accidentally force a re-login.

On success it looks up the process's current-directory DND for that drive
(`dirtbl[run->p_curdir[drv]].dnd`) and calls `dopath`, which recurses from the
root down the DND parent chain, appending each component and a `\` separator into
`buf`, up to a budget of `LEN_ZPATH - 3` (= 111) bytes. Afterwards it overwrites
the final trailing `\` with a NUL terminator (`*--buf = 0`). The result therefore
starts with `\`, contains no drive letter, and is the **empty string** when the
current directory is the root. It returns `E_OK`.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x47
- **Stack frame on entry (m68k):** `Dgetpath(buf, drive)` =
  `trap1(0x47, buf, drive)`:
  ```
  +6: drive (word)
  +2: buf   (LONG, pointer)
   0: function_id (word) = 0x0047
  ```
- **Total parameter bytes:** 6 (LONG + WORD) → `wparms = 3`.
- **Native (AArch64) ABI:** ID `0x47` in `x8`; args in `x0..x1`; LONG result
  (`E_OK`/`EDRIVE`) sign-extended into `x0`. The path is delivered via the
  **output buffer** at `buf`, not the register.

  | m68k stack | AArch64 reg | Name  | Type / size |
  |------------|-------------|-------|-------------|
  | +2         | `x0`        | `buf` | `char *` (ptr) — output buffer, caller allocates ≥ `LEN_ZPATH` (114) |
  | +6         | `x1`        | `drv` | `WORD` (16) — `0 = current`, `1 = A:`, … |

---

## Inputs

| Position | Name  | Type    | Size | Source location     | Notes |
|----------|-------|---------|------|---------------------|-------|
| arg0     | `buf` | `char *`| 4    | `bdos/fsdir.c:1229` | output buffer; caller allocates at least `LEN_ZPATH` (114) bytes; **not validated** |
| arg1     | `drv` | `int`   | 2    | `bdos/fsdir.c:1235` | `0 = current drive`, else `drv-1` = 0-based drive (`1 = A:`) |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** `E_OK` (0) on success; `EDRIVE` (-46) on a bad/absent drive.
- **Source location:** `bdos/fsdir.c:1249` (`return E_OK`), `:1240`
  (`return EDRIVE`).

### Output buffer (`buf`)

- On success: the current directory of the drive as a path string — **leading
  `\`, no drive letter**, `\`-separated components, e.g. `\AUTO\TOOLS`. The
  **root** directory yields the **empty string** `""` (NUL at offset 0).
- On error: `*buf = 0` (empty string), so a program that prints the buffer sees
  `""` rather than garbage.
- Byte budget: `dopath` writes at most `LEN_ZPATH - 3` = 111 bytes before the
  terminator; a path that would exceed the budget is truncated (see edge case 5).

### Side effects on global state

- `ckdrv(drv, FALSE)` may lazily log in a not-yet-accessed drive (populates
  `drvtbl[]` / a `dirtbl[]` root slot). It does **not** run a media-change check.

### Side effects on hardware

- May read directory sectors when logging in the drive; no media probe.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| Drive not present in `Drvmap()`, **or** `ckdrv(drv, FALSE)` fails | `bdos/fsdir.c:1237-1240` | `EDRIVE` (-46) |

On this error `*buf` is set to 0 (empty string) before returning.

**Error paths not in Compendium:** the deliberate empty-string write on error;
the `EDRIVE` (rather than `EPTHNF`) return; the explicit *absence* of a
media-change check. Compendium documents `EDRIVE` but not the empty-buffer or
no-mediach guarantees.

---

## Verification criteria

- **Nominal:** after `Dsetpath("C:\\AUTO\\TOOLS")`, `Dgetpath(buf, 3)` returns
  `E_OK` and `buf == "\\AUTO\\TOOLS"` (leading backslash, no `C:` prefix).
- **Current drive:** `Dgetpath(buf, 0)` returns the current directory of
  `Dgetdrv()`'s drive.
- **Root → empty string:** with C:'s curdir at the root, `Dgetpath(buf, 3)`
  returns `E_OK` and `buf[0] == 0`.
- **Error — absent drive:** `Dgetpath(buf, 20)` (T:) on a system without it
  returns `EDRIVE` (-46) and sets `buf[0] == 0`.
- **No media change:** repeatedly calling `Dgetpath` on a removable drive does
  not trigger a re-login (contrast `Dfree`).

---

## Edge cases discovered in source

1. **`+1` drive convention.** `drv = 0` is current; `drv = 1` is A:. Same as
   `Dfree`; opposite of `Dsetdrv`/`Dgetdrv`. (`bdos/fsdir.c:1235`)

2. **No drive letter in the output.** The path starts with `\` and lists
   `\`-separated components; the drive is implicit. To build a fully-qualified
   path, the caller prepends `"X:"` itself. (`bdos/fsdir.c:1246-1247`)

3. **Root → empty string.** At the root `dopath` writes nothing and the NUL lands
   at offset 0, so `buf == ""`. Callers must distinguish `""` (root) from a
   non-empty path. (`bdos/fsdir.c:1244-1247`)

4. **No media-change check.** `ckdrv(drv, FALSE)` — the `FALSE` skips the
   `Mediach` probe, matching Atari TOS and called out in the source comment.
   (`bdos/fsdir.c:1223-1224,1237`)

5. **Length budget is `LEN_ZPATH-3` = 111, and truncation is silent.** `dopath`
   decrements a `len` budget as it writes; when exhausted it stops. An
   over-length path is truncated with no error. The caller must supply a buffer
   of at least `LEN_ZPATH` (114) bytes. (`bdos/fsdir.c:1245`,
   `bdos/fsdir.c:1376-1386`) (Earlier drafts said `LEN_ZPATH = 128` — it is 114.
   See D-025.)

6. **Both `Drvmap()` bit *and* `ckdrv` must pass.** A drive present in the bitmap
   but failing `ckdrv` (e.g. unreadable media) still returns `EDRIVE`.
   (`bdos/fsdir.c:1237`)

7. **Per-process result.** `run->p_curdir[drv]` is per-PD; each process sees its
   own current directory. (`bdos/fsdir.c:1243`)

8. **No buffer validation.** `buf` is not checked; a bad pointer faults on the
   store. NewTOS should validate the user pointer at trap entry.

---

## Dependencies

### Requires (must be initialized / functional)

- BIOS `Drvmap` (0x0A); `ckdrv`; the DND tree for the drive.
- `run->p_curdir[drv]` and the shared `dirtbl[]`.

### Used by

- Shells building the prompt, the AES title bar, any program needing the cwd for
  relative-path resolution.

---

## Related data structures

- `docs/api/gemdos/_state.md` §5 — the per-process current-path model
  (`PD.p_curdir[]`) and `dirtbl[]`.
- `docs/api/gemdos/directory/3b-dsetpath.md` — sets what this reads.
- `docs/api/gemdos/drive/19-dgetdrv.md` — the drive letter to prepend.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents `Dgetpath`, the
  no-drive-letter format, and `EDRIVE`; the empty-buffer-on-error and
  no-mediach are TOS/EmuTOS behaviour.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `LEN_ZPATH = 114` in this build; the write budget is `111`. Callers must still
  allocate the full `LEN_ZPATH` buffer. NewTOS may add an extension returning the
  full `X:\path` form; the classic syscall keeps the no-letter format.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: identical output format (leading `\`, no drive letter, `""` at root),
`EDRIVE` + empty buffer on error, and no media-change check. The DND-chain walk
is a free NewTOS implementation choice; the string format and error behaviour are
contract.
