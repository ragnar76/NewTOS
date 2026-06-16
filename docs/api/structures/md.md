# `MD` / `MPB` — Memory Descriptor and Memory Partition Block

> Phase 00 spec D6. Sources: EmuTOS @ `1a7484ab` —
> `include/bdosdefs.h:14-29` (`MD`, `MPB`, `MX_*`),
> `bdos/mem.c` (allocator),
> `bdos/iumem.c` (`ffit`, `freeit`, `shrinkit`, `xmaddalt`),
> `bdos/osmem.c` (MD meta-allocator: `xmgetmd`, `xmfremd`).

## Purpose

GEMDOS heap allocator metadata.

- **`MD`** (Memory Descriptor) describes one allocated or free
  contiguous region.
- **`MPB`** (Memory Partition Block) is the head of a free list +
  allocated list pair. EmuTOS uses one MPB per memory pool (ST-RAM,
  optionally TT-RAM).

This is the data structure backing `Malloc`, `Mfree`, `Mxalloc`,
`Mshrink`, and `Pexec`-time TPA carve-out.

## Binary layout — `MD` (16 bytes)

```c
struct _md {
/* 0x00 */ MD    *m_link;     /* next MD in this list, or NULL */
/* 0x04 */ UBYTE *m_start;    /* start address of region       */
/* 0x08 */ LONG   m_length;   /* region size in bytes          */
/* 0x0C */ PD    *m_own;      /* owner's PD, or NULL if free   */
};
```

## Binary layout — `MPB` (12 bytes)

```c
struct _mpb {
/* 0x00 */ MD *mp_mfl;        /* memory free list head       */
/* 0x04 */ MD *mp_mal;        /* memory allocated list head  */
/* 0x08 */ MD *mp_rover;      /* roving pointer (unused)     */
};
```

Comment in the header: *"roving pointer - no longer used"*. EmuTOS
keeps the field for ABI compatibility with code that walks the MPB
externally.

## Allocator semantics

### Allocation strategy

First-fit linear scan of `mp_mfl`. EmuTOS does **not** use a roving
pointer or buddy/best-fit. The free list is kept sorted by ascending
`m_start` so coalescing on free is O(n) in the list length.

### Allocation flow (`bdos/mem.c:xmalloc`)

1. Walk `mp_mfl`. First MD with `m_length >= request`: split.
2. If the MD's length equals the request, unlink it and re-use the
   MD struct.
3. Otherwise, carve `request` bytes off the front: shrink the free MD
   (`m_start += request`, `m_length -= request`), allocate a new MD
   for the allocated region, prepend it to `mp_mal` with
   `m_own = current PD`.

### Free flow (`bdos/mem.c:xmfree`)

1. Find the MD on `mp_mal` whose `m_start == addr`. If none → `EIMBA`.
2. Unlink from `mp_mal`, set `m_own = NULL`, insert into `mp_mfl`
   at the position keeping it sorted.
3. Coalesce with adjacent free MDs (front and back).

> **Note on ownership.** `Mfree` does **not** check `m_own ==
> current PD` — EmuTOS preserves the original Atari quirk that any
> process can free any block by passing its address. Ownership is
> consulted only by `Pterm`'s `free_all_owned` path (see below),
> not by interactive `Mfree`. This was incorrectly stated as a
> validation step in earlier revisions of this document; the
> per-function spec at `docs/api/gemdos/memory/49-mfree.md`
> § "Edge cases — No ownership check" is authoritative.

### `Pterm` cleanup

When a process terminates, EmuTOS walks `mp_mal` and frees every MD
with `m_own == &terminating_PD`. This is the mechanism that prevents
process leaks, including basepage and TPA storage.

## Multiple pools (`MX_*` modes)

`Mxalloc(size, mode)` selects a pool based on `mode`:

| `mode` | Constant         | Behaviour                         |
|-------:|------------------|-----------------------------------|
| 0      | `MX_STRAM`       | ST-RAM only                       |
| 1      | `MX_TTRAM`       | TT-RAM only (fail if none)        |
| 2      | `MX_PREFSTRAM`   | ST-RAM preferred, fall back TT    |
| 3      | `MX_PREFTTRAM`   | TT-RAM preferred, fall back ST    |

`MX_MODEMASK = 0x03`. Bits 2..15 of `mode` are reserved; EmuTOS
clears them with `mode &= MX_MODEMASK` (`bdos/umem.c:241`) before
dispatching, so caller-supplied protection bits (Atari documents
bits 4..15 as supervisor/global flags) are silently ignored.
NewTOS does the same and records the divergence under
`docs/api/gemdos/memory/44-mxalloc.md` § "Known divergences".

Each pool has its own MPB. EmuTOS's globals: `pmd` (ST-RAM MPB),
`pmdalt` (TT-RAM MPB, only when `CONF_WITH_TTRAM`).

NewTOS: two MPBs from the start (one per pool). See
"NewTOS implementation notes" below.

## MD meta-allocator (`xmgetmd` / `xmfremd`)

MD structs themselves need storage. EmuTOS does **not** allocate
them on the heap they manage — it would be a bootstrap problem.
Instead it carves them out of a fixed-size internal OS-memory pool
(`osmem.c`):

- A static word array `osmem[LENOSM]` (~7.7 KiB) sized for
  `NUM_OSM_BLOCKS = 118` blocks of `LEN_OSM_BLOCK = 66` bytes each
  (`bdos/osmem.c:36-39`).
- `xmgetblk(WORD i)` / `xmfreblk` allocate fixed-size blocks from
  this pool via a quick-list of LIFO free chains indexed by
  paragraph count (`root[MAXQUICK]`, `MAXQUICK = 5`). Only index 4
  (= 4 paragraphs = 64 bytes) is used in practice.
- MDs are not allocated one-by-one. They live in `MDBLOCK`s, each
  holding `MDS_PER_BLOCK = 3` MDs plus a `mdb_next` link
  (`bdos/osmem.c:44-55`):

```c
typedef struct {
    MD   md;
    WORD index;        /* slot index 0..2 if used, else -1 */
} MDEXT;

struct _mdb {
    MDBLOCK *mdb_next;
    MDEXT    entry[3];
};
```

`MDBLOCK` size: `4 + 3*(16+2) = 58` bytes; rounded up to a
64-byte `osmem` block.

### `xmgetmd` (`bdos/osmem.c:169`)

1. If `mdbroot` (chain of partially-used MDBLOCKs) is empty,
   `MGET(MDBLOCK)` allocates a fresh one from `osmem` and links it
   in.
2. Walk the entries of the head MDBLOCK; pick the first slot with
   `index < 0`; set `index = i` (slot number 0..2); return
   `&entry[i].md`.
3. If that allocation made the MDBLOCK full (no remaining `index <
   0`), unlink it from `mdbroot`. It will be re-linked on the next
   `xmfremd` that frees one of its MDs.

### `xmfremd` (`bdos/osmem.c:229`)

1. Read the *word following* the MD (`*(WORD *)(md+1)`) — this is
   the slot's `index` field. EmuTOS relies on the
   `MD`/`WORD` adjacency in `MDEXT` to recover the index without a
   back-pointer.
2. From `index` reconstruct the start of `entry[]`, then the
   `MDBLOCK` (one pointer-sized field before).
3. Mark the slot free (`index = -1`). Count remaining free slots:
   - 1 free → add MDBLOCK back to `mdbroot`.
   - 3 free → unlink and return the MDBLOCK to the `osmem` quick
     list via `xmfreblk`.
   - 2 free → already on chain, do nothing.

### Why this matters for NewTOS

The `MDEXT` trick — reading `*(WORD *)(md+1)` to recover the slot
index — is **a layout invariant**, not just an allocator detail.
If the Rust port stores `MD` in a way that puts something other
than `index: i16` immediately after the 16 bytes of MD payload,
`xmfremd` semantics break for any caller that holds an MD pointer
long enough for the meta-allocator to consult it.

NewTOS implementation: store MDs in an `MDExt { md: MD, slot: i16,
_pad: i16 }` `#[repr(C)]` struct grouped 3 per `MDBlock`. The MD
pointer handed to BDOS code is `&mut md`. The 2-byte `slot` field
follows `md` in memory; pad to keep alignment. See PHASE-04 D5.

## Sizing constraints

- `osmem` capacity: 118 blocks of 64 bytes = 7552 bytes. Each
  `MDBLOCK` consumes one block and provides 3 MDs, so the absolute
  ceiling is `118 * 3 = 354` MDs — minus whatever `osmem` storage
  the OFD/DND/handle tables consume.
- Real-world EmuTOS observation: typical workloads peak around
  60–80 simultaneously live MDs.
- NewTOS PHASE-04 D5 plans for **256 MDs** (≡ 86 `MDBlock`s) backed
  by a static array in `tos-api::bdos::mem`. We do **not** port the
  `osmem` quick-list; we allocate `MDBlock` slots from a static
  bump arena because we don't share storage with OFD/DND tables
  (those will get their own arenas in PHASE-08/PHASE-09).

## Invariants

- Free list is sorted ascending by `m_start`; allocated list is in
  insertion order.
- Adjacent regions on the free list are always coalesced —
  `mp_mfl` never contains two MDs `A`, `B` with
  `A->m_start + A->m_length == B->m_start`.
- Every MD on `mp_mal` has `m_own != NULL`.
- Every MD on `mp_mfl` has `m_own == NULL` (used as the free flag).
- `m_length > 0` for all MDs (zero-length MDs are deleted on free).
- The MD struct itself is allocated separately from the region it
  describes (in a dedicated MD pool — see `bdos/mem.c:mdalloc`).

## Cross-references

- **Compendium:** "Memory Allocation" / "MD" — agrees on layout.
- **Profibuch:** "Speicherverwaltung" — agrees.
- **`docs/api/structures/basepage.md`** — `m_own` points to PD.
- **`docs/api/gemdos/memory/Malloc.md`** (TBD) — uses MPB.
- **EmuTOS source:** `bdos/mem.c` — full allocator.

## Configuration

- `CONF_WITH_TTRAM` enables the second pool (`pmdalt`).
- `CONF_WITH_FRB` reserves a 64 KiB ST-RAM buffer for DMA fallback —
  not part of the regular MPB.

## NewTOS implementation notes

Phase 04 plan (specs:
[`docs/api/gemdos/memory/`](../gemdos/memory/)):

- **Two pools from day one.** ST-RAM MPB sits over the `Pool::Stram`
  frame slot from
  [`kernel/src/bios/machine/layout.rs`](../../../kernel/src/bios/machine/layout.rs);
  TT-RAM MPB over `Pool::Ttram`. `MX_PREFSTRAM` / `MX_PREFTTRAM`
  fall through cleanly. The single-pool fallback EmuTOS uses for
  pre-TT machines is irrelevant here — every NewTOS target has
  TT-RAM.
- **MD storage.** `MDBlock` (3 MDs + `mdb_next`) in
  `tos-api/src/bdos/mem/`. Static arena of 86 blocks = 256 MDs.
  The `MDEXT { md, slot }` layout invariant is preserved so the
  `xmfremd`-style index recovery from `*(WORD*)(md+1)` works
  byte-for-byte, even though we will write `xmfremd` in Rust.
- **MPB storage.** Two `MPB` statics in `tos-api::bdos::mem`,
  initialised once during BIOS bring-up after
  `bios::machine::frames::init`.
- **Region storage.** `m_start` points into the corresponding pool's
  frame range. The MPB free list is seeded with one MD per pool
  covering the whole pool extent. Subsequent `Mxalloc` calls
  carve from there.
- **Owner field.** `m_own` is `Option<NonNull<BasePage>>` —
  `None` for free MDs. Until Phase 06 introduces real PDs, every
  alloc gets a sentinel `KERNEL_PD` pointer so the
  `Pterm`-cleanup mechanism is structurally correct from the
  start.
- **Allocations beyond ABI width.** TOS `Malloc` returns `LONG`
  (i32). On QEMU the entire memory map fits in 32 bits; on RPi4
  with `>1` GiB DRAM the high TT-RAM region does not. Phase 05
  introduces a low-half address window for user pointers; until
  then RPi4 EC verification is gated.
