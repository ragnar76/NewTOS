# Phase 04 — BDOS Memory Layer

> **Status:** in progress (D0–D8 done 2026-05-03).

## Goal

Implement the BDOS memory subsystem the way EmuTOS structures it:
a Memory Descriptor (MD) / Memory Pool Block (MPB) layout, the
`ffit` first-fit search, the `freeit` coalescing free, and the five
TOS memory syscalls that sit on top of them.

This is the layer ported source code calls through `Malloc()`,
`Mxalloc()`, `Mfree()`, `Mshrink()`. It is **separate** from the
kernel heap (Phase 03 D5) — the kernel heap backs Rust-internal
allocations; the BDOS pools back GEM/TOS programs.

## EmuTOS sources to read

- `bdos/mem.c` — `MD`/`MPB` structures, `ffit`, `freeit`, `shrinkit`,
  `xmgetmd`, `xmfremd`, `find_mpb`.
- `bdos/umem.c` — the trap-facing wrappers `xmalloc`, `xmxalloc`,
  `xmfree`, `xsetblk`, `xmaddalt`.
- `bdos/iumem.c` — pool initialisation (`pmd`, `pmdalt`).
- `bdos/bdosmain.c` — call path from `osinit` to `bmem_init`.
- `bdos/bdosstub.c` — MD-pool allocator helpers.
- `include/biosdefs.h` — `MD`, `MPB`, `MX_*` constants.

## Phase 00 prerequisites (specs that must exist)

- `docs/api/gemdos/memory/14-maddalt.md` ✅
- `docs/api/gemdos/memory/44-mxalloc.md` ✅
- `docs/api/gemdos/memory/48-malloc.md`  ✅
- `docs/api/gemdos/memory/49-mfree.md`   ✅
- `docs/api/gemdos/memory/4a-mshrink.md` ✅
- `docs/api/structures/md.md` (MD/MPB layout)

## Deliverables

| Id | Title                                                    |
|----|----------------------------------------------------------|
| D0 | Source-tree refactor: create `tos-api/src/bdos/mem/`     ✅ |
| D1 | `MD`/`MPB` structures + free-list helpers                ✅ |
| D2 | `ffit` first-fit search (mirrors `bdos/mem.c`)           ✅ |
| D3 | `freeit` insert + coalesce                               ✅ |
| D4 | `shrinkit` block split                                   ✅ |
| D5 | MD-pool: `xmgetmd` / `xmfremd` (the meta-allocator)      ✅ |
| D6 | Pool init (`pmd` from ST-RAM, `pmdalt` from TT-RAM)      ✅ |
| D7 | `Maddalt` (0x14) trap                                    ✅ |
| D8 | `Mxalloc` (0x44) trap                                    ✅ |
| D9 | `Malloc` (0x48) wrapper around `Mxalloc`                 |
| D10| `Mfree` (0x49) trap                                      |
| D11| `Mshrink` (0x4A) trap                                    |
| D12| Owner attribution (`m_own`) — set on alloc, used in P06  |
| D13| `heap_stats`-equivalent: `pool_stats(pool)` for diagnostics |

## Exit criteria

- **EC1**: 1000-iteration alloc/free churn against `pmd` and `pmdalt`
  separately leaves both pools at the same MD count and free-byte
  total they had at start.
- **EC2**: `Mxalloc(-1, MX_STRAM)` returns the largest free MD in
  `pmd`, `Mxalloc(-1, MX_TTRAM)` the largest in `pmdalt`,
  `Mxalloc(-1, MX_PREFSTRAM)` the larger of the two — verified by
  comparing against `frames::stats(...)`.
- **EC3**: `Mxalloc(SIZE, MX_STRAM)` returns an address within the
  ST-RAM slot; `MX_TTRAM` returns an address within the TT-RAM slot;
  `MX_PREFSTRAM` falls back to TT-RAM when ST-RAM is exhausted.
- **EC4**: `Mfree(ptr)` of a wild pointer returns `EIMBA`, not a
  crash. Double-free returns `EIMBA`.
- **EC5**: `Mshrink(0, blk, smaller)` succeeds and produces a free MD
  for the released tail; `Mshrink(0, blk, bigger)` returns `EGSBF`.
- **EC6**: `Mshrink(0, blk, 0)` is equivalent to `Mfree(blk)`.
- **EC7**: `Maddalt(start, size)` rejects ST-RAM-overlapping ranges
  with `-1`, accepts disjoint TT-RAM additions, and merges adjacent
  free MDs without consuming a fresh MD.
- **EC8**: All five trap entries dispatched correctly through
  `tos-api::gemdos::dispatch`.
- **EC9**: Phase 02 RX-IRQ smoke test still green after this phase.

## Out of scope

- Per-process pools (Phase 06 wires `m_own` to the running BASEPAGE).
- Memory protection bits in `mode` (Atari documentation defines
  bits 4..15 — EmuTOS ignores them, we do too; record as known
  divergence).
- Aligning to anything finer than 16 bytes (AArch64 baseline). EmuTOS
  uses 2-byte ST-RAM alignment; we document the divergence in
  `docs/api/gemdos/memory/4a-mshrink.md`.

## Risks

- **R1 — MD-pool exhaustion**: `xmgetmd` failure cascades into
  `ENSMEM` from `Maddalt` and `ERR` from `Mshrink`. Mitigation:
  generous static MD pool (256 MDs) sized per phase planning.
- **R2 — fragmentation under churn**: first-fit fragments. Live with
  it for now; ADR-trigger for slab/buddy on demand.
- **R3 — Atari quirks vs. clean OS instinct**: `Mfree` does *not*
  check ownership. Easy to "improve" — don't, it breaks ABI.
- **R4 — pointer-to-LONG cast**: AArch64 pointers are 64-bit;
  `Malloc` returns `LONG` (32-bit signed) per Atari ABI. Phase 05
  introduces an address-window translation; until then ST-RAM and
  TT-RAM addresses fit in 32 bits *only* on QEMU. RPi4 high RAM
  needs Phase 05 first. **Block Phase 04 EC behind QEMU only**;
  RPi4 verification waits for Phase 05.

## Implementation notes

- Per CLAUDE.md §5, every trap implementation cites its
  `docs/api/...` entry by ID in the commit subject.
- Keep `MD`/`MPB` as Rust types in `tos-types` so userland headers
  (later) and BDOS internals share them.
