# `Fseek` (GEMDOS 0x42)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/fsio.c:321` (`xlseek`) and the helper
> `bdos/fsio.c:354` (`ixlseek`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI (**offset is the LONG, passed first**), the three
> `flg` modes, the returned absolute position, the `ERANGE`/`EINVFN` errors.
> Reference: the cluster-chain walk + the "one cluster before boundary" trick.

## EmuTOS source

**Primary location:** `bdos/fsio.c:321` — `long xlseek(long n, int h, int flg)`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x42` =
`xlseek`, `stdio_typ = 0x81`, `wparms=4` (`bdos/bdosmain.c:262`). The `0x81`
flag is **inert** for `Fseek`: `0x42` is outside the redirection ranges
(`fn < 12` / `16..19`, `bdos/bdosmain.c:498`), so no standard-handle
interception occurs.

**Helpers called:**
- `bdos/fsmain.c:474` — `getofd()`.
- `bdos/fsio.c:354` — `ixlseek()` — the FAT-walk positioner.
- `bdos/fsfat.c` — `getclnum()`, `cl2rec()`, `endofchain()`.

**Headers / type definitions used:** `bdos/fs.h` — `OFD`, `DFD`, `DMD`.

**System variables / globals read:** OFD position (`o_bytnum`, `o_curcl`),
`dfd->o_fileln`.
**System variables / globals written:** OFD position fields
(`o_curcl`/`o_currec`/`o_bytnum`/`o_curbyt`, `bdos/fsio.c:420-423`).

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Fseek` repositions the read/write pointer of an open handle and returns the
new absolute byte offset. Note the parameter order: the **LONG offset comes
first** on the stack, then the WORD handle, then the WORD mode — this is the
GEMDOS ABI order (`Fseek(offset, handle, seekmode)`, `include/bdosbind.h:43`),
which is the reverse of the C prototype's readability.

`xlseek` looks up the OFD (invalid → `EIHNDL`), then folds `flg` into an
absolute target: `flg == 2` (`SEEK_END`) adds `o_fileln`, `flg == 1`
(`SEEK_CUR`) adds the current `o_bytnum`, `flg == 0` (`SEEK_SET`) uses `n`
as-is, and **any other value returns `EINVFN`**. `ixlseek` rejects the target
if `n < 0` or `n > o_fileln` (`ERANGE`) — TOS files are dense, so you cannot
seek past EOF to create a hole; extending a file is done with `Fwrite`.

`ixlseek` walks the FAT chain to the cluster containing the target. It can
chain forward from the current cluster when the target is at or beyond the
current position (an optimisation for streaming); otherwise it restarts from the
file's start cluster. If the target is exactly on a cluster boundary it
deliberately positions one cluster *before* it, because `xrw` always advances to
the next cluster before its first transfer. If the FAT chain is shorter than the
file length claims, it returns `EINTRN` (a filesystem-corruption signal).

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x42
- **Stack frame on entry (m68k):** `Fseek(offset, handle, seekmode)` =
  `trap1(0x42, offset, handle, seekmode)` (`include/bdosbind.h:43`):
  ```
  +8: seekmode (word)
  +6: handle   (word)
  +2: offset   (LONG)
   0: function_id (word) = 0x0042
  ```
- **Total parameter bytes:** 8 (LONG + WORD + WORD) → `wparms = 4`.
- **Native (AArch64) ABI:** ID `0x42` in `x8`; args in `x0..x2` in the
  GEMDOS order (offset first); LONG result (new position or negative error)
  sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name  | Type / size |
  |------------|-------------|-------|-------------|
  | +2         | `x0`        | `n`   | `LONG` (32) — offset |
  | +6         | `x1`        | `h`   | `WORD` (16) — handle |
  | +8         | `x2`        | `flg` | `WORD` (16) — mode |

---

## Inputs

| Position | Name  | Type   | Size | Source location    | Notes |
|----------|-------|--------|------|--------------------|-------|
| arg0     | `n`   | `long` | 4    | `bdos/fsio.c:321`  | signed offset, relative to `flg` |
| arg1     | `h`   | `int`  | 2    | `bdos/fsio.c:325`  | file handle |
| arg2     | `flg` | `int`  | 2    | `bdos/fsio.c:329`  | 0 = SET, 1 = CUR, 2 = END; else `EINVFN` |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG
- **Encoding:** new absolute byte position (`>= 0`) on success; negative error.
- **Source location:** `bdos/fsio.c:425` (`ixlseek` returns `n`).

### Side effects on global state

- Updates the OFD position fields (`o_curcl`, `o_currec`, `o_bytnum`,
  `o_curbyt`; `bdos/fsio.c:420-423`).

### Side effects on hardware

- May read FAT sectors while walking the chain (through the buffer cache).

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| Invalid handle (`getofd` NULL) | `bdos/fsio.c:325-327` | `EIHNDL` (-37) |
| `flg` not in {0, 1, 2} | `bdos/fsio.c:333-334` | `EINVFN` (-32) |
| Resulting position `< 0` or `> o_fileln` | `bdos/fsio.c:360-361` | `ERANGE` (-64) |
| FAT chain shorter than `o_fileln` (corruption) | `bdos/fsio.c:416-417` | `EINTRN` (-65) |

**Error paths not in Compendium:** the `EINTRN` corruption path; and note the
invalid-mode error is `EINVFN` (-32), not a POSIX-style `EINVAL`.

---

## Verification criteria

- **SEEK_SET nominal:** `Fseek(10, h, 0)` on a ≥10-byte file returns 10.
- **SEEK_END size probe:** `Fseek(0, h, 2)` returns the file length.
- **SEEK_CUR:** after reading 5 bytes, `Fseek(3, h, 1)` returns 8.
- **Past-EOF rejected:** `Fseek(o_fileln + 1, h, 0)` returns `ERANGE` (-64).
- **Negative rejected:** `Fseek(-1, h, 0)` returns `ERANGE` (-64).
- **Bad mode:** `Fseek(0, h, 3)` returns `EINVFN` (-32).
- **Bad handle:** `Fseek(0, 9999, 0)` returns `EIHNDL` (-37).

---

## Edge cases discovered in source

1. **Cannot seek past EOF.** `n > o_fileln` → `ERANGE` (`bdos/fsio.c:360`). TOS
   has no sparse files; grow a file with `Fwrite`, not `Fseek`.

2. **`Fseek(0, h, 2)` returns the size.** The `SEEK_END`+0 idiom is how programs
   read a file's length.

3. **Invalid mode is `EINVFN`.** Not `EINVAL` (`bdos/fsio.c:333-334`).

4. **`n == 0` fast-path.** Seeking to absolute 0 resets all position fields to 0
   without any FAT walk (`bdos/fsio.c:363-367`).

5. **Cluster-boundary "one before" trick.** A target on a cluster boundary is
   positioned one cluster earlier (`clnum--`) so that `xrw`'s next-cluster-first
   logic lands correctly (`bdos/fsio.c:410-411`). Internal, invisible to
   callers, but it is why `Fseek` and `xrw` must stay in sync.

6. **Forward-chaining optimisation.** If the target is `>=` the current position
   and a cluster is loaded, the walk continues from the current cluster instead
   of the start cluster (`bdos/fsio.c:378-396`).

7. **Mode not consulted.** Seeking a read-only-opened handle is allowed.

---

## Dependencies

### Requires (must be initialized / functional)

- A valid OFD; the FAT chain; DMD geometry (`m_clblog`, `m_clbm`).

### Used by

- Every program doing random-access I/O; `Fattrib`/`Fdatime` internally seek the
  directory OFD; archivers, databases.

---

## Related data structures

- `docs/api/structures/ofd.md` — the four OFD position fields kept in sync.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents `flg` 0/1/2 and
  the `(offset, handle, mode)` ABI order.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- None.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-05: same OFD model. The "one cluster before boundary" positioning is tied
to the `xrw` transfer loop; NewTOS may restructure both together but must keep
the observable `Fseek` return (absolute position) and error set identical.

FS08 (2026-10-07): `Fseek` now positions exactly as `ixlseek` does
(`_state.md` §3a): forward chaining from `o_curcl`, otherwise from the start
cluster, one cluster short on a boundary, `EINTRN` for a chain shorter than the
file length claims. Before FS08 `Fseek` only stored the byte position and every
`Fread`/`Fwrite` walked the chain from the start; the `EINTRN` row above was
not reachable.
