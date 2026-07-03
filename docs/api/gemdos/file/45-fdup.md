# `Fdup` (GEMDOS 0x45)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fshand.c:93` (`xdup`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the standard-handles-only restriction, the returned
> non-standard handle, error codes, the "no refcount bump on the shared OFD"
> quirk. Reference: `sft[]` layout.

## EmuTOS source

**Primary location:** `bdos/fshand.c:93` — `long xdup(int h)`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x45` =
`xdup`, `wparms=1` (`bdos/bdosmain.c:265`).

**Helpers called:** none (operates directly on `sft[]` and `run->p_uft[]`).

**Headers / type definitions used:**
- `bdos/fs.h:43-45` — `OPNFILES = 75`, `NUMHANDLES = 81`;
  `include/bdosdefs.h:61` — `NUMSTD = 6`.
- `bdos/fs.h:307-312` — `FTAB` (`f_ofd`, `f_own`, `f_use`).

**System variables / globals read:** `run->p_uft[h]` (the standard handle's
current mapping).
**System variables / globals written:** the newly claimed `sft[]` entry
(`f_own = run`, `f_ofd`, `f_use = 1`).

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Fdup` duplicates a **standard** handle (0..5) into a fresh non-standard handle
(>= 6), returning the new handle. Unlike POSIX `dup`, it rejects any handle
outside `0..NUMSTD-1` with `EIHNDL` — its purpose is to snapshot the current
mapping of stdin/stdout/etc. into a stable handle that survives redirection.

It scans `sft[]` for the first entry with no owner; if none is free it returns
`ENHNDL`. It claims that entry for the current process (`f_own = run`) and then
copies the mapping: if the standard handle currently points at a **non-BIOS**
handle (`p_uft[h] > 0`, i.e. a file), it copies that entry's OFD pointer, so the
duplicate shares the same OFD (and thus the same file position); if the standard
handle points at a BIOS character device (`p_uft[h] <= 0`, negative), it stores
that negative BIOS handle in `f_ofd`. It sets `f_use = 1` and returns
`i + NUMSTD`.

A subtle, deliberately-preserved quirk: when the duplicate shares a file OFD, it
does **not** increment the source SFT entry's `f_use`. So closing the original
handle can free the OFD while the duplicate still points at it — a dangling
reference. Well-behaved programs close the duplicate first. (Contrast `Fforce`,
which *does* bump `f_use` when it maps a standard handle to a file.)

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x45
- **Stack frame on entry (m68k):** `Fdup(handle)`:
  ```
  +2: handle (word)
   0: function_id (word) = 0x0045
  ```
- **Total parameter bytes:** 2 (WORD) → `wparms = 1`.
- **Native (AArch64) ABI:** ID `0x45` in `x8`; `h` in `x0`; LONG result
  sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | +2         | `x0`        | `h`  | `WORD` (16) — must be a standard handle 0..5 |

---

## Inputs

| Position | Name | Type  | Size | Source location   | Notes |
|----------|------|-------|------|-------------------|-------|
| arg0     | `h`  | `int` | 2    | `bdos/fshand.c:93`| standard handle 0..5; anything else → `EIHNDL` |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** the new non-standard handle `i + NUMSTD` (>= 6) on success;
  negative error otherwise.
- **Source location:** `bdos/fshand.c:122`.

### Side effects on global state

- Claims one `sft[]` entry for `run`; `f_use = 1` on the new entry
  (`bdos/fshand.c:107-120`). **Does not** change the source entry's `f_use`.

### Side effects on hardware

- None.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `h < 0` or `h >= NUMSTD` (not a standard handle) | `bdos/fshand.c:97-98` | `EIHNDL` (-37) |
| No free `sft[]` entry | `bdos/fshand.c:104-105` | `ENHNDL` (-35) |

---

## Verification criteria

- **Nominal:** `Fdup(1)` returns a handle `>= 6` that maps to whatever handle 1
  currently points at.
- **Reject non-standard:** `Fdup(6)` returns `EIHNDL` (-37).
- **Reject negative:** `Fdup(-1)` returns `EIHNDL` (-37).
- **Exhaustion:** with all 75 SFT slots owned, `Fdup(1)` returns `ENHNDL` (-35).
- **Save/redirect idiom:** `saved = Fdup(1); Fforce(1, dev); …; Fforce(1, saved);
  Fclose(saved);` restores the original stdout mapping.

---

## Edge cases discovered in source

1. **Standard handles only.** Non-standard handles are rejected (`EIHNDL`) —
   this is not POSIX `dup` (`bdos/fshand.c:97-98`).

2. **Shared OFD without refcount bump.** When the standard handle maps to a file
   (`p_uft[h] > 0`), `Fdup` copies the OFD pointer but leaves the source's
   `f_use` untouched (`bdos/fshand.c:115-118`). Closing the original then frees
   the OFD, dangling the duplicate. Preserved TOS quirk; close the duplicate
   first.

3. **BIOS device pass-through.** If the standard handle maps to a BIOS device
   (`p_uft[h] <= 0`), the negative handle is stored verbatim in `f_ofd`
   (`bdos/fshand.c:118`); the duplicate then does BIOS char-device I/O.

4. **New entry has `f_use = 1`.** A single `Fclose` releases the duplicate
   (`bdos/fshand.c:120`).

5. **Owned by the current process.** `f_own = run` (`bdos/fshand.c:107`), so
   process termination auto-closes the duplicate.

---

## Dependencies

### Requires (must be initialized / functional)

- `sft[]` with a free slot; `run->p_uft[]`.

### Used by

- Shells and AES implementing stdin/stdout redirection; the classic
  save-then-restore idiom around `Fforce`.

---

## Related data structures

- `docs/api/gemdos/_state.md` §2 — SFT / `p_uft[]` model.
- `docs/api/structures/ofd.md` — OFD sharing and `f_use`/`o_usecnt`.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the
  standard-handles-only restriction.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `OPNFILES = 75`, `NUMSTD = 6`, `NUMHANDLES = 81` — corrected from an earlier
  draft that used 34/40.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05/06: identical SFT model. The "shared OFD without refcount bump"
dangling behaviour is a deliberate bug-compat item for m68k binaries; native
source ports should follow the close-duplicate-first idiom.
