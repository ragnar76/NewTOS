# `Maddalt` (GEMDOS 0x14)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/umem.c:455` (`xmaddalt`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the `size &= -4` round-down, the `-1L`/`ENSMEM`
> failure returns, the one-way "register alt-RAM, never remove" semantics.
> Reference: the exact overlap predicate, `xmgetmd`, the `pmdalt` list layout,
> the static-alt-RAM carve-out.

## EmuTOS source

**Primary location:** `bdos/umem.c:455` — `long xmaddalt(UBYTE *start, LONG size)`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x14` =
`xmaddalt`, `wparms = 4`, **only when `CONF_WITH_ALT_RAM`** (else `NI`,
"not implemented") (`bdos/bdosmain.c:194-198`).

**Helpers called:**
- `bdos/osmem.c:169` — `xmgetmd()` — obtain a fresh MD for the new region
  (only on the non-merge path).

**Headers / type definitions used:**
- `include/bdosdefs.h:108-115` — `MD`.
- `include/gemerror.h:47` — `ENSMEM = -39`.
- (`bdos/mem.h` — `extern MPB pmdalt`.)

**System variables / globals read:**
- `start_stram`, `end_stram` (`bdos/umem.c:44-45`) — ST-RAM bounds for the
  overlap check.
- `has_alt_ram` (`bdos/umem.c:38`) — whether alt-RAM already registered.
- `pmdalt.mp_mfl` — walked for the adjacent-block merge.
- `_static_altram_start`/`_static_altram_end` (`CONF_WITH_STATIC_ALT_RAM`).

**System variables / globals written:**
- `pmdalt.mp_mfl` (new/extended free MD), and on the **first** successful add
  `pmdalt.mp_mal = NULL` and `has_alt_ram = 1` (`bdos/umem.c:502-514`).

**Build configuration affecting this function:**
- `CONF_WITH_ALT_RAM` — **required**; without it the whole function and its
  dispatch entry are absent (trap 0x14 falls through to `EINVFN`).
- `CONF_WITH_STATIC_ALT_RAM` — adds the carve-out that skips EmuTOS's own
  static-data region when the exact static base address is passed.

---

## Source extraction summary

`Maddalt` tells GEMDOS about a block of "alternate" RAM that the BIOS did not
discover at boot (historically added via a VME card or hardware mod). The
region is added to the alt-RAM pool `pmdalt` as **free** memory; from then on it
is allocatable via `Mxalloc` (modes 1/2/3) or by `PF_TTRAMMEM` programs' plain
`Malloc`.

The function first rounds `size` **down** to a multiple of 4 (`size &= -4L`);
if the result is `<= 0` it returns `-1`. It then rejects any region that
overlaps ST-RAM: the predicate is `(start < start_stram && start + size >
start_stram) || (start < end_stram)` — i.e. anything starting below the *end*
of ST-RAM is refused, which also refuses ranges entirely below ST-RAM start
(see D-010 for NewTOS's tighter predicate).

Next it tries to **merge** with an existing free MD on `pmdalt.mp_mfl`: if the
new region butts up against the end of a free block it extends that block's
length; if it butts up against the start it moves that block's `m_start` down
and extends its length. Either merge returns 0 and **consumes no new MD** —
important, because `xmgetmd` can run out.

If no merge applies, it (optionally, under `CONF_WITH_STATIC_ALT_RAM`) skips the
static-data sub-region, then allocates a fresh MD via `xmgetmd`
(`ENSMEM` if none), fills it (`m_start`, `m_length`, `m_own = NULL`), and links
it onto `pmdalt.mp_mfl`. On the **very first** alt-RAM registration it also
initialises the pool by setting `pmdalt.mp_mal = NULL` and `has_alt_ram = 1`;
`mp_rover` is **not** touched. Registration is one-way — there is no
"remove alt-RAM" call.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x14
- **Stack frame on entry (m68k):**
  ```
  +6: size  (LONG)
  +2: start (LONG, UBYTE *)
   0: function_id (word) = 0x0014
  ```
- **Total parameter bytes:** 8 (LONG + LONG) → `wparms = 4`.
- **Native (AArch64) ABI:** ID `0x14` in `x8`; `start` (pointer, full 64-bit)
  in `x0`, `size` in `x1`. LONG result (0 / `-1` / `ENSMEM`) sign-extended into
  `x0`.

  | m68k stack | AArch64 reg | Name    | Type / size |
  |------------|-------------|---------|-------------|
  | +2         | `x0`        | `start` | `UBYTE *` (ptr) |
  | +6         | `x1`        | `size`  | `LONG` (32/`long`) — rounded down to ×4 |

---

## Inputs

| Position | Name    | Type     | Size | Source location   | Notes |
|----------|---------|----------|------|-------------------|-------|
| arg0     | `start` | `UBYTE *`| 4    | `bdos/umem.c:455` | base of the region to register |
| arg1     | `size`  | `LONG`   | 4    | `bdos/umem.c:460` | length in bytes; `size &= -4` (round **down** to ×4) |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG (GEMDOS error code / status)
- **Encoding:**

  | Case | Return |
  |------|--------|
  | Region added, or merged with an adjacent free block | `E_OK` (0) |
  | `size` after round-down `<= 0` | `-1` |
  | Region overlaps ST-RAM (per the predicate) | `-1` |
  | No MD available from the MD pool | `ENSMEM` (-39) |

- **Source location:** `bdos/umem.c:464,469,476,481,497,516`.

### Side effects on global state

- Merge path: an existing `pmdalt.mp_mfl` MD's `m_length` (and possibly
  `m_start`) grows; no MD allocated.
- New-MD path: a fresh MD (`m_own = NULL`) is prepended to `pmdalt.mp_mfl`; the
  first-ever add also sets `pmdalt.mp_mal = NULL`, `has_alt_ram = 1`
  (`mp_rover` untouched).
- Failure: no state change.

### Side effects on hardware

None. (`Maddalt` records addresses; it does not probe or map the RAM.)

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `size` non-positive after `& -4` | `bdos/umem.c:463-464` | `-1` |
| Region overlaps ST-RAM | `bdos/umem.c:467-469` | `-1` |
| MD pool exhausted (`xmgetmd` NULL), non-merge path | `bdos/umem.c:495-497` | `ENSMEM` (-39) |

**Error paths not in Compendium:** the geometry rejection is a bare `-1L`, not a
named GEMDOS error (`EINVFN`/`ERR`/`EIMBA` are all different values). Compendium/
Profibuch document it simply as "`-1L` = error", which matches.

---

## Verification criteria

- **Nominal add (first):** with no alt-RAM yet, `Maddalt(A, S)` for a
  non-ST-RAM `[A, A+S)` returns 0; afterwards `Mxalloc(-1, MX_TTRAM)` reports a
  block of (rounded) size `S & -4`, and `has_alt_ram` is set.
- **Merge:** a second `Maddalt(A + (S & -4), S2)` that butts against the first
  free block returns 0 and extends the existing MD (no new MD consumed).
- **Round-down:** `Maddalt(A, 6)` registers 4 bytes; `Maddalt(A, 3)` returns
  `-1` (rounds to 0).
- **ST-RAM overlap:** `Maddalt(start_stram, S)` (or any `start < end_stram`)
  returns `-1`.
- **MD exhaustion:** with the MD pool full and no adjacent block to merge,
  `Maddalt(A, S)` returns `ENSMEM` (-39).

---

## Edge cases discovered in source

1. **`size &= -4` rounds *down*.** 6 → 4; 3 → 0 → `-1` (`bdos/umem.c:460`).

2. **Overlap predicate refuses everything below `end_stram`.** `start <
   end_stram` alone rejects the region, so ranges entirely *below* ST-RAM start
   are also refused (`bdos/umem.c:467-468`). NewTOS uses a tighter half-open
   intersection predicate — see **D-010**.

3. **Adjacent-block merge consumes no MD.** The two merge arms extend an
   existing free MD in place (`bdos/umem.c:472-483`), avoiding the `xmgetmd`
   risk. The free list may end up unsorted across successive prepends, though
   (see next).

4. **First add sets `mp_mal = NULL`, not `mp_rover`.** The one-time init path
   writes `pmdalt.mp_mfl = md; pmdalt.mp_mal = NULL; has_alt_ram = 1`
   (`bdos/umem.c:509-514`); `mp_rover` is never touched. (Corrects the earlier
   draft's fabricated `pmdalt.mp_rover = md`.)

5. **Prepend without sort/coalesce (beyond the immediate adjacency check).**
   Subsequent adds prepend to `mp_mfl` (`bdos/umem.c:506-508`); the list can
   become unsorted. `ffit` does not require order, but `freeit` inserts
   address-sorted and coalesces, so the lists self-heal on the first
   alloc/free cycle. Flagged for the PHASE-04 alt-RAM test list.

6. **Static-alt-RAM carve-out.** With `CONF_WITH_STATIC_ALT_RAM`, when `start`
   exactly equals EmuTOS's own static-data base, the function advances past the
   occupied `[_static_altram_start, _static_altram_end)` before adding the rest
   (`bdos/umem.c:485-493`).

7. **New MD is free (`m_own = NULL`), one-way registration.** The region joins
   the free list unowned; there is no de-registration call
   (`bdos/umem.c:499-501`).

---

## Dependencies

### Requires (must be initialized / functional)

- `umem_init` — `start_stram`/`end_stram` and `has_alt_ram = 0` initial state.
- `xmgetmd` — MD meta-allocator (for the non-merge path).

### Used by

- BIOS/board startup registering physical alt-RAM detected at boot
  (`altram_init` under `CONF_WITH_ALT_RAM`); VME-card / hardware-mod ROMs.

---

## Related data structures

- `docs/api/structures/md.md` — `MD`/`MPB`, `pmdalt`.
- `docs/api/gemdos/_state.md` §8 — the pool model.
- `docs/api/gemdos/memory/44-mxalloc.md` — the consumer of `pmdalt`.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. Documents the signature
  and the `-1L` error — consistent with EmuTOS.

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

### Discrepancy log

- **D-010** — NewTOS uses a half-open intersection overlap predicate instead of
  EmuTOS's `start < end_stram`, accepting regions entirely below ST-RAM. No
  impact on supported boards.

---

## Configuration / version notes

- `CONF_WITH_ALT_RAM` **required** (default 1 on the full build, 0 on cut-down
  machines). Historically "available as of GEMDOS 0.19"; EmuTOS reports modern
  GEMDOS so the call always exists when alt-RAM support is compiled in.
- `CONF_WITH_STATIC_ALT_RAM` adds the static-data carve-out.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-04 (D7): all DRAM is discovered at boot from the device tree, so the
ST-RAM and TT-RAM MPBs already cover known memory — `Maddalt` is a no-op for
in-range addresses and an additive insertion only for a region outside the
existing extents. Reject overlaps with any existing MPB MD (`-1`, per D-010's
tighter predicate). Accepted ranges go to `pmdalt` regardless of physical
location (EmuTOS treats anything passed here as alt-RAM by definition).
Coalesce with a contiguous neighbour to avoid consuming a fresh MD. **Out of
scope:** DMA-safety reclassification — AArch64 has no ST/alt-RAM hardware
distinction; the split is purely a TOS-API contract.

---

## Pi / BCM2711 binding

> Re-points alt-RAM from a **physical TT-RAM extension board** to **a logical
> partition of uniform LPDDR4** — pinned in `44-mxalloc.md`'s binding and
> `docs/HARDWARE-PROFILE.md` §3.6. The `Maddalt` semantics are unchanged.

The TT-RAM (`pmdalt`) pool is already seeded at boot from the AArch64 memory map
(`kernel/src/lib.rs:452-481` → `bdos/mem/init.rs`), so on the Pi `Maddalt` is
mostly redundant — all DRAM is discovered up front. `maddalt`
(`tos-api/src/bdos/mem/traps.rs:153-226`) still honours the full contract: round
`size` down to ×4, reject `size <= 0` (`-1`), reject overlap with the cached
ST-RAM range (`traps.rs:170-174`), merge with an adjacent free MD or prepend a
fresh one, `ENSMEM` if the MD arena is exhausted. Accepted ranges go to
`pmdalt` by definition (no physical zone check — there is no TT board).

### Gaps vs. the contract (Step-2 candidates)

- **`i32` address ABI** — a region above the `Long` range cannot be added until
  the Phase-05 window (shared with `44-mxalloc.md`). Otherwise fully
  implemented; on the Pi the call is chiefly a no-op because boot already
  registers all DRAM.
