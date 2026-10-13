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
flag is **not inert** (correction 2026-10-13, see "Character-device and
standard handles" below): it is outside the *stdio redirection block*
(`fn < 12` / `16..19`, `bdos/bdosmain.c:498`), but bit 7 sends it into the
*handle-resolution block* (`bdos/bdosmain.c:569-641`), which runs **before**
`xlseek` and answers character-device handles itself.

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

## Character-device and standard handles (CORRECTION 2026-10-13)

Earlier text of this spec said "no standard-handle interception". That was
wrong for the handle→character-device case. Source-verified behaviour of
`osif()` for `Fseek` (all line numbers `bdos/bdosmain.c` unless noted):

1. `fn = 0x42` passes `fn <= MAX_FNCALL` (`:444`); `typ = funcs[0x42].stdio_typ
   = 0x81` (`:262`, `:496`).
2. The stdio block `(fn<12)||(16..19)` is **skipped** (`:498`; `typ` stays
   `0x81`).
3. `typ & 0x80` is set (`:571`). `typ == 0x81` → `h = pw[3]` (`:573-576`) =
   the handle word (`pw[0]`=fn, `pw[1..2]`=offset LONG, `pw[3]`=handle,
   `pw[4]`=mode). Read/Write (`0x82`) take `h = pw[1]` instead.
4. Resolution into `numl` (`:578-589`), `h` is a signed `int`:
   - `h >= NUMSTD` (6, `include/bdosdefs.h:61`): `numl = sft[h-6].f_ofd`
     (no upper bound check here; `h >= NUMHANDLES` reads past `sft` —
     EmuTOS weakness, not contract).
   - `0 <= h < 6`: `u = run->p_uft[h]`; `u > 0` → `numl = sft[u-6].f_ofd`
     (handle was `Fforce`d to a file/dup), else `numl = u` (BIOS handle).
   - `h < 0`: `numl = h` (the negative handle itself is the device).
5. `numl == 0` → **`EIHNDL`** (`:592-593`; free sft slot, or `p_uft[h]==0`).
6. `numl < 0` (character device; `-1`=CON, `-2`=AUX, `-3`=PRN,
   `bdos/console.h:23-25`): `num < -3` → **`EIHNDL`** (`:601-602`). Otherwise,
   since `fn` is neither `GEMDOS_FREAD` nor `GEMDOS_FWRITE` (`:608`, `:622`),
   control reaches **`return 0;`** (`:642`). `xlseek` is **never called**.
   The return is `0` independent of `offset` and of `flg` — including
   invalid `flg` (3, 0xFFFF…) which would be `EINVFN` on a file.
7. Otherwise (`numl > 0`: real file OFD) `xlseek` runs (`wparms==4`
   dispatch, `:665-`) with `h` as the *original* `pw[3]`; `getofd()` maps a standard
   handle through `p_uft` (`bdos/fsmain.c:459-463`, `:474-486`).

Default mapping (`bdos/console.c:60-67`, applied by `stdhdl_init` `:111-112`):
handles 0,1 → -1 (CON), 2 → -2 (AUX), 3 → -3 (PRN), 4,5 → -1 (CON).
`Fforce` (`bdos/fshand.c:46-77`) stores a negative `h` as-is, a file handle as
its positive sft handle; `Fclose` of the std handle reverts it
(`bdos/fsopnclo.c:450-455`). `Fdup` of a std handle mapped to a device stores
the negative BIOS handle in the new sft entry's `f_ofd` (`bdos/fshand.c:115-118`),
so `Fseek` on that dup'ed handle (>= 6) also returns `0` (step 4/6).

### Result table (EmuTOS @ `1a7484ab`)

| Handle `h` (word) | Mapped to | `Fseek(off, h, flg)` result | Cite |
|---|---|---|---|
| 0,1,2,3,4,5 (default) | CON/AUX/PRN device | `0`, any `off`, any `flg` | `:581-586,:642` |
| 0..5 after `Fforce(h, f)` to file | file OFD | normal `xlseek` result (position / `ERANGE` / `EINVFN`) | `:581-586`, `fsio.c:321` |
| 0..5 after `Fforce(h, -1..-3)` | BIOS device | `0` | `:586-589,:642` |
| -1, -2, -3 | CON/AUX/PRN | `0` | `:589,:642` |
| <= -4 | none | `EIHNDL` (-37) | `:601-602` |
| >= 6, open file | file | normal `xlseek` | `:578-579` |
| >= 6, from `Fdup` of a device std handle | device | `0` | `:578-579,:642`; `fshand.c:115-118` |
| >= 6, free sft slot | none | `EIHNDL` | `:592-593` |
| >= NUMHANDLES (6+OPNFILES) | — | EmuTOS: out-of-bounds read of `sft` (undefined). Contract: `EIHNDL` (as `xlseek` would via `getofd`, `fsmain.c:480-481`) | `:578-579` |

Inference (not read as code): a result of `0` on a character device is also
the *position* a terminal reports; FreeMiNT's `bios_tseek` (`sys/biosfs.c:1902-1908`)
comments "terminals always are at position 0" and returns 0. This is what makes
mintlib's `isatty`/stdio probing `Fseek(0, h, SEEK_CUR) == 0` work.

### Shared rule (what else this block covers)

The block at `bdos/bdosmain.c:571-643` applies exactly to the `funcs[]` entries
whose `stdio_typ` has bit 7 set **and** that are not consumed by the stdio
block: `0x3F Fread`, `0x40 Fwrite` (`0x82`, `:259-260`) and `0x42 Fseek`
(`0x81`, `:262`). (The other `0x80+` entries, `:156-192`, are `fn<12`/`16..19`
and end in the stdio block / `typ = 0` at `:498-562`.) Within the block only
`Fread` (`:608-620`: `pw[2]!=0` → `0`; 1 byte → `conin`; else `cgets`) and
`Fwrite` (`:622-640`: `tabout` for CON, `Bconout` for AUX/PRN) do real work;
**everything else that reaches it — i.e. `Fseek` — returns `0`** (`:642`).
`Fdatime` (`0x57`, `stdio_typ 0`, `:287`), `Fattrib` (`0x43`, path based),
`Fclose` (`:258`, own handling), `Fdup`, `Fforce` are not in this block:
`Fdatime` on a device handle goes to `xgsdtof` → `getofd` → `NULL` →
`EIHNDL` (`bdos/fsdir.c:686-690`); for std handles and negative handles this is
clean, for a `Fdup` of a device handle (>=6, negative `f_ofd`) `getofd` returns
the negative pointer value as non-NULL (`fsmain.c:486`) — EmuTOS dereferences
garbage; needs-deeper-analysis, not contract, do not replicate.
`Fclose` of a negative handle returns `E_OK` (`bdos/fsopnclo.c:432-433`).

### Cross-checks

- FreeMiNT (`references/freemint`, f906416) differs on the top-level call:
  `sys_f_seek` returns **`ESPIPE`** if `is_terminal(f)` (`sys/dosfile.c:350`, in `sys_f_seek`), whereas the device's own `tseek` (`bios_tseek`,
  `sys/biosfs.c:1902`) returns `0`. EmuTOS (the NewTOS spec authority) returns
  `0`; mintlib stdio targets the MiNT kernel/EmuTOS behaviour and
  must be tested against `0`. Cross-reference only.
- Atari Compendium / Profibuch: not in `references/` (searched); not checked.
  Open question whether TOS 1.x/2.x/3.x ROM also returns 0 — EmuTOS comment-free
  code only; `ARCH03` hardware reference run (tosdump) could confirm.

### Test cases (documented input → documented return)

| # | Setup | Call | Expected |
|---|---|---|---|
| T1 | default process | `Fseek(0, 1, 1)` | `0` |
| T2 | default | `Fseek(0, h, 1)` for h = 0,1,2,3,4,5 | `0` each |
| T3 | default | `Fseek(12345, 1, 0)`, `Fseek(-7, 1, 2)` | `0`, `0` |
| T4 | default | `Fseek(0, 1, 3)`, `Fseek(0, 1, 0xFFFF)` (invalid mode) | `0` (not `EINVFN`) |
| T5 | default | `Fseek(0, -1, 0)`, `(…, -2, …)`, `(…, -3, …)` | `0` |
| T6 | default | `Fseek(0, -4, 0)`, `Fseek(0, -32768, 0)` | `EIHNDL` |
| T7 | `Fforce(1, fh)` on a 100-byte file at pos 10 | `Fseek(0, 1, 1)` | `10` |
| T8 | as T7 | `Fseek(0, 1, 2)` / `Fseek(200, 1, 0)` / `Fseek(0, 1, 3)` | `100` / `ERANGE` / `EINVFN` |
| T9 | T7 then `Fclose(1)` | `Fseek(0, 1, 1)` | `0` (reverted to CON) |
| T10 | `d = Fdup(1)` (CON) | `Fseek(0, d, 1)` | `0` |
| T11 | after `Fclose(d)` | `Fseek(0, d, 1)` | `EIHNDL` |
| T12 | free sft handle 6+k | `Fseek(0, 6+k, 0)` | `EIHNDL` |
| T13 | default | `Fdatime(buf, 1, 0)` | `EIHNDL` (not via Fseek rule) |

Note: the handle is a 16-bit *signed* word at the trap; the AArch64 shim must
sign-extend it (NewTOS currently reads it unsigned, so -1..-3 become 65533..65535).

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
- **Bad handle:** `Fseek(0, 9999, 0)` returns `EIHNDL` (-37) (EmuTOS itself reads out of bounds here, `bdosmain.c:578-579`; contract = `EIHNDL`).
- **Character device / standard handles:** returns `0`, see the section "Character-device and standard handles" (T1-T13).

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
