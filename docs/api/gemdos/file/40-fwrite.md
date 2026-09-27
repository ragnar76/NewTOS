# `Fwrite` (GEMDOS 0x40)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fsio.c:528` (`xwrite`) and the helper
> `bdos/fsio.c:554` (`ixwrite`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, "actual bytes written" return, short-write = disk
> full, write-to-read-only allowed, error codes. Reference: `xrw`/`nextcl`
> cluster allocation.

## EmuTOS source

**Primary location:** `bdos/fsio.c:528` — `long xwrite(int h, long len, void *ubufr)`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x40` =
`xwrite`, `stdio_typ = 0x82`, `wparms=5` (`bdos/bdosmain.c:260`). As with
`Fread`, the `0x82` flag is **inert**: `0x40` is outside the redirection ranges
(`fn < 12` / `16..19`, `bdos/bdosmain.c:498`), so `Fwrite` is never intercepted.

**Helpers called:**
- `bdos/fsmain.c:474` — `getofd()`.
- `bdos/fsio.c:554` — `ixwrite()` → `bdos/fsio.c:208` — `xrw(1, …)` — the
  unified transfer, allocating clusters with `nextcl` when writing past EOF.
- `bdos/fsfat.c` — `nextcl()` — extend the FAT chain.

**Headers / type definitions used:** `bdos/fs.h` — `OFD`, `DFD`, `O_DIRTY`.

**System variables / globals read:** `run`; OFD position; `dfd->o_fileln`.
**System variables / globals written:** OFD position fields; `dfd->o_fileln`
grows and `dfd->o_flag |= O_DIRTY` when the file extends (`bdos/fsio.c:42-46`).

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Fwrite` transfers up to `len` bytes from `ubufr` to the file at handle `h`,
starting at the current position, and returns the number of bytes actually
written. `xwrite` looks up the OFD (invalid → `EIHNDL`) and — unlike `ixread`
— calls `ixwrite` unconditionally, which forwards to `xrw(1, …)`. `xrw` walks
the cluster chain, calling `nextcl` to allocate fresh clusters whenever the
write runs past the currently allocated end. As bytes are written past the old
file length, `addit` grows `dfd->o_fileln` and sets `O_DIRTY`, so the new size
and time are flushed by `Fclose`.

The return value is `o_bytnum - bytpos` (bytes moved). A return **less than
`len`** signals a short write, typically because the disk is full: `xrw` stops
when `nextcl` can no longer allocate a cluster and returns the partial count.
There is no dedicated "disk full" error code from `Fwrite`; the caller must
compare `ret == len`.

There is no explicit `len <= 0` guard (`ixwrite` has none, unlike `ixread`);
`xrw` with `len == 0` simply moves nothing and returns `0`. A source comment
(`bdos/fsio.c:536-538`) documents that EmuTOS stopped rejecting writes to a
read-only-*opened* handle for Atari-TOS compatibility — the `o_mod` access code
is not enforced. Physical write faults (write-protect, media error) come back
through the BIOS `longjmp` path, not as an inline `xwrite` return.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x40
- **Stack frame on entry (m68k):** `Fwrite(handle, count, buf)` =
  `trap1(0x40, handle, count, buf)` (`include/bdosbind.h:41`):
  ```
  +8: buf    (LONG, pointer)
  +4: count  (LONG)
  +2: handle (word)
   0: function_id (word) = 0x0040
  ```
- **Total parameter bytes:** 10 (WORD + LONG + LONG) → `wparms = 5`.
- **Native (AArch64) ABI:** ID `0x40` in `x8`; args in `x0..x2`; LONG result
  (bytes written or negative error) sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name    | Type / size |
  |------------|-------------|---------|-------------|
  | +2         | `x0`        | `h`     | `WORD` (16) |
  | +4         | `x1`        | `len`   | `LONG` (32) |
  | +8         | `x2`        | `ubufr` | `void *` (ptr) |

---

## Inputs

| Position | Name    | Type    | Size | Source location    | Notes |
|----------|---------|---------|------|--------------------|-------|
| arg0     | `h`     | `int`   | 2    | `bdos/fsio.c:528`  | file handle |
| arg1     | `len`   | `long`  | 4    | `bdos/fsio.c:554`  | bytes to write; `<= 0` transfers nothing |
| arg2     | `ubufr` | `void *`| 4    | `bdos/fsio.c:528`  | source buffer; not validated |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** bytes actually written (`0 .. len`) on success; negative error.
- **Source location:** `bdos/fsio.c:281-283` (`xrw` returns `o_bytnum - bytpos`).

### Side effects on global state

- OFD position advances; `dfd->o_fileln` grows and `O_DIRTY` is set on extend
  (`bdos/fsio.c:42-46`). New clusters are linked into the FAT chain.

### Side effects on hardware

- Sector reads (for partial head/tail sectors) and writes via the buffer cache.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| Invalid handle (`getofd` NULL) | `bdos/fsio.c:540-543` | `EIHNDL` (-37) |
| Disk full (cannot allocate cluster) | `bdos/fsio.c` (`xrw` stops, `nextcl` fails) | **short count**, not an error code |
| Write-protect / media fault | via `longjmp_rwabs` (`bdos/fs.h:60`) | BIOS code (e.g. `EWRPRO` -13, `EWRITF` -10) |

**Error paths not in Compendium:** disk-full is reported as a **short byte
count**, never as a negative code, from `Fwrite` itself.

---

## Verification criteria

- **Nominal:** `Fwrite(h, 100, buf)` on a file with room returns 100 and
  extends the file; a following `Fclose` persists the new length.
- **Extend past EOF:** writing beyond the current end allocates clusters and
  grows `o_fileln`.
- **Short write on full disk:** on a nearly full volume, `Fwrite(h, N, buf)`
  returns `< N`; the caller detects disk-full by `ret != N`.
- **Zero len:** `Fwrite(h, 0, buf)` returns 0, no side effects.
- **Write to read-only-opened handle succeeds** (mode not enforced).
- **Error:** `Fwrite(9999, 1, buf)` returns `EIHNDL` (-37).

---

## Edge cases discovered in source

1. **Disk full is a short count, not an error.** `xrw` returns the bytes it
   managed to write; subsequent writes at the full boundary return 0
   (`bdos/fsio.c`).

2. **No explicit `len <= 0` guard.** Unlike `ixread`, `ixwrite` passes straight
   to `xrw`; `len == 0` moves nothing and yields `0` (`bdos/fsio.c:554-557`).

3. **Read-only-opened handles are writable.** The `o_mod` access code is not
   enforced; documented Atari-TOS compatibility (`bdos/fsio.c:536-538`).

4. **Length update is in-memory until flush.** `o_fileln` and time are written
   to the directory entry only by `Fclose` (or an internal buffer flush) — a
   crash before close loses the size update.

5. **No truncate syscall.** GEMDOS has no "set length" call; shrinking a file
   requires `Fcreate` (which deletes + recreates empty). Writing 0 bytes does
   **not** truncate.

6. **Shared-handle position hazard.** As with `Fread`, handles sharing one OFD
   share `o_bytnum`; concurrent writes must be serialised by the caller.

---

## Dependencies

### Requires (must be initialized / functional)

- A valid OFD; FAT free-cluster state; buffer cache + `Rwabs`.

### Used by

- Every program that writes files; the loader when creating files; AES.

---

## Related data structures

- `docs/api/structures/ofd.md` — OFD/DFD position + length fields, `O_DIRTY`.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the
  `(handle, count, buf)` ABI and "returns bytes written / short = disk full".)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- None; behaviour independent of build flags.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: FAT write path with explicit cluster allocation. NewTOS may add an
`Fsync`-style extension later; existing programs rely on `Fclose` for flush.

HW03 (2026-09-27): the hard-error path is wired. EmuTOS reaches it with
`longjmp_rwabs` (`bdos/fs.h:60`), which cannot be reproduced in Rust; instead
the sector cache records the BIOS code of a failed access
(`tos-api/src/bdos/fs/cache.rs`, the reimplementation of EmuTOS' `rwerr` /
`errcode` globals) and `fwrite` returns it. The distinction the table above
draws is therefore live: a **full disk** is still a short byte count, a
**device error** is `EWRITF` (-10) / `EDRVNR` (-2) / `ESECNF` (-8) — whatever
the driver reported. Driven by `tos-api/tests/gemdos_fs_device_error.rs`.
Before HW03, `Fwrite` returned the full byte count for data that never left
the machine (`docs/api/hw/emmc-audit.md` finding 5).
