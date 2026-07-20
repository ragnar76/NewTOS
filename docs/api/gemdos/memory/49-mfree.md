# `Mfree` (GEMDOS 0x49)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/umem.c:135` (`xmfree`) and `bdos/iumem.c:144`
> (`freeit`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the "address must be a block start" rule, the
> `E_OK`/`EIMBA` result codes, the *absence* of an ownership check (a program
> can free any block). Reference: `find_mpb`, `freeit`, the MD/MPB lists,
> coalescing.

## EmuTOS source

**Primary location:** `bdos/umem.c:135` — `long xmfree(void *addr)`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x49` =
`xmfree`, `wparms = 2` (`bdos/bdosmain.c:269`).

**Helpers called:**
- `bdos/umem.c:99` — `find_mpb()` — decide which pool (`pmd`/`pmdalt`) owns the
  address, by ST-RAM range test then alt-RAM fallback.
- `bdos/iumem.c:144` — `freeit()` — unlink the MD from `mp_mal`, insert it into
  the address-sorted `mp_mfl`, coalesce with adjacent free MDs.

**Headers / type definitions used:**
- `include/bdosdefs.h:108-115` — `MD`.
- `include/gemerror.h:19,48` — `E_OK = 0`, `EIMBA = -40`.

**System variables / globals read:**
- `start_stram`, `end_stram` (`bdos/umem.c:44-45`) — ST-RAM bounds used by
  `find_mpb`.
- `pmd`, `pmdalt` (`has_alt_ram`) — the two pools.

**System variables / globals written:**
- The chosen pool's `mp_mal` (MD unlinked) and `mp_mfl` (MD inserted,
  neighbours coalesced) (`bdos/iumem.c:164-222`). An absorbed neighbour MD is
  returned to the OS-memory MD pool via `xmfremd`.

**Build configuration affecting this function:**
- `CONF_WITH_ALT_RAM` — only then does `find_mpb` consult `pmdalt`; without it,
  a non-ST-RAM address yields `NULL` → `EIMBA`.

---

## Source extraction summary

`Mfree` releases a block previously returned by `Malloc`/`Mxalloc`. It first
asks `find_mpb` which pool the address belongs to: if the address is within
`[start_stram, end_stram)` it is `pmd`; otherwise, if any alt-RAM has been
registered, it is *assumed* to be `pmdalt` (a trust-the-caller shortcut — the
real membership check is the following scan); otherwise `find_mpb` returns NULL
and `Mfree` returns `EIMBA`.

It then scans that pool's allocated list (`mp_mal`) for an MD whose `m_start`
**exactly equals** `addr`. There is no "block containing this address"
semantics — a pointer into the middle of an allocation is not found. If no MD
matches, it returns `EIMBA`; this is also what a double-free hits (the first
free already moved the MD off `mp_mal`).

On a match it calls `freeit`, which unlinks the MD from `mp_mal`, walks
`mp_mfl` to the correct ascending-`m_start` position, links the MD in there,
and coalesces: if the newly-inserted MD is contiguous with its higher neighbour
it absorbs it (and frees that neighbour's MD via `xmfremd`), and likewise for
the lower neighbour. `Mfree` returns `E_OK`.

**Note — no ownership check, and `m_own` is left stale.** `xmfree` never
compares `m_own` with `run`, so any process can free any process's block by
passing its address (a preserved Atari quirk). And `freeit` does **not** set
`m_own = NULL` on the freed MD — it only relinks it. The freed MD keeps
whatever `m_own` it had; nothing in the free path reads `m_own` (membership is
by list position + `m_start`), so this is harmless internally, but it means
"`m_own == NULL` ⇔ free" is **not** an invariant of this build.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x49
- **Stack frame on entry (m68k):**
  ```
  +2: addr (LONG, void *)
   0: function_id (word) = 0x0049
  ```
- **Total parameter bytes:** 4 (LONG) → `wparms = 2`.
- **Native (AArch64) ABI:** ID `0x49` in `x8`; `addr` (pointer, full 64-bit) in
  `x0`. LONG result (error code) sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name   | Type / size |
  |------------|-------------|--------|-------------|
  | +2         | `x0`        | `addr` | `void *` (ptr) |

---

## Inputs

| Position | Name   | Type     | Size | Source location   | Notes |
|----------|--------|----------|------|-------------------|-------|
| arg0     | `addr` | `void *` | 4    | `bdos/umem.c:135` | must equal the `m_start` of an allocated MD |

`addr` is not otherwise validated; a wild pointer simply fails the scan and
returns `EIMBA` (no fault, no crash from `Mfree` itself).

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG (GEMDOS error code)
- **Encoding:**

  | Case | Return |
  |------|--------|
  | Block found on `mp_mal` and freed | `E_OK` (0) |
  | Address in no pool (`find_mpb` NULL) | `EIMBA` (-40) |
  | Address in a pool but not a block start | `EIMBA` (-40) |

- **Source location:** `bdos/umem.c:144,153,158`.

### Side effects on global state

- On success: MD moved from `mp_mal` to `mp_mfl` (address-sorted), coalesced
  with contiguous free neighbours; absorbed neighbour MDs returned to the MD
  pool (`xmfremd`). No change to `m_own`.
- On error: no state change.

### Side effects on hardware

None.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `find_mpb(addr)` returns NULL (address below ST-RAM and no alt-RAM, etc.) | `bdos/umem.c:142-144` | `EIMBA` (-40) |
| No MD on the pool's `mp_mal` has `m_start == addr` (wrong pointer, mid-block, or double-free) | `bdos/umem.c:148-153` | `EIMBA` (-40) |

**Error paths not in Compendium:** Compendium documents `EIMBA` for a bad
address; it does not spell out that *double-free* and *mid-block* pointers both
return `EIMBA` rather than faulting — that is EmuTOS behaviour.

---

## Verification criteria

- **Nominal free:** after `p = Malloc(N)`, `Mfree(p)` returns `E_OK` and the
  block's bytes return to `mp_mfl` (a subsequent `Malloc(N)` can reuse them).
- **Coalescing:** freeing two blocks that are address-adjacent leaves a single
  merged free MD (the higher/lower neighbour is absorbed).
- **Mid-block pointer:** `Mfree(p + 4)` for a block `p` returns `EIMBA` (-40)
  and changes nothing.
- **Double free:** a second `Mfree(p)` on the same pointer returns `EIMBA`.
- **Out-of-pool pointer:** `Mfree(addr)` for an address in no pool returns
  `EIMBA`.

---

## Edge cases discovered in source

1. **Must be the block *start*.** Matching is `addr == p->m_start`
   (`bdos/umem.c:149`); interior pointers → `EIMBA`. No "containing block"
   lookup.

2. **No ownership check.** `xmfree` never tests `m_own == run`; any process can
   free any block. `Pterm` cleanup *does* filter by owner
   (`free_all_owned`/`reserve_blocks`, `bdos/proc.c:77-101`), but interactive
   `Mfree` does not.

3. **`freeit` does not clear `m_own`.** The freed MD retains its old owner
   pointer (`bdos/iumem.c:144-222` — no `m_own` write). "Free ⇔ `m_own ==
   NULL`" is therefore **not** an invariant here; the free flag is list
   membership, not `m_own`. (Corrects the earlier docs.)

4. **`find_mpb` trusts non-ST-RAM addresses.** Any address outside ST-RAM with
   alt-RAM registered is routed to `pmdalt` *without* a range check
   (`bdos/umem.c:104-107`); the actual validation is the `mp_mal` scan, so a
   wild non-ST-RAM pointer still fails cleanly with `EIMBA`.

5. **Double-free is not a crash in EmuTOS.** The MD is already off `mp_mal`, so
   the second free returns `EIMBA`. The caller's pointer is still dangling,
   though — later access corrupts whoever now owns the region.

6. **Freeing basepage / environment is legal.** These are ordinary owned MDs; a
   process freeing its own basepage is self-destructive but not prevented.

---

## Dependencies

### Requires (must be initialized / functional)

- `umem_init` — `start_stram`/`end_stram` and `pmd`.
- `freeit`, `xmfremd` — the coalescing free path and MD meta-free.

### Used by

- User programs releasing `Malloc`/`Mxalloc` blocks.
- `Pterm`/`Pexec` cleanup via `free_all_owned` (which calls `freeit` directly,
  not `xmfree`, `bdos/proc.c:99`) and `xmfree` for env/PD blocks
  (`bdos/proc.c:223,297,325-326,340-341`).

---

## Related data structures

- `docs/api/structures/md.md` — `MD`/`MPB`, `freeit` coalescing.
- `docs/api/gemdos/_state.md` §8 — the pool model.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. Documents `EIMBA` for a
  bad address — consistent with EmuTOS.

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `CONF_WITH_ALT_RAM` gates the `pmdalt` branch of `find_mpb`; otherwise
  identical across TOS versions.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-04 (D9): route via `find_mpb` (ST-RAM range test, then alt-RAM); on a
match unlink from `mp_mal`, insert address-ordered into `mp_mfl`, coalesce both
neighbours, return the redundant MD to the MD arena. **Default: no ownership
check** (Atari-compatible; parents legitimately free child memory after
`Pwait`). An **optional** `strict-ownership` build feature may return `EACCDN`
when `m_own != current_pd`, off by default, for catching bugs in ported
sources. NewTOS should also clear `m_own` on free (cleaner than EmuTOS) since it
does **not** rely on the stale-`m_own` quirk.

### NewTOS EL0 realization (ADR-021)

An **EL0** caller frees by the **user VA** it received from `Malloc`/`Mxalloc`,
not a pool physical address. The trap entry translates that user VA back to the
pool key (`m_start`) through the per-process block table
(`bdos::process::usermem`) and frees it via the **unchanged**
`bdos::mem::traps::mfree`; a user VA that is not a tracked block start returns
`EIMBA`, matching EmuTOS's "address not on `mp_mal`". The window page mapping is
left in place — the freed VA is never re-handed by the bump cursor, so no TLBI is
required (QUIRK-001). A loaded AES resource is reclaimed via `rsrc_free` (which
tracks it by pool key), **not** `Mfree`, so a stray `Mfree(rsc_va)` returns
`EIMBA` rather than racing `rsrc_free` into a double free. **EL1 / kernel
callers** free the raw address directly. Discrimination: non-null
`bdos::process::current()`, mirroring `aes::resource`.

---

## Pi / BCM2711 binding

> Re-points the pool backing to **uniform LPDDR4 split into ST-RAM + TT-RAM
> logical pools** — pinned in `44-mxalloc.md`'s binding and
> `docs/HARDWARE-PROFILE.md` §3.6. The `Mfree` semantics are unchanged.

`mfree` (`tos-api/src/bdos/mem/traps.rs:413-440`) locates the owning pool with
`find_mpb` (the cached ST-RAM range test → `pmd`, else `pmdalt`;
`traps.rs:386-393`), walks `mp_mal` for `m_start == addr`, returns `EIMBA` if
not found, else `freeit` (address-ordered `mp_mfl` insert + coalesce). Which
physical pool an address belongs to is decided purely by the logical ST-RAM
range on the Pi — there is no hardware zone.

### Gaps vs. the contract (Step-2 candidates)

- **None pool-specific.** The `find_mpb` routing is correct for the logical
  split; the ownership/`m_own`-clearing choices above are generic (not Pi-bound).
