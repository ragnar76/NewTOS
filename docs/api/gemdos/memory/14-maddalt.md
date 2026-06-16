# `Maddalt` — GEMDOS 0x14

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/umem.c:455` (`xmaddalt`).

## EmuTOS source

- **Primary:** `bdos/umem.c:455` — `long xmaddalt(UBYTE *start, LONG size)`
- **Helpers:** `xmgetmd()` — pull a fresh MD from the MD pool;
  list manipulation on `pmdalt.mp_mfl`.
- **Globals:** `pmdalt`, `has_alt_ram`, `start_stram`, `end_stram`,
  `_static_altram_start`, `_static_altram_end` (CONF_WITH_STATIC_ALT_RAM).
- **Build config:** `CONF_WITH_ALT_RAM`. Without it, `Maddalt` is
  not present in the dispatcher table.

## Source extraction

```c
long xmaddalt(UBYTE *start, LONG size)
{
    /* round size down to multiple of 4 */
    size &= -4L;
    if (size <= 0) return -1;

    /* must not overlap ST-RAM */
    if (((start < start_stram) && (start + size > start_stram))
     || (start < end_stram))
        return -1;

    /* try to merge with an adjacent free block */
    for (p = pmdalt.mp_mfl; p; p = p->m_link) {
        if (p->m_start + p->m_length == start) {
            p->m_length += size; return 0;
        } else if (start + size == p->m_start) {
            p->m_start  -= size;
            p->m_length += size; return 0;
        }
    }

#if CONF_WITH_STATIC_ALT_RAM
    /* skip over EmuTOS's own alt-RAM static-data region */
    if (start == _static_altram_start && _static_altram_end > _static_altram_start) {
        start = _static_altram_end;
        size -= _static_altram_end - _static_altram_start;
    }
#endif

    md = xmgetmd();
    if (!md) return ENSMEM;
    md->m_start  = start;
    md->m_length = size;
    md->m_own    = NULL;

    if (has_alt_ram) {
        md->m_link = pmdalt.mp_mfl;
        pmdalt.mp_mfl = md;
    } else {
        md->m_link = NULL;
        pmdalt.mp_mfl = md;
        pmdalt.mp_rover = md;
        has_alt_ram = TRUE;
    }
    return E_OK;
}
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0014` |
| WORD args | 4 (LONG `start` + LONG `size`) |
| Stack frame | `+6: size (LONG)`, `+2: start (LONG)`, `+0: 0x0014` |

## Inputs

| Pos | Name    | Type    | Range / meaning                              |
|-----|---------|---------|----------------------------------------------|
| 0   | `start` | UBYTE * | base address of new alt-RAM region            |
| 1   | `size`  | LONG    | length in bytes (rounded down to multiple of 4) |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (GEMDOS error code) |

| Case                                          | Return       |
|-----------------------------------------------|--------------|
| Region added (or merged with neighbour)        | `E_OK` (0)  |
| `size` after rounding ≤ 0                      | `-1`        |
| Region overlaps ST-RAM                         | `-1`        |
| MD pool exhausted                              | `ENSMEM` (-39) |

## Errors

`Maddalt` returns a bare `-1` (not a named GEMDOS error) for
geometry rejection. This is **not** `EINVFN`/`ERR`/`EIMBA` — it is
literal `-1L`. Compendium and Profibuch both document it as
`-1L = error`.

`ENSMEM` (-39) only when no MD struct is available.

## Edge cases

### 1. Available "as of GEMDOS 0.19 only"

Programs that detect TOS ≥ 1.04 may call `Maddalt`. On older TOS,
the trap returns whatever the dispatcher's unknown-function path
yields (`EINVFN`). EmuTOS reports modern GEMDOS so applications
calling `Maddalt` succeed unconditionally.

### 2. Size rounding

`size &= -4L` rounds **down** to a multiple of 4. A `size` of 6
becomes 4; a `size` of 3 becomes 0 → returns `-1`.

### 3. ST-RAM overlap rejection

Two checks:
- Region starts below `start_stram` but extends into it.
- Region starts inside ST-RAM at all (`start < end_stram`).

The check missed a case where the region starts above ST-RAM but
overlaps it from below — but that's geometrically impossible if
`start_stram < end_stram` and the first check already covers it.

### 4. Adjacent-block merge

If the new region butts up against an existing free MD on `pmdalt`:
- New region after free block: extend the free block's length.
- New region before free block: extend the free block backwards.

**No new MD is allocated** in the merge path. This is important
because `xmgetmd` can run out — the merge path avoids that risk.

### 5. Static alt-RAM carve-out

When EmuTOS itself uses alt-RAM for its BSS (`CONF_WITH_STATIC_ALT_RAM`),
the boot code typically calls `Maddalt(start_of_altram, full_size)`.
The function detects this exact base address and skips over the
self-occupied portion before adding the rest.

### 6. First call vs. subsequent calls

The first successful `Maddalt` sets `has_alt_ram = TRUE` and
initialises `pmdalt`. Subsequent calls just prepend to the free
list. There is no "remove alt-RAM" function — registration is
permanent.

### 7. No coalescing on first add

Note: when adding to an existing alt-RAM pool, EmuTOS prepends to
`mp_mfl` *without* sorting or coalescing beyond the immediate
adjacency check. The free list may end up unsorted. `xmxalloc`'s
`ffit` doesn't depend on order, but `freeit` later expects sorted
order — practical impact: the first allocation/free cycle on
alt-RAM may produce non-coalesced free MDs that should have merged.
Manually verifying this is on the PHASE-03 testing list.

### 8. Owner of new MD

`m_own = NULL` — the region is added to the free list directly,
not pre-allocated to anyone.

## Dependencies

- `xmgetmd()` — fresh MD from the MD pool.
- `pmdalt` MPB.
- `has_alt_ram` flag.

Used by: BIOS startup (registers physical alt-RAM detected at
boot), VME-card drivers, hardware mod ROMs.

## Cross-references

- **Compendium:** "Maddalt" — agrees on signature and `-1L` error.
- **Profibuch:** "Maddalt (0x14)" — agrees.
- **`docs/api/gemdos/memory/44-mxalloc.md`** — consumer of `pmdalt`.
- **`docs/api/structures/md.md`** — MD/MPB.

## Configuration

- `CONF_WITH_ALT_RAM` — required.
- `CONF_WITH_STATIC_ALT_RAM` — adds the static-data carve-out.

## NewTOS implementation notes

Phase 04 (D7) plan:

- **All DRAM is discovered at boot** via the device tree
  (PHASE-03 D8). The ST-RAM and TT-RAM MPBs already cover everything
  we know about \u2014 `Maddalt` is therefore a *no-op for in-range
  addresses* and an *additive insertion* only when the caller
  describes a region outside the existing MPB extents.
- **Validation.** Reject ranges that overlap any existing MPB free
  or allocated MD with return value `-1` (matches EmuTOS
  `xmaddalt` failure semantics for double-add).
- **Where it ends up.** The accepted range is added to `pmdalt`
  (TT-RAM MPB), regardless of its physical location \u2014 EmuTOS
  treats anything passed to `Maddalt` as alt-RAM by definition.
- **Adjacency.** Coalesce with the neighbouring free MD if the new
  region is contiguous; this avoids consuming a fresh MD for
  back-to-back `Maddalt` calls.
- **Out of scope.** No DMA-safety reclassification: AArch64 has no
  ST-RAM/alt-RAM distinction at the hardware level, the split is
  purely a TOS-API contract.
