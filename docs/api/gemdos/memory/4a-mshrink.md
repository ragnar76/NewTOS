# `Mshrink` — GEMDOS 0x4A

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/umem.c:170` (`xsetblk`).

## EmuTOS source

- **Primary:** `bdos/umem.c:170` — `long xsetblk(int n, void *blk, long len)`
- **Helpers:** `find_mpb`, `freeit`, `shrinkit` (`bdos/mem.c`).

## Source extraction

```c
long xsetblk(int n, void *blk, long len)
{
    MD *p;
    MPB *mpb = find_mpb(blk);
    if (!mpb) return EIMBA;

    for (p = mpb->mp_mal; p; p = p->m_link)
        if ((UBYTE *)blk == p->m_start) break;
    if (!p) return EIMBA;

    /* alignment: ST-RAM rounds to malloc_align_stram, alt-RAM to LONG */
    if (mpb == &pmd)
        len = (len + malloc_align_stram) & ~malloc_align_stram;
    else
        len = (len + MALLOC_ALIGN_ALTRAM) & ~MALLOC_ALIGN_ALTRAM;

    if (p->m_length <  len) return EGSBF;     /* requested grow */
    if (p->m_length == len) return E_OK;      /* no change */
    if (len == 0) {                           /* equivalent to Mfree */
        freeit(p, mpb);
        return E_OK;
    }

    if (shrinkit(p, mpb, len) < 0) return ERR;
    return E_OK;
}
```

The `n` argument is **unused** — it is the historic "block number"
parameter from CP/M roots. Atari TOS preserves it for ABI
compatibility.

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x004A` |
| WORD args | 5 (WORD `n` + LONG `blk` + LONG `len`) |
| Stack frame | `+8: len (LONG)`, `+4: blk (LONG)`, `+2: n (WORD)`, `+0: 0x004A` |

## Inputs

| Pos | Name | Type   | Range / meaning                                |
|-----|------|--------|------------------------------------------------|
| 0   | `n`  | WORD   | unused; historically zero                      |
| 1   | `blk`| void * | start address of allocated block               |
| 2   | `len`| LONG   | new block length in bytes; 0 == free           |

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (GEMDOS error code) |

| Case                                                | Return       |
|-----------------------------------------------------|--------------|
| Shrink succeeded                                    | `E_OK`       |
| `len == p->m_length` after alignment                | `E_OK`       |
| `len == 0` after alignment (block freed)            | `E_OK`       |
| `blk` not the start of any allocated MD             | `EIMBA` (-40) |
| `len` (after rounding) > current block length       | `EGSBF` (-67) |
| MD-pool exhaustion during split                     | `ERR` (-1)   |

## Errors

| Code | Constant | Cause                                              |
|-----:|----------|----------------------------------------------------|
| -40  | `EIMBA`  | invalid block address                              |
| -67  | `EGSBF`  | "Growth attempt" — new size larger than current    |
| -1   | `ERR`    | could not allocate MD struct for the split half    |

## Edge cases

### 1. Alignment rounding

`len` is rounded **up** to the pool's alignment grain. So
`Mshrink(blk, 1)` rounds to 2 bytes (ST-RAM) or 4 bytes (alt-RAM).
The caller cannot shrink below the alignment unit.

### 2. `len == 0` ⇒ free

After alignment, `len == 0` only when the original `len` was 0.
EmuTOS treats this as `Mfree` — the entire block is released and
the MD goes onto `mp_mfl`. **Programs commonly use this idiom** to
"free a block they previously got but no longer need" while
continuing to address it via a different MD they kept.

### 3. Cannot grow

`Mshrink` is a one-way operation: only shrinking is supported. A
"grow" attempt returns `EGSBF`. Programs that need to grow must
`Mfree` and `Malloc` afresh.

### 4. The `Pexec` post-load idiom

Standard pattern after `Pexec(PE_LOADGO, ...)`:

```c
basepage = Pexec(PE_LOAD, "child.prg", tail, env);
needed = sizeof(BASEPAGE) + tlen + dlen + blen + stack_size;
Mshrink(0, basepage, needed);          /* release excess TPA */
Pexec(PE_GO, NULL, basepage, NULL);
```

This is how an Atari shell hands a child program only the memory it
truly needs — by default `Pexec` gives it the largest free block.

### 5. `shrinkit` MD pool exhaustion

The split creates a new MD for the freed tail. If the system's MD
pool (separate from regular memory) is exhausted, `shrinkit`
returns -1 → `Mshrink` returns `ERR` (-1). The original block is
**not** modified in this case — caller may retry later.

### 6. Ownership unchanged

The shrunk MD retains `m_own`. The new freed-tail MD has
`m_own = NULL` (free).

### 7. `n` parameter

Always ignored. Programs that pass garbage there work fine.
Disassemblers occasionally show "Mshrink(handle, ...)" — the
"handle" is `n` and has no meaning.

## Dependencies

- `find_mpb`, `freeit`, `shrinkit`.

Used by: `Ptermres` (`xtermres` calls `xsetblk(0, run, blkln)`),
shell post-`Pexec` size adjustment, programs that grab "all
memory" via `Malloc(-1)` then trim.

## Cross-references

- **Compendium:** "Mshrink" — agrees, including unused `n`.
- **Profibuch:** "Mshrink (0x4A)" — agrees.
- **`docs/api/gemdos/process/03-ptermres.md`** — primary internal user.
- **`docs/api/gemdos/memory/49-mfree.md`** — `len = 0` case.
- **`docs/api/structures/md.md`**.

## Configuration

- `malloc_align_stram` is `1` (= 2-byte align) for 68000-class
  builds, `3` (= 4-byte align) when LONG alignment is mandated.
  Alt-RAM is always 4-byte aligned.

## NewTOS implementation notes

Phase 04 (D10) plan:

- **Alignment grain.** `malloc_align_stram = 15` (16-byte) for
  AArch64 SIMD friendliness; `malloc_align_ttram = 15` likewise.
  Historic Atari TOS uses 2 / 4-byte alignment; the rounded `len`
  is echoed back to the caller as part of `Mshrink` semantics, so
  programs that recompute their heap top must consult the returned
  size rather than the requested one.
- **Trailing-MD reuse.** If shrinking creates a tail of
  `>= sizeof(MD)` bytes, allocate a fresh MD via `xmgetmd`,
  initialise it as free, and link it into `mp_mfl` adjacent to the
  shrunk block. Coalesce immediately if the next MD on `mp_mfl` is
  contiguous.
- **Zero-grow fast path.** Shrink of `0` bytes (no-op) and
  same-size shrink return `E_OK` without consulting the MPB.
