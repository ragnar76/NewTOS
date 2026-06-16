# `Mfree` — GEMDOS 0x49

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/umem.c:135` (`xmfree`).

## EmuTOS source

- **Primary:** `bdos/umem.c:135` — `long xmfree(void *addr)`
- **Helpers:** `find_mpb()` — locate which pool (`pmd` / `pmdalt`)
  contains `addr`; `freeit()` (`bdos/mem.c`) — unlink from
  allocated list, insert into free list, coalesce.
- **Globals:** `pmd`, `pmdalt`, `start_stram`, `end_stram`.

## Source extraction

```c
long xmfree(void *addr)
{
    MD *p;
    MPB *mpb = find_mpb(addr);
    if (!mpb) return EIMBA;            /* address not in any pool */

    for (p = mpb->mp_mal; p; p = p->m_link)
        if (addr == p->m_start) break;

    if (!p) return EIMBA;              /* address not the start of any MD */

    freeit(p, mpb);
    return E_OK;
}
```

```c
static MPB *find_mpb(void *addr)
{
    if (((UBYTE *)addr >= start_stram) && ((UBYTE *)addr < end_stram))
        return &pmd;
#if CONF_WITH_ALT_RAM
    if (has_alt_ram) return &pmdalt;
#endif
    return NULL;
}
```

`find_mpb` is **address-range-only** for ST-RAM. For alt-RAM,
EmuTOS returns `&pmdalt` for any address that isn't ST-RAM —
trusting the caller. The actual MD-membership check happens in the
linear scan. This is why passing a wild pointer that *happens* to
fall outside ST-RAM still gets `EIMBA` (not a crash) — the loop
simply finds no matching MD.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0049` |
| WORD args | 2 (LONG) |
| Stack frame | `+2: addr (LONG)`, `+0: 0x0049 (fn)` |

## Inputs

| Pos | Name   | Type   | Range / meaning                                  |
|-----|--------|--------|--------------------------------------------------|
| 0   | `addr` | void * | start address of a previously-allocated block    |

## Outputs

| Reg | D0  |
| --- | --- |
| Type | LONG (GEMDOS error code) |

| Case                              | Return                  |
|-----------------------------------|-------------------------|
| Block found and freed             | `E_OK` (`0L`)           |
| Address not in any pool           | `EIMBA` (`-40`)         |
| Address in pool but not on `mp_mal` | `EIMBA` (`-40`)       |

## Errors

| Code | Constant | Cause                                                   |
|-----:|----------|---------------------------------------------------------|
| -40  | `EIMBA`  | "Improper Memory Block Address" — address not the start of an allocated block |

## Edge cases

- **Must point at the **start** of a block.** Pointers into the
  middle of an allocation return `EIMBA`. There is no
  "free containing block" semantics — the address must equal an MD's
  `m_start` exactly.
- **No ownership check.** `Mfree` does **not** verify that
  `m_own == run`. A process can free another process's block by
  passing its address. EmuTOS preserves this Atari quirk; it has
  bitten programs in practice. `Pterm` cleanup *does* check
  ownership, but interactive `Mfree` does not.
- **Coalescing**: `freeit` walks the sorted free list, inserts the
  newly-freed MD at the right position, and merges with adjacent
  free MDs (front and back). The MD struct of an absorbed neighbour
  is returned to the MD pool via `xmfremd`.
- **Double free**: if a block is freed twice, the second `Mfree`
  finds nothing on `mp_mal` (the first free moved it) and returns
  `EIMBA`. There is no use-after-free crash from EmuTOS itself, but
  the caller's pointer is dangling and any data access scribbles
  the new owner's data.
- **Free of a `Ptermres`-reserved block**: returns `EIMBA` because
  `reserve_blocks` already removed the MD from `mp_mal`.
- **Free of basepage / env**: legal — these are ordinary MDs. A
  process freeing its own basepage is suicidal but not prevented;
  the next instruction fetch is undefined.

## Dependencies

- `find_mpb` (file-local).
- `freeit` (`bdos/mem.c`) — coalescing free.

Used by: `Pterm` (per-MD cleanup via `free_all_owned`); user
programs releasing `Malloc`/`Mxalloc` blocks; `Pexec` cleanup paths.

## Cross-references

- **Compendium:** "Mfree" — agrees on `EIMBA` semantics.
- **Profibuch:** "Mfree (0x49)" — agrees.
- **`docs/api/gemdos/memory/48-malloc.md`**, **44-mxalloc.md**.
- **`docs/api/structures/md.md`** — list manipulation.

## Configuration

- `CONF_WITH_ALT_RAM` — only then does `find_mpb` consult `pmdalt`.

## NewTOS implementation notes

Phase 04 (D9) plan:

- **Pool routing via `find_mpb`.** Walk `pmd.mp_mal` first; on miss
  walk `pmdalt.mp_mal`. No address-range short-circuit — the MPB
  ranges may interleave once `Maddalt` enters the picture.
- **Coalescing.** After detaching the MD from the allocated list,
  insert into `mp_mfl` keeping it address-ordered, then merge with
  the neighbouring free MD on either side if their `m_start +
  m_length` matches up. Free the now-redundant MD via `xmfremd`.
- **Default behaviour: no ownership check.** Even when `m_own`
  belongs to a different process, `Mfree` succeeds — this is
  Atari-compatible and a regular pattern in real software (parent
  freeing child memory after `Pwait`).
- **Strict mode (build-time).** Optional `feature = "strict-ownership"`
  flag in `tos-api`: when set, `Mfree` returns `EACCDN` if
  `m_own != current_pd`. Off by default; useful for catching real
  bugs in ported sources during Phase 09 work.
