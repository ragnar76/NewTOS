# `Mxalloc` — GEMDOS 0x44

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/umem.c:234` (`xmxalloc`).

## EmuTOS source

- **Primary:** `bdos/umem.c:234` — `void *xmxalloc(long amount, int mode)`
- **Helpers:** `ffit()` (`bdos/mem.c`) — first-fit allocator,
  `dump_mem_map()` — debug only
- **Globals read/written:** `pmd` (ST-RAM MPB), `pmdalt` (alt-RAM MPB)
- **Build config:** `CONF_WITH_ALT_RAM`.

## Source extraction (essence)

```c
void *xmxalloc(long amount, int mode)
{
    MD *m;
    mode &= MX_MODEMASK;     /* keep only bits 0..1 */

    /* amount == -1: report largest free block in selected pool(s) */
    if (amount == -1L) {
        switch (mode) {
        case MX_STRAM:     return ffit(-1L, &pmd);
        case MX_TTRAM:     return ffit(-1L, &pmdalt);
        case MX_PREFSTRAM:
        case MX_PREFTTRAM: {
            void *r = ffit(-1L, &pmd);
            void *t = ffit(-1L, &pmdalt);
            return r > t ? r : t;          /* biggest of either */
        }
        default: return NULL;
        }
    }

    if (amount <= 0) return NULL;

    switch (mode) {
    case MX_STRAM:     m = ffit(amount, &pmd);  break;
    case MX_TTRAM:     m = ffit(amount, &pmdalt); break;
    case MX_PREFSTRAM: m = ffit(amount, &pmd);
                       if (!m) m = ffit(amount, &pmdalt);  break;
    case MX_PREFTTRAM: m = ffit(amount, &pmdalt);
                       if (!m) m = ffit(amount, &pmd);     break;
    default:           m = NULL;
    }
    return m ? m->m_start : NULL;
}
```

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0044` |
| WORD args | 3 (LONG `amount` + WORD `mode`) |
| Stack frame | `+6: mode (WORD)`, `+2: amount (LONG)`, `+0: 0x0044` |

## Inputs

| Pos | Name     | Type | Range / meaning                            |
|-----|----------|------|--------------------------------------------|
| 0   | `amount` | LONG | bytes to allocate, `-1L` for query         |
| 1   | `mode`   | WORD | pool selector (only bits 0..1 used)        |

### `mode` values (`include/bdosdefs.h:18-22`)

| Mode | Constant         | Behaviour                              |
|------|------------------|----------------------------------------|
| 0    | `MX_STRAM`       | ST-RAM only                            |
| 1    | `MX_TTRAM`       | TT-RAM only                            |
| 2    | `MX_PREFSTRAM`   | ST-RAM preferred, fallback TT-RAM      |
| 3    | `MX_PREFTTRAM`   | TT-RAM preferred, fallback ST-RAM      |

`mode & MX_MODEMASK = mode & 0x03` — upper bits silently ignored
(some Atari documentation defines bits 4..15 for "memory protection
mode" / "globally accessible"; EmuTOS does not implement those).

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG |

| Case                                  | Return                          |
|---------------------------------------|---------------------------------|
| `amount > 0`, success                 | pointer to allocated region     |
| `amount > 0`, no memory in pool       | `0L` (NULL)                     |
| `amount == -1L`                       | size of largest free block (LONG) |
| `amount <= 0` (and != -1L)            | `0L`                            |
| `mode` invalid                        | `0L`                            |
| `mode == MX_TTRAM` without alt-RAM    | `0L` (CONF_WITH_ALT_RAM=0)      |

## Errors

No GEMDOS error codes returned. `0L` (NULL) covers all failure
modes; callers must use the error log / context to distinguish.

## Edge cases

- **`MX_MODEMASK = 0x03`** masks the mode immediately on entry.
  Calls passing `mode = 0x40` (Atari's "memory protection: globally
  accessible") collapse to `MX_STRAM`. Programs depending on the
  protection-mode behaviour will malfunction silently — but EmuTOS
  has no MMU-based protection so the bits cannot be honoured anyway.
- **`amount == -1L` query**: returns the size of the largest *single*
  free MD, not the sum of all free space. Programs that want
  total-free-bytes must walk the free list themselves (or use the
  undocumented Atari `Maddalt` introspection — there is none).
- **`MX_PREFSTRAM` fallback** retries the alt pool only if the
  primary `ffit` returned NULL. There is no "split between pools"
  fallback — a single allocation always lives in one pool.
- **Negative `amount` other than `-1L`**: returns `0L`. Some Atari
  documentation suggests `-2L` etc. have meanings; EmuTOS treats
  them all as failure.
- **Owner attribution**: `ffit` sets `m_own = run` on success. The
  block belongs to the calling process and is freed by `Pterm`'s
  `free_all_owned` path.
- **Alignment**: same as `Malloc`; depends on pool. ST-RAM aligned to
  `malloc_align_stram` (typically WORD); alt-RAM aligned to LONG
  (`MALLOC_ALIGN_ALTRAM = 3`).
- **`amount` rounding**: the requested size may be rounded up by
  `ffit` to the alignment grain before the MD is sized. The caller
  sees a region at least as large as requested, but may not see the
  rounded-up size unless they query via `Mshrink(..., 0)`-then-fail.

## Dependencies

- `ffit` (`bdos/mem.c`) — first-fit search + MD split + ownership
  tagging.
- `pmd`, `pmdalt` MPBs initialised at boot.

Used by: `Malloc` wrapper, AES (which sets `MX_PREFTTRAM` for large
buffers), drivers needing DMA-safe ST-RAM via `MX_STRAM`.

## Cross-references

- **Compendium:** "Mxalloc" — agrees on modes 0..3 and `-1L`
  behaviour.
- **Profibuch:** "Mxalloc (0x44)" — agrees, notes higher mode bits
  for memory protection (not implemented).
- **`docs/api/gemdos/memory/48-malloc.md`** — wrapper.
- **`docs/api/gemdos/memory/49-mfree.md`** — companion.
- **`docs/api/structures/md.md`** — pool layout.

## Configuration

- `CONF_WITH_ALT_RAM` — without it, modes 1 and 3 always fail and 2
  collapses to ST-RAM-only. Mode 1 returns `NULL`, mode 3 returns
  whatever ST-RAM gives.

## NewTOS implementation notes

Phase 04 (D8) plan:

- **Two pools always present.** Both ST-RAM and TT-RAM MPBs are
  initialised at BIOS bring-up from the corresponding
  `bios::machine::layout::Pool` extents. There is no "no TT-RAM"
  configuration on AArch64 — the additive layout from PHASE-03 D1
  guarantees `>= 256 MiB` of TT-RAM on every supported board.
- **Mode dispatch.** `mode &= MX_MODEMASK` (clear the supervisor /
  global bits) before the switch. Then:
  - 0 (`MX_STRAM`)     → search `pmd` only.
  - 1 (`MX_TTRAM`)     → search `pmdalt` only.
  - 2 (`MX_PREFSTRAM`) → search `pmd`, on failure `pmdalt`.
  - 3 (`MX_PREFTTRAM`) → search `pmdalt`, on failure `pmd`.
  Other mode values → return `EINVFN` (the documented EmuTOS path).
- **Probe semantics.** `Mxalloc(-1, mode)` returns the largest free
  MD in the selected pool — implemented as a separate `largest_free`
  scan on `mp_mfl` so callers can size buffers without committing.
- **Atari divergence — protection bits ignored.** Atari documents
  bits 4..15 of `mode` as supervisor/global flags. EmuTOS clears
  them via `MX_MODEMASK` before dispatch (`bdos/umem.c:241`); we do
  the same. Programs that rely on the supervisor-only flag for
  safety on real hardware will silently get a normal allocation on
  NewTOS. Recorded as a known divergence; revisit when Phase 06
  introduces process privilege.
