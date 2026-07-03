# `Mshrink` (GEMDOS 0x4A)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/umem.c:170` (`xsetblk`) and `bdos/iumem.c:228`
> (`shrinkit`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the unused reserved `n` word, the shrink-only rule
> (grow → `EGSBF`), the `len == 0` ⇒ free equivalence, the
> `E_OK`/`EIMBA`/`EGSBF`/`ERR` result codes, alignment rounding of `len`.
> Reference: `find_mpb`, `shrinkit`, `freeit`, the MD/MPB lists.

## EmuTOS source

**Primary location:** `bdos/umem.c:170` — `long xsetblk(int n, void *blk, long len)`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x4A` =
`xsetblk`, `wparms = 5` (`bdos/bdosmain.c:270`).

**Helpers called:**
- `bdos/umem.c:99` — `find_mpb()` — pool the block lives in.
- `bdos/iumem.c:228` — `shrinkit()` — allocate a tail MD, place it on `mp_mal`,
  update the block length, then `freeit` the tail (which coalesces).
- `bdos/iumem.c:144` — `freeit()` — reached for the `len == 0` case and inside
  `shrinkit`.

**Headers / type definitions used:**
- `include/bdosdefs.h:108-115` — `MD`.
- `bdos/mem.h` — `MALLOC_ALIGN_ALTRAM = 3`; `extern ULONG malloc_align_stram`.
- `include/gemerror.h:19,20,48,60` — `E_OK = 0`, `ERR = -1`, `EIMBA = -40`,
  `EGSBF = -67`.

**System variables / globals read:**
- `malloc_align_stram` (`bdos/umem.c:200`) — ST-RAM alignment grain.
- `pmd`/`pmdalt`, `start_stram`/`end_stram` — via `find_mpb`.

**System variables / globals written:**
- The block's MD `m_length` (shrunk) and, for a real shrink, a new tail MD on
  `mp_mal` immediately freed into `mp_mfl` (coalesced) (`bdos/iumem.c:242-262`).

**Build configuration affecting this function:**
- `CONF_WITH_ALT_RAM` — only then can `find_mpb` route to `pmdalt`; the
  alt-RAM alignment path (`MALLOC_ALIGN_ALTRAM`) is otherwise unreached.

---

## Source extraction summary

`Mshrink` reduces the size of an already-allocated block, returning the freed
tail to the pool. The first argument `n` is a historical CP/M "block number"
reserved word — it is **completely unused**; the m68k bindings hard-code it and
the code never reads it.

`xsetblk` locates the block: `find_mpb(blk)` picks the pool, then a linear scan
of `mp_mal` finds the MD whose `m_start == blk` (else `EIMBA`). It rounds the
new length **up** to the pool's alignment grain — `malloc_align_stram + 1` for
ST-RAM (2 or 4 bytes) or 4 bytes for alt-RAM — then compares against the
current length:

- rounded `len` **>** current length → `EGSBF` (you cannot grow with
  `Mshrink`);
- rounded `len` **==** current length → `E_OK`, nothing to do;
- rounded `len` **== 0** (only when the caller passed 0) → treated exactly like
  `Mfree`: the whole block is `freeit`-ed and `E_OK` returned;
- otherwise → `shrinkit`.

`shrinkit` needs a fresh MD to describe the freed tail. It calls `xmgetmd`; if
the OS-memory MD pool is exhausted it returns -1 and `Mshrink` returns `ERR`
(-1) with the original block **unchanged**. On success it sets the tail MD's
`m_start = blk + newlen`, `m_length = old_length - newlen`, links it onto
`mp_mal`, shrinks the original MD's `m_length` to `newlen`, and then calls
`freeit` on the tail — which unlinks it from `mp_mal`, inserts it address-sorted
into `mp_mfl`, and coalesces with the following free block if contiguous.

Note that the tail MD's `m_own` is **not** explicitly set (it holds whatever
`xmgetmd` left), and `freeit` does not clear it either — as in `Mfree`, the free
flag is list membership, not `m_own`. The original (kept) MD retains its owner.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x4A
- **Stack frame on entry (m68k):**
  ```
  +8: len (LONG)
  +4: blk (LONG, void *)
  +2: n   (WORD, reserved/unused)
   0: function_id (word) = 0x004A
  ```
- **Total parameter bytes:** 10 (WORD + LONG + LONG) → `wparms = 5`.
- **Native (AArch64) ABI:** ID `0x4A` in `x8`; `n` in `x0`, `blk` (pointer) in
  `x1`, `len` in `x2`. LONG result (error code) sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name  | Type / size |
  |------------|-------------|-------|-------------|
  | +2         | `x0`        | `n`   | `WORD` (16) — reserved, ignored |
  | +4         | `x1`        | `blk` | `void *` (ptr) |
  | +8         | `x2`        | `len` | `LONG` (32/`long`) |

---

## Inputs

| Position | Name  | Type     | Size | Source location   | Notes |
|----------|-------|----------|------|-------------------|-------|
| arg0     | `n`   | `int`    | 2    | `bdos/umem.c:170` | reserved; **never read** |
| arg1     | `blk` | `void *` | 4    | `bdos/umem.c:177` | must equal an allocated block's `m_start` |
| arg2     | `len` | `long`   | 4    | `bdos/umem.c:200` | new length; rounded **up** to alignment; 0 ⇒ free |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG (GEMDOS error code)
- **Encoding:**

  | Case | Return |
  |------|--------|
  | Shrink succeeded | `E_OK` (0) |
  | rounded `len` == current length (no-op) | `E_OK` (0) |
  | `len == 0` (block freed like `Mfree`) | `E_OK` (0) |
  | `blk` not a block start | `EIMBA` (-40) |
  | rounded `len` > current length (grow attempt) | `EGSBF` (-67) |
  | MD pool exhausted while splitting the tail | `ERR` (-1) |

- **Source location:** `bdos/umem.c:194,211,213,217,224,228`.

### Side effects on global state

- Real shrink: original MD's `m_length` reduced; a tail MD is created, briefly
  on `mp_mal`, then moved to `mp_mfl` and coalesced (`bdos/iumem.c:242-262`).
- `len == 0`: whole MD moved from `mp_mal` to `mp_mfl` (coalesced).
- No-op / error: no change.

### Side effects on hardware

None.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `find_mpb(blk)` NULL, or no `mp_mal` MD with `m_start == blk` | `bdos/umem.c:178-194` | `EIMBA` (-40) |
| rounded `len` greater than the block's current length | `bdos/umem.c:210-211` | `EGSBF` (-67) |
| `xmgetmd` fails (MD pool exhausted) inside `shrinkit` | `bdos/iumem.c:235-240` → `bdos/umem.c:223-224` | `ERR` (-1) |

**Error paths not in Compendium:** the `ERR` (-1) return on MD-pool exhaustion
is an EmuTOS internal-resource failure not called out in typical documentation;
the block is left unmodified in that case, so a retry is safe.

---

## Verification criteria

- **Nominal shrink:** allocate `N`, `Mshrink(0, blk, N/2)` returns `E_OK`; the
  freed tail becomes available (a `Malloc(N/2)`-sized request can reuse it).
- **No-op shrink:** `Mshrink(0, blk, N)` (same size, after rounding) returns
  `E_OK` and changes nothing.
- **Free-via-shrink:** `Mshrink(0, blk, 0)` returns `E_OK` and releases the
  whole block (equivalent to `Mfree(blk)`).
- **Grow rejected:** `Mshrink(0, blk, N + 16)` returns `EGSBF` (-67); the block
  is unchanged.
- **Bad address:** `Mshrink(0, blk + 8, ...)` (mid-block) returns `EIMBA` (-40).
- **Alignment:** `Mshrink(0, blk, 1)` on ST-RAM shrinks to the alignment grain
  (2 bytes non-Falcon, 4 bytes Falcon/alt-RAM), not to 1 byte.
- **Reserved word ignored:** any `n` value yields identical behaviour.

---

## Edge cases discovered in source

1. **`n` is dead.** Never read (`bdos/umem.c:170`). Disassemblies that show
   "`Mshrink(handle, …)`" are mislabelling the reserved word.

2. **`len` rounds *up* to the alignment grain.** `Mshrink(blk, 1)` cannot
   shrink below 2 bytes (ST-RAM) or 4 bytes (alt-RAM) (`bdos/umem.c:200-203`).

3. **`len == 0` ⇒ free.** After rounding, `len` is 0 only if the caller passed
   0; EmuTOS then `freeit`s the whole block (`bdos/umem.c:214-218`). Programs
   use this as an alias for `Mfree`.

4. **Grow is impossible.** `Mshrink` is one-way; a larger `len` returns `EGSBF`
   (`bdos/umem.c:210-211`). To grow, `Mfree` + `Malloc` afresh.

5. **The `Pexec` post-load idiom.** After `Pexec(PE_LOAD, …)` a shell shrinks
   the child's over-large TPA to just what it needs, then `Pexec(PE_GO, …)`.
   This is the canonical `Mshrink` use.

6. **`shrinkit` MD-pool exhaustion leaves the block intact.** On `xmgetmd`
   failure `Mshrink` returns `ERR` (-1) *without* modifying the block, so a
   later retry is valid (`bdos/iumem.c:235-240`).

7. **Tail-MD `m_own` is not zeroed.** `shrinkit` puts the tail MD on `mp_mal`
   without setting `m_own`, and `freeit` does not clear it (`bdos/iumem.c`).
   The kept MD keeps its owner; the freed tail's `m_own` is undefined but never
   read (free ⇔ list membership, not `m_own`).

---

## Dependencies

### Requires (must be initialized / functional)

- `umem_init` — pools, `start_stram`/`end_stram`, `malloc_align_stram`.
- `find_mpb`, `shrinkit`, `freeit`, `xmgetmd`.

### Used by

- `Ptermres` (`xtermres` calls `xsetblk(0, run, blkln)` and **discards** the
  return, see D-006) — `docs/api/gemdos/process/31-ptermres.md`.
- Shells trimming a child's TPA after `Pexec(PE_LOAD, …)`.
- Programs that grab "all memory" via `Malloc(-1)` then trim.

---

## Related data structures

- `docs/api/structures/md.md` — `MD`/`MPB`, `shrinkit`/`freeit`.
- `docs/api/gemdos/_state.md` §8 — the pool model.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. Documents the unused `n`,
  the shrink-only rule, and `EGSBF` — consistent with EmuTOS.

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `malloc_align_stram` is 1 (2-byte align) on ST/STe/TT, 3 (4-byte) on Falcon;
  alt-RAM is always 4-byte aligned. The **rounded** length is what the block
  becomes, so a program recomputing its heap top must use the rounded value.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-04 (D10): alignment grain is a NewTOS choice (e.g. 16-byte for AArch64
SIMD friendliness) — the *rounded* `len` is the observable result, so programs
that recompute their heap top must read it back rather than assume the request.
On a real shrink, allocate a tail MD, mark it free, link into `mp_mfl` adjacent
to the block, coalesce if the neighbour is contiguous. No-op and same-size
shrinks take a fast `E_OK` path without touching the MPB. Grow → `EGSBF`,
`len == 0` → free, bad address → `EIMBA`, MD-arena exhaustion → `ERR` (block
untouched), all per EmuTOS.

---

## Pi / BCM2711 binding

> Re-points the pool backing to **uniform LPDDR4 split into ST-RAM + TT-RAM
> logical pools** — pinned in `44-mxalloc.md`'s binding and
> `docs/HARDWARE-PROFILE.md` §3.6. The `Mshrink` semantics are unchanged.

`mshrink` (`tos-api/src/bdos/mem/traps.rs:471-530`) routes via `find_mpb`,
rounds `len` up to the **16-byte** `PoolConfig::AARCH64.align_mask`
(`traps.rs:497-501`) — NewTOS's grain for both pools vs. EmuTOS's 2/4 bytes —
then the three-way compare (`EGSBF` grow / `E_OK` same / `freeit` on `len==0` /
`shrinkit` on shrink). The rounded `len` is the observable result on both pools.

### Gaps vs. the contract (Step-2 candidates)

- **16-byte alignment grain** (not 2/4) — the only Pi-relevant divergence;
  observable as a slightly larger rounded `len`. Callers must read back the
  effective size, as the notes say.
