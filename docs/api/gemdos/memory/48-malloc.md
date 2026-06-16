# `Malloc` — GEMDOS 0x48

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bdos/umem.c:116` (`xmalloc`).

## EmuTOS source

- **Primary:** `bdos/umem.c:116` — `void *xmalloc(long amount)`
- **Delegates to:** `xmxalloc` (`Mxalloc`, GEMDOS 0x44) with mode
  derived from the calling process's `p_flags`.
- **Globals read:** `run->p_flags` (program's TT-RAM preference bit).

## Source extraction

```c
void *xmalloc(long amount)
{
    void *rc;
    if (run->p_flags & PF_TTRAMMEM) {
        rc = xmxalloc(amount, MX_PREFTTRAM);
    } else {
        rc = xmxalloc(amount, MX_STRAM);
    }
    return rc;
}
```

`Malloc` is a **policy wrapper** around `Mxalloc`. The mode is
chosen automatically from `PF_TTRAMMEM` (bit 2 of the program
header's `PRGFLAGS`):

| `p_flags & PF_TTRAMMEM` | Mode used                        |
|------------------------|----------------------------------|
| 0                      | `MX_STRAM` (ST-RAM only)          |
| `PF_TTRAMMEM` (0x04)   | `MX_PREFTTRAM` (TT-RAM preferred) |

## Trap interface

| | |
|--|--|
| Trap | 1 (GEMDOS) |
| Function ID | `0x0048` |
| WORD args | 2 (LONG) |
| Stack frame | `+2: amount (LONG)`, `+0: 0x0048 (fn)` |

## Inputs

| Pos | Name     | Type | Range / meaning                                      |
|-----|----------|------|------------------------------------------------------|
| 0   | `amount` | LONG | bytes to allocate, or `-1L` for "biggest free block" |

## Outputs

| | |
|--|--|
| Reg | D0 |
| Type | LONG (pointer or special) |

| `amount` | Return                                              |
|----------|-----------------------------------------------------|
| `> 0`    | pointer to allocated region (cast to LONG), or `0L` if no memory |
| `-1L`    | size in bytes of the largest free block (LONG)     |
| `0`      | `NULL` (`xmxalloc` returns NULL for amount ≤ 0)    |
| `< -1`   | `NULL`                                              |

## Errors

`Malloc` does not return GEMDOS error codes. A `NULL` (`0L`) return
indicates failure (typically out-of-memory). Callers cannot
distinguish "no memory" from a legitimate-but-impossible
zero-address allocation; in practice user RAM never starts at 0.

## Edge cases

- **`PF_TTRAMMEM` semantics**: programs without the bit set get
  ST-RAM only, even if TT-RAM is available. This preserves DMA
  compatibility — old programs would crash if `Malloc` returned
  TT-RAM addresses unsuitable for the floppy DMA controller.
  NewTOS: AArch64 has no DMA-zone constraint per se; the flag is
  preserved for ABI-compat but practically maps to "any RAM page".
- **`amount == -1L`** is the documented "query biggest free block"
  protocol. Used by programs that want to grab "all available
  memory" — they query first, then `Malloc(size)`.
- **Owner attribution**: `xmxalloc` (via `ffit`) sets `m_own = run`
  on the new MD. Memory is freed automatically on the calling
  process's `Pterm`.
- **Alignment**: blocks are aligned to `malloc_align_stram` for ST
  RAM (`bdos/mem.c` — typically 2 bytes) or `MALLOC_ALIGN_ALTRAM`
  (4 bytes). The allocated size is rounded up to the alignment
  boundary; the caller sees the rounded-up MD length on
  `Mshrink`/`Mfree`.
- **No header**: unlike libc `malloc`, GEMDOS `Malloc` returns the
  raw block start; there is no in-band length prefix. The MD list
  is the only metadata.

## Dependencies

- `xmxalloc` (full implementation lives there).
- `run->p_flags` set by `Pexec` from program header.

Used by: every C-runtime `malloc()`, AES allocations, TOS programs
that build their own dynamic structures.

## Cross-references

- **Compendium:** "Malloc" — agrees including `-1L` behaviour.
- **Profibuch:** "Malloc (0x48)" — agrees.
- **`docs/api/gemdos/memory/44-mxalloc.md`** — actual logic.
- **`docs/api/structures/md.md`** — MD/MPB layout.

## Configuration

- `CONF_WITH_ALT_RAM` — enables TT-RAM path.
- `PF_TTRAMMEM` — process-flag bit set by program header.

## NewTOS implementation notes

Phase 04 (D8) plan:

- `Malloc(amount)` with `amount >= 0` is a thin wrapper over
  `Mxalloc(amount, default_mode)` where `default_mode` derives
  from the caller PD's `PF_TTRAMMEM` flag (set in the GEMDOS
  header by the loader): `PF_TTRAMMEM` → `MX_PREFTTRAM`,
  cleared → `MX_STRAM`. Until Phase 06 wires up real PDs, the
  kernel sentinel PD has `PF_TTRAMMEM = 1`.
- `Malloc(-1)` returns the largest free block across both pools
  (max of `largest_free(pmd)` and `largest_free(pmdalt)`).
  EmuTOS implements this in `umem.c` by probing each pool
  separately with mode 0 / 1; we mirror that.
- **Return type vs. AArch64 pointers.** Return value is `i32` /
  `LONG`. On QEMU and on RPi4 with `mem=1G` everything fits.
  Above 1 GiB of TT-RAM (RPi4 with `>= 2` GiB) the returned
  address may not be representable in `i32`. Phase 05 introduces
  a low-half user address window; until then, RPi4 ≥ 2 GiB
  configurations are gated.
