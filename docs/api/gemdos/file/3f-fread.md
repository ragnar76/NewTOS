# `Fread` (GEMDOS 0x3F)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fsio.c:440` (`xread`) and the helper
> `bdos/fsio.c:498` (`ixread`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, "actual bytes read" return, EOF-truncation,
> read-of-write-only allowed, error codes. Reference: `xrw`/buffer-cache
> cluster walk.

## EmuTOS source

**Primary location:** `bdos/fsio.c:440` — `long xread(int h, long len, void *ubufr)`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x3F` =
`xread`, `stdio_typ = 0x82`, `wparms=5` (`bdos/bdosmain.c:259`). The `0x82`
flag is **inert** for `Fread`: the dispatcher's standard-handle redirection
only fires for `fn < 12` or `16 <= fn <= 19` (`bdos/bdosmain.c:498`), and
`0x3F` is outside both ranges — so `Fread` is never intercepted and reaches
`xread` directly.

**Helpers called:**
- `bdos/fsmain.c:474` — `getofd()` — handle → OFD.
- `bdos/fsio.c:498` — `ixread()` — EOF-clamp `len`, then call `xrw(0, …)`.
- `bdos/fsio.c:208` — `xrw()` — unified read/write cluster walk through the
  buffer cache; `bdos/fsio.c:57` — `usrio()` → `longjmp_rwabs` (`Rwabs`).

**Headers / type definitions used:** `bdos/fs.h` — `OFD`, `DFD`, `DMD`.

**System variables / globals read:** `run` (via `getofd`); `p->o_bytnum`,
`p->o_dfd->o_fileln`.
**System variables / globals written:** the OFD position fields (`o_bytnum`,
`o_curcl`, `o_currec`, `o_curbyt`) advance as bytes are copied.

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Fread` transfers up to `len` bytes from the file at handle `h` into `ubufr`,
starting at the file's current position, and returns the number of bytes
actually read. `xread` looks up the OFD (invalid handle → `EIHNDL`). `ixread`
clamps `len` to `o_fileln - o_bytnum` (the bytes remaining before EOF); if that
is `<= 0` it returns `0L` immediately. Otherwise it calls `xrw(0, …)`, which
walks the FAT cluster chain, moving whole logical sectors through the buffer
cache where possible and byte-copying the head/tail fragments, and returns the
byte count transferred.

The number returned may be less than `len` without being an error — that is how
callers detect EOF (`ret < len`, or `ret == 0` at/after EOF). EmuTOS does not
distinguish "reached EOF" from "read zero bytes" — both surface as `0`.

A source comment (`bdos/fsio.c:503-505`) records that EmuTOS used to reject
reads from a write-only handle but stopped, for Atari-TOS compatibility: the
`o_mod` access code is **not** consulted by `Fread`. Disk errors during the
transfer are raised via the BIOS `longjmp` path (see the dispatcher spec),
not returned inline by `xread`.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x3F
- **Stack frame on entry (m68k):** `Fread(handle, count, buf)` =
  `trap1(0x3f, handle, count, buf)` (`include/bdosbind.h:40`):
  ```
  +8: buf    (LONG, pointer)
  +4: count  (LONG)
  +2: handle (word)
   0: function_id (word) = 0x003F
  ```
- **Total parameter bytes:** 10 (WORD + LONG + LONG) → `wparms = 5`.
- **Native (AArch64) ABI:** ID `0x3F` in `x8`; args in `x0..x2`; LONG result
  (bytes read or negative error) sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name    | Type / size |
  |------------|-------------|---------|-------------|
  | +2         | `x0`        | `h`     | `WORD` (16) |
  | +4         | `x1`        | `len`   | `LONG` (32) |
  | +8         | `x2`        | `ubufr` | `void *` (ptr) |

---

## Inputs

| Position | Name    | Type    | Size | Source location    | Notes |
|----------|---------|---------|------|--------------------|-------|
| arg0     | `h`     | `int`   | 2    | `bdos/fsio.c:440`  | file handle |
| arg1     | `len`   | `long`  | 4    | `bdos/fsio.c:498`  | max bytes; clamped to bytes-to-EOF; `<= 0` ⇒ 0 |
| arg2     | `ubufr` | `void *`| 4    | `bdos/fsio.c:440`  | destination buffer; no alignment or bounds check |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** bytes actually read (`0 .. len`) on success; negative error.
- **Source location:** `bdos/fsio.c:281-283` (`xrw` returns `o_bytnum - bytpos`).

### Side effects on global state

- Advances the OFD position fields (`bdos/fsio.c:33-47` `addit`).

### Side effects on hardware

- Sector reads via the buffer cache / `Rwabs` for any cluster not already cached.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| Invalid handle (`getofd` NULL) | `bdos/fsio.c:445-449` | `EIHNDL` (-37) |
| Underlying disk read failure | via `longjmp_rwabs` (`bdos/fs.h:60`) | BIOS code (e.g. `EREADF` -11) |

**Error paths not in Compendium:** none new; note `len <= 0` is **not** an
error (returns `0`), and reading at/after EOF returns `0`, not an error.

---

## Verification criteria

- **Nominal:** on a 100-byte file at position 0, `Fread(h, 100, buf)` returns
  100 and fills `buf`.
- **Partial at EOF:** at position 90 of that file, `Fread(h, 100, buf)` returns
  10 and leaves the position at EOF.
- **At EOF:** a further `Fread(h, 100, buf)` returns 0.
- **Zero/negative len:** `Fread(h, 0, buf)` and `Fread(h, -5, buf)` return 0
  with no transfer.
- **Read of write-only handle succeeds** (mode not enforced).
- **Error:** `Fread(9999, 1, buf)` (no such open handle) returns `EIHNDL` (-37).

---

## Edge cases discovered in source

1. **EOF truncation is silent.** `len` is clamped to `o_fileln - o_bytnum`;
   there is no short-read error (`bdos/fsio.c:507-513`).

2. **`len <= 0` returns 0.** The `if (len > 0)` gate means zero and negative
   requests transfer nothing (`bdos/fsio.c:510-513`).

3. **Write-only handles are readable.** The `o_mod` access code is not checked;
   a comment documents the deliberate Atari-TOS compatibility choice
   (`bdos/fsio.c:503-505`).

4. **BIOS char devices are not readable via `Fread`.** For `h < 0`, `getofd`
   returns NULL → `EIHNDL`; console/serial input is via `Cconin`/`Bconin`,
   not `Fread` (this build has no MiNT `Fread(-1)` bridge).

5. **No buffer validation.** `ubufr` is trusted; no alignment or region check
   (`bdos/fsio.c` uses `memcpy` into it).

6. **Shared-handle position hazard.** Two handles onto the same OFD (via
   `Fdup`/`Fforce`) share `o_bytnum`; interleaved reads corrupt each other's
   position. Single-tasking makes this the caller's responsibility.

---

## Dependencies

### Requires (must be initialized / functional)

- A valid OFD (from `Fopen`/`Fcreate`); the FAT cluster chain; buffer cache.

### Used by

- Every program that reads files; the loader; AES resource loading.

---

## Related data structures

- `docs/api/structures/ofd.md` — OFD position fields, DFD `o_fileln`.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the
  `(handle, count, buf)` ABI and "returns bytes read".)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- None; behaviour independent of build flags.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: backed by the FAT driver. On AArch64 the `[ubufr, ubufr+len)` range
is validated once at the trap boundary; the mode-not-enforced behaviour is
contract for source compatibility.

HW03 (2026-09-27): a read that stops because the **device** failed returns the
BIOS code (`EREADF` -11, or `EDRVNR` / `ESECNF` as the driver reported) rather
than a short count; end-of-file and a chain that ends early stay short counts.
Same mechanism as `40-fwrite.md`; driven by
`tos-api/tests/gemdos_fs_device_error.rs`.

FS08 (2026-10-07): the transfer follows EmuTOS `xrw` as extracted in
[`_state.md` §3a](../_state.md): the OFD keeps `o_curcl`/`o_currec`/`o_curbyt`,
so a sequential `Fread` steps the FAT chain one link per new cluster instead
of walking it from the file start on every call; whole records go in one block
request straight into the caller's buffer (contiguous clusters merged up to
`CNTMAX` records), only the partial head/tail records pass through the data
buffer list. Unobservable through the API (same bytes, same return values);
measured effect in `measurements/measurement-2026-10-07-fat-curcl/`.
